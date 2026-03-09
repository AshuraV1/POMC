use std::sync::{Arc, Mutex};

use ash::vk;
use gpu_allocator::vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator};
use gpu_allocator::MemoryLocation;

use crate::renderer::shader;
use crate::renderer::util;

pub struct PanoramaPipeline {
    pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    scroll_layout: vk::DescriptorSetLayout,
    strip_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    scroll_set: vk::DescriptorSet,
    strip_set: vk::DescriptorSet,
    scroll_buffer: vk::Buffer,
    scroll_allocation: Option<Allocation>,
    strip_image: vk::Image,
    strip_view: vk::ImageView,
    strip_sampler: vk::Sampler,
    strip_allocation: Option<Allocation>,
    staging_buffer: vk::Buffer,
    staging_allocation: Option<Allocation>,
    has_strip: bool,
}

impl PanoramaPipeline {
    pub fn new(
        device: &ash::Device,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
        render_pass: vk::RenderPass,
        allocator: &Arc<Mutex<Allocator>>,
        assets_dir: &std::path::Path,
    ) -> Self {
        let scroll_layout = util::create_descriptor_set_layout(
            device,
            vk::DescriptorType::UNIFORM_BUFFER,
            vk::ShaderStageFlags::FRAGMENT,
        );
        let strip_layout = util::create_descriptor_set_layout(
            device,
            vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            vk::ShaderStageFlags::FRAGMENT,
        );

        let layouts = [scroll_layout, strip_layout];
        let layout_info = vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts);
        let pipeline_layout = unsafe { device.create_pipeline_layout(&layout_info, None) }
            .expect("failed to create panorama pipeline layout");

        let pipeline = create_pipeline(device, render_pass, pipeline_layout);

        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 1,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: 1,
            },
        ];
        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .max_sets(2)
            .pool_sizes(&pool_sizes);
        let descriptor_pool = unsafe { device.create_descriptor_pool(&pool_info, None) }
            .expect("failed to create panorama descriptor pool");

        let scroll_layouts = [scroll_layout];
        let scroll_alloc = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&scroll_layouts);
        let scroll_set = unsafe { device.allocate_descriptor_sets(&scroll_alloc) }
            .expect("failed to allocate scroll descriptor set")[0];

        let strip_layouts = [strip_layout];
        let strip_alloc = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&strip_layouts);
        let strip_set = unsafe { device.allocate_descriptor_sets(&strip_alloc) }
            .expect("failed to allocate strip descriptor set")[0];

        let (scroll_buffer, scroll_allocation) = create_scroll_buffer(device, allocator);

        let buffer_info = [vk::DescriptorBufferInfo {
            buffer: scroll_buffer,
            offset: 0,
            range: std::mem::size_of::<f32>() as u64,
        }];
        let write = vk::WriteDescriptorSet::default()
            .dst_set(scroll_set)
            .dst_binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .buffer_info(&buffer_info);
        unsafe { device.update_descriptor_sets(&[write], &[]) };

        let (strip_image, strip_view, strip_sampler, strip_alloc_mem, staging_buffer, staging_alloc_mem, has_strip) =
            load_panorama_strip(device, queue, command_pool, allocator, assets_dir);

        let image_info = [vk::DescriptorImageInfo {
            sampler: strip_sampler,
            image_view: strip_view,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        }];
        let strip_write = vk::WriteDescriptorSet::default()
            .dst_set(strip_set)
            .dst_binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(&image_info);
        unsafe { device.update_descriptor_sets(&[strip_write], &[]) };

        Self {
            pipeline,
            pipeline_layout,
            scroll_layout,
            strip_layout,
            descriptor_pool,
            scroll_set,
            strip_set,
            scroll_buffer,
            scroll_allocation: Some(scroll_allocation),
            strip_image,
            strip_view,
            strip_sampler,
            strip_allocation: Some(strip_alloc_mem),
            staging_buffer,
            staging_allocation: Some(staging_alloc_mem),
            has_strip,
        }
    }

    pub fn draw(&mut self, device: &ash::Device, cmd: vk::CommandBuffer, scroll: f32) {
        if !self.has_strip {
            return;
        }

        let bytes = scroll.to_le_bytes();
        self.scroll_allocation.as_mut().unwrap().mapped_slice_mut().unwrap()[..4]
            .copy_from_slice(&bytes);

        unsafe {
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &[self.scroll_set, self.strip_set],
                &[],
            );
            device.cmd_draw(cmd, 3, 1, 0, 0);
        }
    }

    pub fn destroy(&mut self, device: &ash::Device, allocator: &Arc<Mutex<Allocator>>) {
        let mut alloc = allocator.lock().unwrap();

        unsafe { device.destroy_buffer(self.scroll_buffer, None) };
        if let Some(a) = self.scroll_allocation.take() {
            alloc.free(a).ok();
        }

        unsafe {
            device.destroy_sampler(self.strip_sampler, None);
            device.destroy_image_view(self.strip_view, None);
        }
        if let Some(a) = self.strip_allocation.take() {
            alloc.free(a).ok();
        }
        unsafe { device.destroy_image(self.strip_image, None) };

        if let Some(a) = self.staging_allocation.take() {
            alloc.free(a).ok();
        }
        unsafe { device.destroy_buffer(self.staging_buffer, None) };

        drop(alloc);

        unsafe {
            device.destroy_pipeline(self.pipeline, None);
            device.destroy_pipeline_layout(self.pipeline_layout, None);
            device.destroy_descriptor_pool(self.descriptor_pool, None);
            device.destroy_descriptor_set_layout(self.scroll_layout, None);
            device.destroy_descriptor_set_layout(self.strip_layout, None);
        }
    }
}

