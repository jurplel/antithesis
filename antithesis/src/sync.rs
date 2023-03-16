use crate::device::QueueFamilyIndices;

use ash::vk;

pub const MAX_FRAMES_IN_FLIGHT: usize = 2;

pub struct SyncObjects {
    pub image_available_semaphores: Vec<vk::Semaphore>,
    pub render_finished_semaphores: Vec<vk::Semaphore>,
    pub inflight_fences: Vec<vk::Fence>,
}

pub fn create_sync_objects(device: &ash::Device) -> SyncObjects {
    let mut sync_objects = SyncObjects {
        image_available_semaphores: vec![],
        render_finished_semaphores: vec![],
        inflight_fences: vec![],
    };

    let semaphore_create_info = vk::SemaphoreCreateInfo::builder();

    let fence_create_info = vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED);

    for _ in 0..MAX_FRAMES_IN_FLIGHT {
        unsafe {
            let image_available_semaphore = device
                .create_semaphore(&semaphore_create_info, None)
                .expect("Failed to create Semaphore Object!");
            let render_finished_semaphore = device
                .create_semaphore(&semaphore_create_info, None)
                .expect("Failed to create Semaphore Object!");
            let inflight_fence = device
                .create_fence(&fence_create_info, None)
                .expect("Failed to create Fence Object!");

            sync_objects
                .image_available_semaphores
                .push(image_available_semaphore);
            sync_objects
                .render_finished_semaphores
                .push(render_finished_semaphore);
            sync_objects.inflight_fences.push(inflight_fence);
        }
    }

    sync_objects
}

pub fn create_command_buffers(
    device: &ash::Device,
    command_pool: vk::CommandPool,
    gfx_pipeline: vk::Pipeline,
    framebuffers: &Vec<vk::Framebuffer>,
    render_pass: vk::RenderPass,
    surface_extent: vk::Extent2D,
    vertex_buffer: vk::Buffer,
) -> Vec<vk::CommandBuffer> {
    let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
        .command_pool(command_pool)
        .command_buffer_count(framebuffers.len() as u32)
        .level(vk::CommandBufferLevel::PRIMARY);

    let command_buffers = unsafe {
        device
            .allocate_command_buffers(&command_buffer_allocate_info)
            .expect("Failed to allocate command buffers!")
    };

    for (i, &command_buffer) in command_buffers.iter().enumerate() {
        let command_buffer_begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::SIMULTANEOUS_USE);

        unsafe {
            device
                .begin_command_buffer(command_buffer, &command_buffer_begin_info)
                .expect("Failed to begin recording command buffer at beginning!");
        };

        let clear_values = [vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.0, 0.0, 0.0, 1.0],
            },
        }];

        let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
            .render_pass(render_pass)
            .framebuffer(framebuffers[i])
            .render_area(
                *vk::Rect2D::builder()
                    .offset(*vk::Offset2D::builder())
                    .extent(surface_extent),
            )
            .clear_values(&clear_values);

        unsafe {
            device.cmd_begin_render_pass(
                command_buffer,
                &render_pass_begin_info,
                vk::SubpassContents::INLINE,
            );
            device.cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                gfx_pipeline,
            );

            let vertex_buffers = [vertex_buffer];
            let offsets = [0_u64];

            device.cmd_bind_vertex_buffers(command_buffer, 0, &vertex_buffers, &offsets);

            device.cmd_draw(command_buffer, 3, 1, 0, 0);
            device.cmd_end_render_pass(command_buffer);

            device
                .end_command_buffer(command_buffer)
                .expect("Failed to record command buffer at ending!");
        }
    }

    command_buffers
}

pub fn create_command_pool(
    device: &ash::Device,
    queue_families: &QueueFamilyIndices,
) -> vk::CommandPool {
    let command_pool_create_info = vk::CommandPoolCreateInfo::builder()
        .queue_family_index(queue_families.graphics_family.unwrap());

    unsafe {
        device
            .create_command_pool(&command_pool_create_info, None)
            .expect("Failed to create Command Pool!")
    }
}
