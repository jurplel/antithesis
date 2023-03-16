use crate::{
    device::{create_logical_device, pick_physical_device},
    pipeline::{create_framebuffers, create_gfx_pipeline, create_render_pass, create_vertex_buffer},
    swapchain::{create_swapchain, SwapchainInfo},
    sync::{
        create_command_buffers, create_command_pool, create_sync_objects, MAX_FRAMES_IN_FLIGHT,
    },
};

use ash::{
    extensions::khr::Surface,
    vk::{self, ApplicationInfo},
    Entry, Instance,
};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

use std::{ffi::CStr, os::raw::c_char};

struct VulkanApp {
    window: Window,
    entry: Entry,
    instance: Instance,
    surface_info: SurfaceInfo,

    physical_device: vk::PhysicalDevice,
    device: ash::Device, // Logical device

    graphics_queue: vk::Queue,
    present_queue: vk::Queue,

    swapchain_info: SwapchainInfo,

    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    gfx_pipeline: vk::Pipeline,
    swapchain_framebuffers: Vec<vk::Framebuffer>,

    vertex_buffer: vk::Buffer,
    vertex_buffer_memory: vk::DeviceMemory,

    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,

    image_available_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,
    current_frame: usize,

    is_framebuffer_resized: bool,
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

        let (device, queue_families) =
            create_logical_device(&instance, &physical_device, &surface_info);

        let graphics_queue =
            unsafe { device.get_device_queue(queue_families.graphics_family.unwrap(), 0) };
        let present_queue =
            unsafe { device.get_device_queue(queue_families.present_family.unwrap(), 0) };

        let swapchain_info = create_swapchain(&instance, &device, &physical_device, &surface_info);

        let render_pass = create_render_pass(&device, &swapchain_info.swapchain_format);

        let (pipeline_layout, gfx_pipeline) =
            create_gfx_pipeline(&device, render_pass, &swapchain_info.swapchain_extent);

        let swapchain_framebuffers = create_framebuffers(
            &device,
            render_pass,
            &swapchain_info.swapchain_imageviews,
            &swapchain_info.swapchain_extent,
        );

        let command_pool = create_command_pool(&device, &queue_families);

        let (vertex_buffer, vertex_buffer_memory) =
            create_vertex_buffer(&device, physical_device, &instance);

        let command_buffers = create_command_buffers(
            &device,
            command_pool,
            gfx_pipeline,
            &swapchain_framebuffers,
            render_pass,
            swapchain_info.swapchain_extent,
            vertex_buffer
        );

        let sync_objects = create_sync_objects(&device);