fn create_scroll_buffer(
    device: &ash::Device,
    allocator: &Arc<Mutex<Allocator>>,
) -> (vk::Buffer, Allocation) {
    let buffer_info = vk::BufferCreateInfo::default()
        .size(std::mem::size_of::<f32>() as u64)
        .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);

    let buffer = unsafe { device.create_buffer(&buffer_info, None) }
        .expect("failed to create scroll buffer");
    let mem_reqs = unsafe { device.get_buffer_memory_requirements(buffer) };

    let allocation = allocator
        .lock()
        .unwrap()
        .allocate(&AllocationCreateDesc {
            name: "panorama_scroll",
            requirements: mem_reqs,
            location: MemoryLocation::CpuToGpu,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })
        .expect("failed to allocate scroll buffer memory");

    unsafe {
        device
            .bind_buffer_memory(buffer, allocation.memory(), allocation.offset())
            .expect("failed to bind scroll buffer memory");
    }

    (buffer, allocation)
}

fn load_panorama_strip(
    device: &ash::Device,
    queue: vk::Queue,
    command_pool: vk::CommandPool,
    allocator: &Arc<Mutex<Allocator>>,
    assets_dir: &std::path::Path,
) -> (vk::Image, vk::ImageView, vk::Sampler, Allocation, vk::Buffer, Allocation, bool) {
    let panorama_dir = assets_dir.join("assets/minecraft/textures/gui/title/background");

    let mut faces: Vec<Vec<u8>> = Vec::new();
    let mut face_w = 0u32;
    let mut face_h = 0u32;

    for i in 0..6 {
        let path = panorama_dir.join(format!("panorama_{i}.png"));
        match load_png_rgba(&path) {
            Some((data, w, h)) if w > 1 && h > 1 => {
                face_w = w;
                face_h = h;
                faces.push(data);
            }
            _ => {
                log::info!("Panorama textures not available, skipping");
                return create_fallback_strip(device, allocator);
            }
        }
    }

    let strip_w = face_w * 6;
    let strip_h = face_h;
    let mut strip_pixels = vec![0u8; (strip_w * strip_h * 4) as usize];

    for (i, face) in faces.iter().enumerate() {
        let x_off = (i as u32) * face_w;
        for y in 0..face_h {
            let dst_start = ((y * strip_w + x_off) * 4) as usize;
            let src_start = (y * face_w * 4) as usize;
            let len = (face_w * 4) as usize;
            strip_pixels[dst_start..dst_start + len]
                .copy_from_slice(&face[src_start..src_start + len]);
        }
    }

    let (image, view, allocation) = util::create_gpu_image(device, allocator, strip_w, strip_h, "panorama_strip");
    let (staging_buffer, staging_allocation) = util::create_staging_buffer(device, allocator, &strip_pixels, "panorama_staging");

    util::upload_image(device, queue, command_pool, staging_buffer, image, strip_w, strip_h);

    let sampler_info = vk::SamplerCreateInfo::default()
        .mag_filter(vk::Filter::LINEAR)
        .min_filter(vk::Filter::LINEAR)
        .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
        .address_mode_u(vk::SamplerAddressMode::REPEAT)
        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE);
    let sampler = unsafe { device.create_sampler(&sampler_info, None) }
        .expect("failed to create panorama sampler");

    log::info!("Panorama strip loaded: {strip_w}x{strip_h}");

    (image, view, sampler, allocation, staging_buffer, staging_allocation, true)
}

