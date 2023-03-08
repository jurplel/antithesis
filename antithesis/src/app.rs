
use crate::{swapchain::{create_swapchain, SwapchainInfo}, device::{pick_physical_device, create_logical_device, QueueFamilyIndices}};

use ash::{
    extensions::khr::Surface,
    vk::{self, ApplicationInfo},
    Entry, Instance, util::read_spv,
};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::{
    event_loop::{EventLoop, ControlFlow},
    window::{Window, WindowBuilder}, event::{Event, WindowEvent},
};

use std::{ffi::{CStr, CString}, os::raw::c_char, io::Cursor};

struct VulkanApp {
    window: Window,
    entry: Entry,
    instance: Instance,
    physical_device: vk::PhysicalDevice,
    device: ash::Device, // Logical device
   
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,

    swapchain_info: SwapchainInfo,
    gfx_pipeline: vk::Pipeline,
    swapchain_framebuffers: Vec<vk::Framebuffer>,

    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,

    image_available_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,
    current_frame: usize,
}

impl VulkanApp {
    fn initialize(window: Window) -> Self {

        // Load vulkan through linking
        let entry = ash::Entry::linked();

        // Make instance
        let instance = create_instance(&window, &entry);
       
        // Create surface and other surface thing
        let surface_info = SurfaceInfo::create(&window, &entry, &instance);

        // Get physical device, logical device, and gfx queue
        let physical_device = pick_physical_device(&instance, &surface_info);

        let (device, queue_families) = create_logical_device(&instance, &physical_device, &surface_info);

        let graphics_queue =
            unsafe { device.get_device_queue(queue_families.graphics_family.unwrap(), 0) };
        let present_queue =
            unsafe { device.get_device_queue(queue_families.present_family.unwrap(), 0) };

        let swapchain_info = create_swapchain(&instance, &device, &physical_device, &surface_info);

        let render_pass = create_render_pass(&device, &swapchain_info.swapchain_format);

        let gfx_pipeline = create_gfx_pipeline(&device, render_pass, &swapchain_info.swapchain_extent);

        let swapchain_framebuffers = create_framebuffers(&device, render_pass, &swapchain_info.swapchain_imageviews, &swapchain_info.swapchain_extent);

        let command_pool = create_command_pool(&device, &queue_families);

        let command_buffers = create_command_buffers(&device, command_pool, gfx_pipeline, &swapchain_framebuffers, render_pass, swapchain_info.swapchain_extent);

        let sync_objects = create_sync_objects(&device);

        VulkanApp { window, entry, instance, physical_device, device, graphics_queue, present_queue, swapchain_info, gfx_pipeline, swapchain_framebuffers, command_pool, command_buffers, image_available_semaphores: sync_objects.image_available_semaphores, render_finished_semaphores: sync_objects.render_finished_semaphores, in_flight_fences: sync_objects.inflight_fences, current_frame: 0 }
    }

