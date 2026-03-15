use std::sync::{Arc, Mutex};

use ash::vk;
use gpu_allocator::vulkan::{Allocation, Allocator};

use crate::renderer::chunk::buffer::{ChunkAABB, DrawIndexedIndirectCommand, MAX_CHUNKS};
use crate::renderer::shader;
use crate::renderer::util;
use crate::renderer::MAX_FRAMES_IN_FLIGHT;
const INDIRECT_STRIDE: u64 = std::mem::size_of::<DrawIndexedIndirectCommand>() as u64;
const AABB_STRIDE: u64 = std::mem::size_of::<ChunkAABB>() as u64;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CullPushConstants {
    pub frustum_planes: [[f32; 4]; 6],
    pub draw_count: u32,
    pub _pad: [u32; 3],
}

struct PerFrameData {
    indirect_buffer: vk::Buffer,
    indirect_alloc: Allocation,
    aabb_buffer: vk::Buffer,
    aabb_alloc: Allocation,
    descriptor_set: vk::DescriptorSet,
}

pub struct CullPipeline {
    pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    frames: Vec<PerFrameData>,
}

impl CullPipeline {
    pub fn new(device: &ash::Device, allocator: &Arc<Mutex<Allocator>>) -> Self {
        let bindings = [
            vk::DescriptorSetLayoutBinding {
                binding: 0,
                descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::COMPUTE,
                ..Default::default()
            },
            vk::DescriptorSetLayoutBinding {
                binding: 1,
                descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::COMPUTE,
                ..Default::default()
            },
        ];
        let layout_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
        let descriptor_set_layout =
            unsafe { device.create_descriptor_set_layout(&layout_info, None) }
                .expect("failed to create cull descriptor set layout");

        let push_range = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::COMPUTE,
            offset: 0,
            size: std::mem::size_of::<CullPushConstants>() as u32,
        }];
        let layouts = [descriptor_set_layout];
        let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&layouts)
            .push_constant_ranges(&push_range);
        let pipeline_layout = unsafe { device.create_pipeline_layout(&pipeline_layout_info, None) }
            .expect("failed to create cull pipeline layout");

        let comp_spv = shader::include_spirv!("cull.comp.spv");
        let comp_module = shader::create_shader_module(device, comp_spv);

        let stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(comp_module)
            .name(c"main");

        let pipeline_info = [vk::ComputePipelineCreateInfo::default()
            .stage(stage)
            .layout(pipeline_layout)];
        let pipeline = unsafe {
            device.create_compute_pipelines(vk::PipelineCache::null(), &pipeline_info, None)
        }
        .expect("failed to create cull compute pipeline")[0];

        unsafe { device.destroy_shader_module(comp_module, None) };

        let pool_sizes = [vk::DescriptorPoolSize {
            ty: vk::DescriptorType::STORAGE_BUFFER,
            descriptor_count: MAX_FRAMES_IN_FLIGHT as u32 * 2,
        }];
        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .max_sets(MAX_FRAMES_IN_FLIGHT as u32)
            .pool_sizes(&pool_sizes);
        let descriptor_pool = unsafe { device.create_descriptor_pool(&pool_info, None) }
            .expect("failed to create cull descriptor pool");

        let set_layouts: Vec<_> = (0..MAX_FRAMES_IN_FLIGHT)
            .map(|_| descriptor_set_layout)
            .collect();
        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&set_layouts);
        let sets = unsafe { device.allocate_descriptor_sets(&alloc_info) }
            .expect("failed to allocate cull descriptor sets");

        let mut frames = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
        for set in sets {
            let indirect_size = MAX_CHUNKS as u64 * INDIRECT_STRIDE;
            let aabb_size = MAX_CHUNKS as u64 * AABB_STRIDE;

            let (indirect_buffer, indirect_alloc) = util::create_host_buffer(
                device,
                allocator,
                indirect_size,
                vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::INDIRECT_BUFFER,
                "cull_indirect",
            );
            let (aabb_buffer, aabb_alloc) = util::create_host_buffer(
                device,
                allocator,
                aabb_size,
                vk::BufferUsageFlags::STORAGE_BUFFER,
                "cull_aabb",
            );

            let buffer_infos_indirect = [vk::DescriptorBufferInfo {
                buffer: indirect_buffer,
                offset: 0,
                range: indirect_size,
            }];
            let buffer_infos_aabb = [vk::DescriptorBufferInfo {
                buffer: aabb_buffer,
                offset: 0,
                range: aabb_size,
            }];

            let writes = [
                vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(0)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .buffer_info(&buffer_infos_indirect),
                vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(1)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .buffer_info(&buffer_infos_aabb),
            ];
            unsafe { device.update_descriptor_sets(&writes, &[]) };

            frames.push(PerFrameData {
                indirect_buffer,
                indirect_alloc,
                aabb_buffer,
                aabb_alloc,
                descriptor_set: set,
            });
        }

        Self {
            pipeline,
            pipeline_layout,
            descriptor_set_layout,
            descriptor_pool,
            frames,
        }
    }

    pub fn upload_and_dispatch(
        &mut self,
        device: &ash::Device,
        cmd: vk::CommandBuffer,
        frame: usize,
        frustum_planes: &[[f32; 4]; 6],
        chunk_buffers: &crate::renderer::chunk::buffer::ChunkBufferStore,
    ) -> u32 {
        let draw_count = chunk_buffers.chunk_count();
        if draw_count == 0 {
            return 0;
        }

        let fd = &mut self.frames[frame];

        let indirect_slice = fd.indirect_alloc.mapped_slice_mut().unwrap();
        let commands: &mut [DrawIndexedIndirectCommand] = bytemuck::cast_slice_mut(
            &mut indirect_slice[..draw_count as usize * INDIRECT_STRIDE as usize],
        );

        let aabb_slice = fd.aabb_alloc.mapped_slice_mut().unwrap();
        let aabbs: &mut [ChunkAABB] =
            bytemuck::cast_slice_mut(&mut aabb_slice[..draw_count as usize * AABB_STRIDE as usize]);

        let count = chunk_buffers.write_draw_data(commands, aabbs);

        let push = CullPushConstants {
            frustum_planes: *frustum_planes,
            draw_count: count,
            _pad: [0; 3],
        };

        unsafe {
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, self.pipeline);
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::COMPUTE,
                self.pipeline_layout,
                0,
                &[fd.descriptor_set],
                &[],
            );
            device.cmd_push_constants(
                cmd,
                self.pipeline_layout,
                vk::ShaderStageFlags::COMPUTE,
                0,
                bytemuck::bytes_of(&push),
            );

            let workgroups = count.div_ceil(64);
            device.cmd_dispatch(cmd, workgroups, 1, 1);

            let barrier = vk::BufferMemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                .dst_access_mask(vk::AccessFlags::INDIRECT_COMMAND_READ)
                .buffer(fd.indirect_buffer)
                .offset(0)
                .size(vk::WHOLE_SIZE);

            device.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::DRAW_INDIRECT,
                vk::DependencyFlags::empty(),
                &[],
                &[barrier],
                &[],
            );
        }

        count
    }

    pub fn indirect_buffer(&self, frame: usize) -> vk::Buffer {
        self.frames[frame].indirect_buffer
    }

    pub fn destroy(&mut self, device: &ash::Device, allocator: &Arc<Mutex<Allocator>>) {
        let mut alloc = allocator.lock().unwrap();
        for fd in self.frames.drain(..) {
            unsafe {
                device.destroy_buffer(fd.indirect_buffer, None);
                device.destroy_buffer(fd.aabb_buffer, None);
            }
            alloc.free(fd.indirect_alloc).ok();
            alloc.free(fd.aabb_alloc).ok();
        }
        drop(alloc);

        unsafe {
            device.destroy_pipeline(self.pipeline, None);
            device.destroy_pipeline_layout(self.pipeline_layout, None);
            device.destroy_descriptor_pool(self.descriptor_pool, None);
            device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
        }
    }
}
