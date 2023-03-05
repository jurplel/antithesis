use ash::{vk, extensions::ext::DebugUtils};
use winit::{event_loop::EventLoop, window::{WindowBuilder, Window}};
use raw_window_handle::{HasRawDisplayHandle};

use std::{ffi::CStr, os::raw::c_char};

fn main() {
    init_vulkan();
}

fn create_window(width: u32, height: u32, title: &str) -> Window {
    let event_loop = EventLoop::new();
    WindowBuilder::new()
        .with_title(title)
        .with_inner_size(winit::dpi::LogicalSize::new(width, height))
        .build(&event_loop)
        .unwrap()
}

fn init_vulkan() {
    let window = create_window(1280, 720, "Antithesis");
    let instance = create_instance(&window);

}

fn create_instance(window: &Window) -> ash::Instance {
    let app_name = CStr::from_bytes_with_nul(b"Demo\0").unwrap();
    let engine_name = CStr::from_bytes_with_nul(b"Antithesis\0").unwrap();
    let app_info = vk::ApplicationInfo::builder()
        .application_name(app_name)
        .application_version(1)
        .engine_name(engine_name)
        .engine_version(1)
        .api_version(vk::make_api_version(0, 1, 0, 0));
 
    let layer_names = [CStr::from_bytes_with_nul(
        b"VK_LAYER_KHRONOS_validation\0",
    ).unwrap()];
    let layers_names_raw: Vec<*const c_char> = layer_names
        .iter()
        .map(|raw_name| raw_name.as_ptr())
        .collect();

    let mut extension_names =
        ash_window::enumerate_required_extensions(window.raw_display_handle())
            .unwrap()
            .to_vec();
    extension_names.push(DebugUtils::name().as_ptr());

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        extension_names.push(KhrPortabilityEnumerationFn::name().as_ptr());
        // Enabling this extension is a requirement when using `VK_KHR_portability_subset`
        extension_names.push(KhrGetPhysicalDeviceProperties2Fn::name().as_ptr());
    }



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

    let entry = ash::Entry::linked();

    // TODO: make sure this doesn't break everything
    unsafe {
        return entry
            .create_instance(&create_info, None)
            .expect("Instance creation error")
    }
}
