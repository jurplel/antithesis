use std::{ffi::CString, io::Cursor};

use ash::{
    util::read_spv,
    vk
};
use memoffset::offset_of;

// hardcoded
const VERTICES_DATA: [Vertex; 3] = [
    Vertex {
        pos: [0.0, -0.5],
        color: [1.0, 0.0, 0.0],
    },
    Vertex {
        pos: [0.5, 0.5],
        color: [0.0, 1.0, 0.0],
    },
    Vertex {
        pos: [-0.5, 0.5],
        color: [0.0, 0.0, 1.0],
    },
];

#[repr(C)]
#[derive(Debug, Clone)]
struct Vertex {
    pos: [f32; 2],
    color: [f32; 3],
}

impl Vertex {
    fn get_binding_descriptions() -> [vk::VertexInputBindingDescription; 1] {
        [*vk::VertexInputBindingDescription::builder()
            .binding(0)
            .stride(std::mem::size_of::<Self>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX)]
    }

    fn get_attribute_descriptions() -> [vk::VertexInputAttributeDescription; 2] {
        [
            *vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(0)
                .format(vk::Format::R32G32_SFLOAT)
                .offset(offset_of!(Self, pos) as u32),
            *vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(1)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(offset_of!(Self, color) as u32),
        ]
    }
}

pub fn create_vertex_buffer(
    device: &ash::Device,
    physical_device: vk::PhysicalDevice,
    instance: &ash::Instance,
) -> (vk::Buffer, vk::DeviceMemory) {
    let vertex_buffer_create_info = vk::BufferCreateInfo::builder()
        .size(std::mem::size_of_val(&VERTICES_DATA) as u64)
        .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);

    let vertex_buffer = unsafe {
        device
            .create_buffer(&vertex_buffer_create_info, None)
            .expect("Failed to create the vertex buffer!")
    };

    let mem_requirements = unsafe { device.get_buffer_memory_requirements(vertex_buffer) };
    let mem_properties = unsafe { instance.get_physical_device_memory_properties(physical_device) };
    let required_memory_flags: vk::MemoryPropertyFlags =
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
    let memory_type = find_memory_type(
        mem_requirements.memory_type_bits,
        required_memory_flags,
        mem_properties,
    );

    let allocate_info = vk::MemoryAllocateInfo::builder()
        .allocation_size(mem_requirements.size)
        .memory_type_index(memory_type);

    let vertex_buffer_memory = unsafe {
        device
            .allocate_memory(&allocate_info, None)
            .expect("Failed to allocate vertex buffer memory!")
    };

    unsafe {
        device
            .bind_buffer_memory(vertex_buffer, vertex_buffer_memory, 0)
            .expect("Failed to bind Buffer");

        let data_ptr = device
            .map_memory(
                vertex_buffer_memory,
                0,
                vertex_buffer_create_info.size,
                vk::MemoryMapFlags::empty(),
            )
            .expect("Failed to Map Memory") as *mut Vertex;

        data_ptr.copy_from_nonoverlapping(VERTICES_DATA.as_ptr(), VERTICES_DATA.len());

        device.unmap_memory(vertex_buffer_memory);
    }

    (vertex_buffer, vertex_buffer_memory)
}

fn find_memory_type(
    type_filter: u32,
    required_properties: vk::MemoryPropertyFlags,
    mem_properties: vk::PhysicalDeviceMemoryProperties,
) -> u32 {
    for (i, memory_type) in mem_properties.memory_types.iter().enumerate() {
        //if (type_filter & (1 << i)) > 0 && (memory_type.property_flags & required_properties) == required_properties {
        //    return i as u32
        // }

        // same implementation
        if (type_filter & (1 << i)) > 0 && memory_type.property_flags.contains(required_properties)
        {
            return i as u32;
        }
    }

    panic!("Failed to find suitable memory type!")
}

pub fn create_render_pass(device: &ash::Device, surface_format: &vk::Format) -> vk::RenderPass {
    let color_attachment = vk::AttachmentDescription::builder()
        .format(*surface_format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

    let color_attachment_ref =
        [*vk::AttachmentReference::builder().layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];

    let render_pass_attachments = [*color_attachment];

    let subpasses = [*vk::SubpassDescription::builder()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_attachment_ref)];

    let subpass_dependencies = [*vk::SubpassDependency::builder()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)];

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

pub fn create_gfx_pipeline(
    device: &ash::Device,
    render_pass: vk::RenderPass,
    swapchain_extent: &vk::Extent2D,
) -> (vk::PipelineLayout, vk::Pipeline) {
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
            .stage(vk::ShaderStageFlags::FRAGMENT),
    ];

    let binding_description = Vertex::get_binding_descriptions();
    let attribute_description = Vertex::get_attribute_descriptions();

    let vertex_input_state_create_info = vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_attribute_descriptions(&attribute_description)
        .vertex_binding_descriptions(&binding_description);

    let input_assembly_state_info = vk::PipelineInputAssemblyStateCreateInfo::builder()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

    let viewports = [*vk::Viewport::builder()
        .x(0.0)
        .y(0.0)
        .width(swapchain_extent.width as f32)
        .height(swapchain_extent.height as f32)
        .min_depth(0.0)
        .max_depth(1.0)];

    let scissors = [*vk::Rect2D::builder()
        .offset(*vk::Offset2D::builder())
        .extent(*swapchain_extent)];

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

    let color_blend_attachment_states = [*vk::PipelineColorBlendAttachmentState::builder()
        .color_write_mask(vk::ColorComponentFlags::RGBA)
        .src_color_blend_factor(vk::BlendFactor::SRC_COLOR)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_DST_COLOR)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ZERO)
        .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
        .alpha_blend_op(vk::BlendOp::ADD)
        .color_write_mask(vk::ColorComponentFlags::RGBA)];

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
    let gfx_pipeline_create_info = [*vk::GraphicsPipelineCreateInfo::builder()
        .stages(&shader_stages)
        .vertex_input_state(&vertex_input_state_create_info)
        .input_assembly_state(&input_assembly_state_info)
        .viewport_state(&viewport_state_create_info)
        .rasterization_state(&rasterization_state_create_info)
        .multisample_state(&multisample_state_create_info)
        .depth_stencil_state(&depth_stencil_state_create_info)
        .color_blend_state(&color_blend_state_create_info)
        .layout(pipeline_layout)
        .render_pass(render_pass)];

    let gfx_pipeline = unsafe {
        device
            .create_graphics_pipelines(vk::PipelineCache::null(), &gfx_pipeline_create_info, None)
            .expect("Failed to create graphics pipeline!")
    };

    unsafe {
        device.destroy_shader_module(vert_shader, None);
        device.destroy_shader_module(frag_shader, None);
    }

    (pipeline_layout, gfx_pipeline[0])
}

pub fn create_framebuffers(
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

fn create_shader_module(
    device: &ash::Device,
    file: &mut (impl std::io::Seek + std::io::Read),
) -> vk::ShaderModule {
    let code = read_spv(file).unwrap();

    let create_info = vk::ShaderModuleCreateInfo::builder().code(&code);

    unsafe {
        device
            .create_shader_module(&create_info, None)
            .expect("Failed to create shader module!")
    }
}