    fn draw_frame(&mut self) {
        let wait_fences = [self.in_flight_fences[self.current_frame]];

        let (image_index, _is_sub_optimal) = unsafe {
            self.device
                .wait_for_fences(&wait_fences, true, std::u64::MAX)
                .expect("Failed to wait for Fence!");

            self.swapchain_info.swapchain_loader
                .acquire_next_image(
                    self.swapchain_info.swapchain,
                    std::u64::MAX,
                    self.image_available_semaphores[self.current_frame],
                    vk::Fence::null(),
                )
                .expect("Failed to acquire next image.")
        };

        let wait_semaphores = [self.image_available_semaphores[self.current_frame]];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let signal_semaphores = [self.render_finished_semaphores[self.current_frame]];

        let submit_infos = [
            *vk::SubmitInfo::builder()
            .wait_semaphores(&wait_semaphores)
            .command_buffers(&self.command_buffers)
            .signal_semaphores(&signal_semaphores)
            .wait_dst_stage_mask(&wait_stages)
        ];

        unsafe {
            self.device
                .reset_fences(&wait_fences)
                .expect("Failed to reset Fence!");

            self.device
                .queue_submit(
                    self.graphics_queue,
                    &submit_infos,
                    self.in_flight_fences[self.current_frame],
                )
                .expect("Failed to execute queue submit.");
        }

        let swapchains = [self.swapchain_info.swapchain];

        let image_indices = [image_index];
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(&signal_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        unsafe {
            self.swapchain_info.swapchain_loader
                .queue_present(self.present_queue, &present_info)
                .expect("Failed to execute queue present.");
        }

        self.current_frame = (self.current_frame + 1) % MAX_FRAMES_IN_FLIGHT;
    }

    fn run(mut self, event_loop: EventLoop<()>) {
        event_loop.run(move |event, _, control_flow| {
            match event {
                | Event::WindowEvent { event, .. } => {
                    match event {
                        | WindowEvent::CloseRequested => {
                            *control_flow = ControlFlow::Exit
                        },
                        | _ => {},
                    }
                },
                | Event::MainEventsCleared => {
                    self.window.request_redraw();
                },
                | Event::RedrawRequested(_window_id) => {
                    self.draw_frame();
                },
                | Event::LoopDestroyed => {
                    unsafe {
                        self.device.device_wait_idle()
                            .expect("Failed to wait device idle!")
                    };
                },
                _ => (),
            }

        })
    }
}

// impl Drop for VulkanApp {
//     fn drop(&mut self) {
//         unsafe {
//             self.swapchain
//         }    
//     }
// }

pub fn run_app() {
    let (event_loop, window) = create_window(1280, 720, "Antithesis");

    let app = VulkanApp::initialize(window);
    app.run(event_loop);
}

const MAX_FRAMES_IN_FLIGHT: usize = 2;

struct SyncObjects {
    image_available_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,
    inflight_fences: Vec<vk::Fence>,
}

fn create_sync_objects(device: &ash::Device) -> SyncObjects {
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


fn create_command_buffers(
    device: &ash::Device,
    command_pool: vk::CommandPool,
    graphics_pipeline: vk::Pipeline,
    framebuffers: &Vec<vk::Framebuffer>,
    render_pass: vk::RenderPass,
    surface_extent: vk::Extent2D,
) -> Vec<vk::CommandBuffer> {
    let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
        .command_pool(command_pool)
        .command_buffer_count(framebuffers.len() as u32)
        .level(vk::CommandBufferLevel::PRIMARY);

    let command_buffers = unsafe {
        device.allocate_command_buffers(&command_buffer_allocate_info)
        .expect("Failed to allocate command buffers!")
    };

    for (i, &command_buffer) in command_buffers.iter().enumerate() {
        let command_buffer_begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::SIMULTANEOUS_USE);

        unsafe {
            device.begin_command_buffer(command_buffer, &command_buffer_begin_info)
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
            .render_area(*vk::Rect2D::builder().offset(*vk::Offset2D::builder()).extent(surface_extent))
            .clear_values(&clear_values);

        unsafe {
            device.cmd_begin_render_pass(command_buffer, &render_pass_begin_info, vk::SubpassContents::INLINE);
            device.cmd_bind_pipeline(command_buffer, vk::PipelineBindPoint::GRAPHICS, graphics_pipeline);
            device.cmd_draw(command_buffer, 3, 1, 0, 0);
            device.cmd_end_render_pass(command_buffer);

            device.end_command_buffer(command_buffer).expect("Failed to record command buffer at ending!");
        }
    };

    command_buffers
}

fn create_command_pool(
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

fn create_render_pass(device: &ash::Device, surface_format: &vk::Format) -> vk::RenderPass {
    let color_attachment = vk::AttachmentDescription::builder()
        .format(*surface_format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

    let color_attachment_ref = [
        *vk::AttachmentReference::builder()
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
    ];

    let render_pass_attachments = [*color_attachment];


    let subpasses = [
        *vk::SubpassDescription::builder()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachment_ref)
    ];

    let subpass_dependencies = [
        *vk::SubpassDependency::builder()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
    ];

    let render_pass_create_info = vk::RenderPassCreateInfo::builder()
        .attachments(&render_pass_attachments)
        .subpasses(&subpasses)
        .dependencies(&subpass_dependencies);

    unsafe {
        device
            .create_render_pass(&render_pass_create_info, None)
            .expect("Failed to create render pass!")
    }
}

fn create_framebuffers(
    device: &ash::Device,
    render_pass: vk::RenderPass,
    image_views: &Vec<vk::ImageView>,
    swapchain_extent: &vk::Extent2D,
) -> Vec<vk::Framebuffer> {
    let mut framebuffers = vec![];

    for &image_view in image_views.iter() {
        let attachments = [image_view];

        let framebuffer_create_info = vk::FramebufferCreateInfo::builder()
            .render_pass(render_pass)
            .attachments(&attachments)
            .width(swapchain_extent.width)
            .height(swapchain_extent.height)
            .layers(1);

        let framebuffer = unsafe {
            device
                .create_framebuffer(&framebuffer_create_info, None)
                .expect("Failed to create Framebuffer!")
        };

        framebuffers.push(framebuffer);
    }

    framebuffers
}

fn create_gfx_pipeline(device: &ash::Device, render_pass: vk::RenderPass, swapchain_extent: &vk::Extent2D) -> vk::Pipeline {
    let mut vert_file = Cursor::new(&include_bytes!("../shaders/vert.spv"));
    let mut frag_file = Cursor::new(&include_bytes!("../shaders/frag.spv"));

    let vert_shader = create_shader_module(&device, &mut vert_file);
    let frag_shader = create_shader_module(&device, &mut frag_file);

    let main_function_name = CString::new("main").unwrap(); // the beginning function name in shader code.
    let shader_stages = [
        *vk::PipelineShaderStageCreateInfo::builder()
            .module(vert_shader)
            .name(&main_function_name)
            .stage(vk::ShaderStageFlags::VERTEX),
        *vk::PipelineShaderStageCreateInfo::builder()
            .module(frag_shader)
            .name(&main_function_name)
            .stage(vk::ShaderStageFlags::FRAGMENT)
    ];

    let vertex_input_state_create_info = vk::PipelineVertexInputStateCreateInfo::builder();

    let input_assembly_state_info = vk::PipelineInputAssemblyStateCreateInfo::builder()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

    let viewports = [
        *vk::Viewport::builder()
            .x(0.0)
            .y(0.0)
            .width(swapchain_extent.width as f32)
            .height(swapchain_extent.height as f32)
            .min_depth(0.0)
            .max_depth(1.0)
    ];

    let scissors = [*vk::Rect2D::builder().offset(*vk::Offset2D::builder()).extent(*swapchain_extent)];

    let viewport_state_create_info = vk::PipelineViewportStateCreateInfo::builder()
        .scissors(&scissors)
        .viewports(&viewports);

    // Maybe shouldn't be default!
    let rasterization_state_create_info = vk::PipelineRasterizationStateCreateInfo::builder()
        .cull_mode(vk::CullModeFlags::BACK)
        .front_face(vk::FrontFace::CLOCKWISE)
        .line_width(1.0)
        .polygon_mode(vk::PolygonMode::FILL);

    // Maybe also shouldn't be default
    let multisample_state_create_info = vk::PipelineMultisampleStateCreateInfo::builder()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1);

    let stencil_state = vk::StencilOpState::builder()
        .fail_op(vk::StencilOp::KEEP)
        .pass_op(vk::StencilOp::KEEP)
        .depth_fail_op(vk::StencilOp::KEEP)
        .compare_op(vk::CompareOp::ALWAYS);

    let depth_stencil_state_create_info = vk::PipelineDepthStencilStateCreateInfo::builder()
        .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL)
        .front(*stencil_state)
        .back(*stencil_state)
        .max_depth_bounds(1.0)
        .min_depth_bounds(0.0);


    let color_blend_attachment_states = [
        *vk::PipelineColorBlendAttachmentState::builder()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .src_color_blend_factor(vk::BlendFactor::SRC_COLOR)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_DST_COLOR)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ZERO)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(vk::ColorComponentFlags::RGBA)
    ];

    let color_blend_state_create_info = vk::PipelineColorBlendStateCreateInfo::builder()
        .logic_op(vk::LogicOp::COPY)
        .attachments(&color_blend_attachment_states);

    let pipeline_layout_create_info = vk::PipelineLayoutCreateInfo::builder();

    let pipeline_layout = unsafe {
        device
            .create_pipeline_layout(&pipeline_layout_create_info, None)
            .expect("Failed to create pipeline layout!")
    };

    // dynamic state not included for now
    let graphics_pipeline_create_info = [
        *vk::GraphicsPipelineCreateInfo::builder()
            .stages(&shader_stages)
            .vertex_input_state(&vertex_input_state_create_info)
            .input_assembly_state(&input_assembly_state_info)
            .viewport_state(&viewport_state_create_info)
            .rasterization_state(&rasterization_state_create_info)
            .multisample_state(&multisample_state_create_info)
            .depth_stencil_state(&depth_stencil_state_create_info)
            .color_blend_state(&color_blend_state_create_info)
            .layout(pipeline_layout)
            .render_pass(render_pass)
    ];

    let graphics_pipeline = unsafe {
        device
            .create_graphics_pipelines(vk::PipelineCache::null(),&graphics_pipeline_create_info, None)
            .expect("Failed to create graphics pipeline!")
    };

    unsafe {
        device.destroy_shader_module(vert_shader, None);
        device.destroy_shader_module(frag_shader, None);
    }

    graphics_pipeline[0]
}