        VulkanApp {
            window,
            entry,
            instance,
            surface_info,
            physical_device,
            device,
            graphics_queue,
            present_queue,
            swapchain_info,
            render_pass,
            pipeline_layout,
            gfx_pipeline,
            swapchain_framebuffers,
            vertex_buffer,
            vertex_buffer_memory,
            command_pool,
            command_buffers,
            image_available_semaphores: sync_objects.image_available_semaphores,
            render_finished_semaphores: sync_objects.render_finished_semaphores,
            in_flight_fences: sync_objects.inflight_fences,
            current_frame: 0,
            is_framebuffer_resized: false,
        }
    }

    fn draw_frame(&mut self) {
        let wait_fences = [self.in_flight_fences[self.current_frame]];

        let (image_index, _is_sub_optimal) = unsafe {
            self.device
                .wait_for_fences(&wait_fences, true, std::u64::MAX)
                .expect("Failed to wait for Fence!");

            self.swapchain_info
                .swapchain_loader
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

        let submit_infos = [*vk::SubmitInfo::builder()
            .wait_semaphores(&wait_semaphores)
            .command_buffers(&self.command_buffers)
            .signal_semaphores(&signal_semaphores)
            .wait_dst_stage_mask(&wait_stages)];

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

        let result = unsafe {
            self.swapchain_info
                .swapchain_loader
                .queue_present(self.present_queue, &present_info)
        };
        let is_resized = match result {
            Ok(_) => self.is_framebuffer_resized,
            Err(vk_result) => match vk_result {
                vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR => true,
                _ => panic!("Failed to execute queue present."),
            },
        };
        if is_resized {
            self.is_framebuffer_resized = false;
            self.recreate_swapchain();
        }

        self.current_frame = (self.current_frame + 1) % MAX_FRAMES_IN_FLIGHT;
    }

    fn recreate_swapchain(&mut self) {
        unsafe {
            self.device
                .device_wait_idle()
                .expect("Failed to wait device idle!")
        };
        self.cleanup_swapchain();

        let swapchain_info = create_swapchain(
            &self.instance,
            &self.device,
            &self.physical_device,
            &self.surface_info,
        );

        self.swapchain_info = swapchain_info;

        self.render_pass = create_render_pass(&self.device, &self.swapchain_info.swapchain_format);
        (self.pipeline_layout, self.gfx_pipeline) = create_gfx_pipeline(
            &self.device,
            self.render_pass,
            &self.swapchain_info.swapchain_extent,
        );

        self.swapchain_framebuffers = create_framebuffers(
            &self.device,
            self.render_pass,
            &self.swapchain_info.swapchain_imageviews,
            &self.swapchain_info.swapchain_extent,
        );
        self.command_buffers = create_command_buffers(
            &self.device,
            self.command_pool,
            self.gfx_pipeline,
            &self.swapchain_framebuffers,
            self.render_pass,
            self.swapchain_info.swapchain_extent,
            self.vertex_buffer
        );
    }

    fn cleanup_swapchain(&self) {
        unsafe {
            self.device
                .free_command_buffers(self.command_pool, &self.command_buffers);
            for &framebuffer in self.swapchain_framebuffers.iter() {
                self.device.destroy_framebuffer(framebuffer, None);
            }
            self.device.destroy_pipeline(self.gfx_pipeline, None);
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_render_pass(self.render_pass, None);
            for &image_view in self.swapchain_info.swapchain_imageviews.iter() {
                self.device.destroy_image_view(image_view, None);
            }
            self.swapchain_info
                .swapchain_loader
                .destroy_swapchain(self.swapchain_info.swapchain, None);
        }
    }

    fn run(mut self, event_loop: EventLoop<()>) {
        event_loop.run(move |event, _, control_flow| match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                _ => {}
            },
            Event::MainEventsCleared => {
                self.window.request_redraw();
            }
            Event::RedrawRequested(_window_id) => {
                self.draw_frame();
            }
            Event::LoopDestroyed => {
                unsafe {
                    self.device
                        .device_wait_idle()
                        .expect("Failed to wait device idle!")
                };
            }
            _ => (),
        })
    }
}

impl Drop for VulkanApp {
    fn drop(&mut self) {
        unsafe {
            for i in 0..MAX_FRAMES_IN_FLIGHT {
                self.device
                    .destroy_semaphore(self.image_available_semaphores[i], None);
                self.device
                    .destroy_semaphore(self.render_finished_semaphores[i], None);
                self.device.destroy_fence(self.in_flight_fences[i], None);
            }

            self.cleanup_swapchain();

            self.device.destroy_command_pool(self.command_pool, None);

            self.device.destroy_device(None);
            self.surface_info
                .surface_loader
                .destroy_surface(self.surface_info.surface, None);

            self.instance.destroy_instance(None);
        }
    }
}

pub fn run_app() {
    let (event_loop, window) = create_window(1280, 720, "Antithesis");

    let app = VulkanApp::initialize(window);
    app.run(event_loop);
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
    pub surface_loader: Surface,
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
            .unwrap()
        };

        // What is this actually? How is this different from the surfaceKHR above?
        let surface_loader = Surface::new(&entry, &instance);

        SurfaceInfo {
            surface,
            surface_loader,
        }
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
