
use crate::{device::get_device, swapchain::SwapchainInfo};
use crate::swapchain::create_swapchain;

use ash::vk::PipelineShaderStageCreateInfo;
use ash::{
    extensions::khr::Surface,
    vk::{self, ApplicationInfo},
    Entry, Instance, util::read_spv,
};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::{
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

use std::{ffi::CStr, os::raw::c_char, io::Cursor};

pub struct VulkanApp {
    window: Window,
    entry: Entry,
    instance: Instance,
    physical_device: vk::PhysicalDevice,
    device: ash::Device, // Logical device
    gfx_queue: vk::Queue,
    swapchain_info: SwapchainInfo,
    gfx_pipeline: vk::Pipeline
}

impl VulkanApp {
    pub fn initialize(width: u32, height: u32) -> Self {
        let window = create_window(width, height, "Antithesis");

        // Load vulkan through linking
        let entry = ash::Entry::linked();

        // Make instance
        let instance = create_instance(&window, &entry);
       
        // Create surface and other surface thing
        let (surface, surface_loader) = create_surface(&window, &entry, &instance);

        // Get physical device, logical device, and gfx queue
        let (physical_device, device, gfx_queue) = unsafe { get_device(&instance, &surface_loader, &surface) };

        let swapchain_info = create_swapchain(&instance, &device, &physical_device, &surface, &surface_loader);


        let render_pass = create_render_pass(&device, &swapchain_info.swapchain_format);

        let gfx_pipeline = create_gfx_pipeline(&device, render_pass, &swapchain_info.swapchain_extent);

        VulkanApp { window, entry, instance, physical_device, device, gfx_queue, swapchain_info, gfx_pipeline }
    }
}

// impl Drop for VulkanApp {
//     fn drop(&mut self) {
//         unsafe {
//             self.swapchain
//         }    
//     }
// }

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

fn create_gfx_pipeline(device: &ash::Device, render_pass: vk::RenderPass, swapchain_extent: &vk::Extent2D) -> vk::Pipeline {
    let mut vert_file = Cursor::new(&include_bytes!("../shaders/vert.spv"));
    let mut frag_file = Cursor::new(&include_bytes!("../shaders/frag.spv"));

    let vert_shader = create_shader_module(&device, &mut vert_file);
    let frag_shader = create_shader_module(&device, &mut frag_file);

    // Might need main function name here
    let shader_stages = [
        *vk::PipelineShaderStageCreateInfo::builder()
            .module(vert_shader)
            .stage(vk::ShaderStageFlags::VERTEX),
        *vk::PipelineShaderStageCreateInfo::builder()
            .module(frag_shader)
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
    let graphics_pipeline_create_infos = [
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
            .create_graphics_pipelines(vk::PipelineCache::null(),&graphics_pipeline_create_infos, None)
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


fn create_window(width: u32, height: u32, title: &str) -> Window {
    let event_loop = EventLoop::new();
    WindowBuilder::new()
        .with_title(title)
        .with_inner_size(winit::dpi::LogicalSize::new(width, height))
        .build(&event_loop)
        .unwrap()
}


fn create_surface(window: &Window, entry: &Entry, instance: &Instance) -> (vk::SurfaceKHR, Surface) {
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

    (surface, surface_loader)
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