fn create_shader_module(device: &ash::Device, file: &mut (impl std::io::Seek + std::io::Read)) -> vk::ShaderModule {
    let code = read_spv(file).unwrap();
    
    let create_info = vk::ShaderModuleCreateInfo::builder()
        .code(&code);

    unsafe {
        device.create_shader_module(&create_info, None).expect("Failed to create shader module!")
    }
}


fn create_window(width: u32, height: u32, title: &str) -> (EventLoop<()>, Window) {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(title)
        .with_inner_size(winit::dpi::LogicalSize::new(width, height))
        .build(&event_loop)
        .unwrap();

    (event_loop, window)
}

pub struct SurfaceInfo {
    pub surface: vk::SurfaceKHR,
    pub surface_loader: Surface
}

impl SurfaceInfo {
    pub fn create(window: &Window, entry: &Entry, instance: &Instance) -> Self {
        let surface = unsafe {
            ash_window::create_surface(
                &entry,
                &instance,
                window.raw_display_handle(),
                window.raw_window_handle(),
                None,
            )
            .unwrap() };

        // What is this actually? How is this different from the surfaceKHR above?
        let surface_loader = Surface::new(&entry, &instance);

        SurfaceInfo { surface, surface_loader }
    }
}


fn create_instance(window: &Window, entry: &Entry) -> Instance {
    let app_name = CStr::from_bytes_with_nul(b"Demo\0").unwrap();
    let engine_name = CStr::from_bytes_with_nul(b"Antithesis\0").unwrap();
    let app_info = ApplicationInfo::builder()
        .application_name(app_name)
        .application_version(1)
        .engine_name(engine_name)
        .engine_version(1)
        .api_version(vk::make_api_version(0, 1, 0, 0));

    let layer_names = [CStr::from_bytes_with_nul(b"VK_LAYER_KHRONOS_validation\0").unwrap()];
    let layers_names_raw: Vec<*const c_char> = layer_names
        .iter()
        .map(|raw_name| raw_name.as_ptr())
        .collect();

    // required extensions to support the passed window
    let extension_names = ash_window::enumerate_required_extensions(window.raw_display_handle())
        .unwrap()
        .to_vec();

    let create_flags = if cfg!(any(target_os = "macos", target_os = "ios")) {
        vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR
    } else {
        vk::InstanceCreateFlags::default()
    };

    let create_info = vk::InstanceCreateInfo::builder()
        .application_info(&app_info)
        .enabled_layer_names(&layers_names_raw)
        .enabled_extension_names(&extension_names)
        .flags(create_flags);

    unsafe {
        return entry
            .create_instance(&create_info, None)
            .expect("Instance creation error");
    }
}
