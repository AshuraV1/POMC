use std::sync::{Arc, Mutex};

use ash::vk;
use gpu_allocator::vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator};
use gpu_allocator::MemoryLocation;

pub fn create_gpu_image(
    device: &ash::Device,
    allocator: &Arc<Mutex<Allocator>>,
    width: u32,
    height: u32,
    name: &str,
) -> (vk::Image, vk::ImageView, Allocation) {
    let image_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(vk::Format::R8G8B8A8_SRGB)
        .extent(vk::Extent3D { width, height, depth: 1 })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED);

    let image = unsafe { device.create_image(&image_info, None) }
        .expect("failed to create image");
    let mem_reqs = unsafe { device.get_image_memory_requirements(image) };

    let allocation = allocator
        .lock()
        .unwrap()
        .allocate(&AllocationCreateDesc {
            name,
            requirements: mem_reqs,
            location: MemoryLocation::GpuOnly,
            linear: false,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })
        .expect("failed to allocate image memory");

    unsafe {
        device
            .bind_image_memory(image, allocation.memory(), allocation.offset())
            .expect("failed to bind image memory");
    }

    let view_info = vk::ImageViewCreateInfo::default()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(vk::Format::R8G8B8A8_SRGB)
        .subresource_range(COLOR_SUBRESOURCE_RANGE);
    let view = unsafe { device.create_image_view(&view_info, None) }
        .expect("failed to create image view");

    (image, view, allocation)
}

pub fn create_staging_buffer(
    device: &ash::Device,
    allocator: &Arc<Mutex<Allocator>>,
    data: &[u8],
    name: &str,
) -> (vk::Buffer, Allocation) {
    let buffer_info = vk::BufferCreateInfo::default()
        .size(data.len() as u64)
        .usage(vk::BufferUsageFlags::TRANSFER_SRC)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);

    let buffer = unsafe { device.create_buffer(&buffer_info, None) }
        .expect("failed to create staging buffer");
    let mem_reqs = unsafe { device.get_buffer_memory_requirements(buffer) };

    let mut allocation = allocator
        .lock()
        .unwrap()
        .allocate(&AllocationCreateDesc {
            name,
            requirements: mem_reqs,
            location: MemoryLocation::CpuToGpu,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })
        .expect("failed to allocate staging memory");

    unsafe {
        device
            .bind_buffer_memory(buffer, allocation.memory(), allocation.offset())
            .expect("failed to bind staging memory");
    }

    allocation.mapped_slice_mut().unwrap()[..data.len()].copy_from_slice(data);

    (buffer, allocation)
}

pub fn upload_image(
    device: &ash::Device,
    queue: vk::Queue,
    command_pool: vk::CommandPool,
    staging_buffer: vk::Buffer,
    image: vk::Image,
    width: u32,
    height: u32,
) {
    let alloc_info = vk::CommandBufferAllocateInfo::default()
        .command_pool(command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(1);
    let cmd = unsafe { device.allocate_command_buffers(&alloc_info) }
        .expect("failed to allocate upload command buffer")[0];

    let begin_info =
        vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    unsafe { device.begin_command_buffer(cmd, &begin_info) }
        .expect("failed to begin command buffer");

    let barrier_to_transfer = vk::ImageMemoryBarrier::default()
        .image(image)
        .old_layout(vk::ImageLayout::UNDEFINED)
        .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
        .src_access_mask(vk::AccessFlags::empty())
        .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
        .subresource_range(COLOR_SUBRESOURCE_RANGE);

    unsafe {
        device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier_to_transfer],
        );
    }

    let copy_region = vk::BufferImageCopy {
        buffer_offset: 0,
        buffer_row_length: 0,
        buffer_image_height: 0,
        image_subresource: vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        },
        image_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
        image_extent: vk::Extent3D { width, height, depth: 1 },
    };

    unsafe {
        device.cmd_copy_buffer_to_image(
            cmd,
            staging_buffer,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[copy_region],
        );
    }

    let barrier_to_shader = vk::ImageMemoryBarrier::default()
        .image(image)
        .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
        .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
        .dst_access_mask(vk::AccessFlags::SHADER_READ)
        .subresource_range(COLOR_SUBRESOURCE_RANGE);

    unsafe {
        device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier_to_shader],
        );

        device.end_command_buffer(cmd).expect("failed to end command buffer");

        let cmd_buffers = [cmd];
        let submit_info = vk::SubmitInfo::default().command_buffers(&cmd_buffers);
        device
            .queue_submit(queue, &[submit_info], vk::Fence::null())
            .expect("failed to submit upload");
        device.queue_wait_idle(queue).expect("failed to wait for upload");
        device.free_command_buffers(command_pool, &[cmd]);
    }
}

pub fn create_descriptor_set_layout(
    device: &ash::Device,
    descriptor_type: vk::DescriptorType,
    stage_flags: vk::ShaderStageFlags,
) -> vk::DescriptorSetLayout {
    let bindings = [vk::DescriptorSetLayoutBinding {
        binding: 0,
        descriptor_type,
        descriptor_count: 1,
        stage_flags,
        ..Default::default()
    }];
    let info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
    unsafe { device.create_descriptor_set_layout(&info, None) }
        .expect("failed to create descriptor set layout")
}

const COLOR_SUBRESOURCE_RANGE: vk::ImageSubresourceRange = vk::ImageSubresourceRange {
    aspect_mask: vk::ImageAspectFlags::COLOR,
    base_mip_level: 0,
    level_count: 1,
    base_array_layer: 0,
    layer_count: 1,
};