fn create_fallback_strip(
    device: &ash::Device,
    allocator: &Arc<Mutex<Allocator>>,
) -> (vk::Image, vk::ImageView, vk::Sampler, Allocation, vk::Buffer, Allocation, bool) {
    let pixels = vec![0u8; 4];
    let (image, view, allocation) = util::create_gpu_image(device, allocator, 1, 1, "panorama_fallback");
    let (staging_buffer, staging_allocation) = util::create_staging_buffer(device, allocator, &pixels, "panorama_fallback_staging");

    let sampler_info = vk::SamplerCreateInfo::default()
        .mag_filter(vk::Filter::LINEAR)
        .min_filter(vk::Filter::LINEAR);
    let sampler = unsafe { device.create_sampler(&sampler_info, None) }
        .expect("failed to create fallback sampler");

    (image, view, sampler, allocation, staging_buffer, staging_allocation, false)
}

fn load_png_rgba(path: &std::path::Path) -> Option<(Vec<u8>, u32, u32)> {
    let file = std::fs::File::open(path).ok()?;
    let decoder = png::Decoder::new(file);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;

    let data = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => {
            let pixels = info.width as usize * info.height as usize;
            let mut rgba = Vec::with_capacity(pixels * 4);
            for chunk in buf[..pixels * 3].chunks_exact(3) {
                rgba.extend_from_slice(chunk);
                rgba.push(255);
            }
            rgba
        }
        _ => return None,
    };

    Some((data, info.width, info.height))
}

fn create_pipeline(
    device: &ash::Device,
    render_pass: vk::RenderPass,
    layout: vk::PipelineLayout,
) -> vk::Pipeline {
    let vert_spv = shader::include_spirv!("panorama.vert.spv");
    let frag_spv = shader::include_spirv!("panorama.frag.spv");

    let vert_module = shader::create_shader_module(device, vert_spv);
    let frag_module = shader::create_shader_module(device, frag_spv);

    let entry = c"main";
    let stages = [
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vert_module)
            .name(entry),
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(frag_module)
            .name(entry),
    ];

    let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();

    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

    let viewport_state = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);

    let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(vk::PolygonMode::FILL)
        .cull_mode(vk::CullModeFlags::NONE)
        .line_width(1.0);

    let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1);

    let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
        .depth_test_enable(false)
        .depth_write_enable(false);

    let blend_attachment = [vk::PipelineColorBlendAttachmentState {
        blend_enable: vk::FALSE,
        color_write_mask: vk::ColorComponentFlags::RGBA,
        ..Default::default()
    }];
    let color_blending =
        vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend_attachment);

    let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic_state =
        vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

    let pipeline_info = [vk::GraphicsPipelineCreateInfo::default()
        .stages(&stages)
        .vertex_input_state(&vertex_input)
        .input_assembly_state(&input_assembly)
        .viewport_state(&viewport_state)
        .rasterization_state(&rasterizer)
        .multisample_state(&multisampling)
        .depth_stencil_state(&depth_stencil)
        .color_blend_state(&color_blending)
        .dynamic_state(&dynamic_state)
        .layout(layout)
        .render_pass(render_pass)
        .subpass(0)];

    let pipeline = unsafe {
        device.create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_info, None)
    }
    .expect("failed to create panorama pipeline")[0];

    unsafe {
        device.destroy_shader_module(vert_module, None);
        device.destroy_shader_module(frag_module, None);
    }

    pipeline
}
