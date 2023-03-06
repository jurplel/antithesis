use ash::{
    extensions::khr::{Surface, Swapchain},
    vk::{self, ApplicationInfo},
    Entry, Instance,
};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::{
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

use std::{ffi::CStr, os::raw::c_char};

fn main() {
    VulkanApp::initialize(1280, 720);
}

fn create_window(width: u32, height: u32, title: &str) -> Window {
    let event_loop = EventLoop::new();
    WindowBuilder::new()
        .with_title(title)
        .with_inner_size(winit::dpi::LogicalSize::new(width, height))
        .build(&event_loop)
        .unwrap()
}

struct VulkanApp {
    window: Window,
    entry: Entry,
    instance: Instance,
    device: ash::Device, // Logical device
    gfx_queue: vk::Queue
}

impl VulkanApp {
    fn initialize(width: u32, height: u32) -> Self {
        let window = create_window(width, height, "Antithesis");

        // Load vulkan through linking
        let entry = ash::Entry::linked();

        // Make instance
        let instance = create_instance(&window, &entry);
       
        // Create window surface and other surface thing
        let (surface, surface_loader) = create_window_surface(&window, &entry, &instance);

        // Get physical device, logical device, and gfx queue
        let (device, gfx_queue) = unsafe { get_device(&instance, &surface_loader, &surface) };


        VulkanApp { window, entry, instance, device, gfx_queue }
    }
}

fn create_window_surface(window: &Window, entry: &Entry, instance: &Instance) -> (vk::SurfaceKHR, Surface) {
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

// fn is_physical_device_suitable(
//     instance: &Instance,
//     physical_device: vk::PhysicalDevice,
// ) -> bool {
//     // todo: replace crappy find_map logic below
//     false
// }

// todo: split to physical & logical device construction
// todo: isolate unsafe blocks instead of making this fn unsafe
unsafe fn get_device(
    instance: &Instance,
    surface_loader: &Surface,
    surface: &vk::SurfaceKHR,
) -> (ash::Device, vk::Queue) {
    // Physical device construction
    let pdevices = instance
        .enumerate_physical_devices()
        .expect("Physical device error");

    // todo: replace with proper suitability checks
    // also todo: separate queue family acquisition to another function
    // todo: perform a check for swapchain extension support here, even though its required for
    // presentation support
    // This currently only separates grabs a queue with graphical ability, hence .contains(GRAPHICS)

    // Select the first physical device that matches the requirements
    let (pdevice, queue_family_index) = pdevices
        .iter()
        .find_map(|pdevice| {
            // Go through all properties and check if... some
            instance
                .get_physical_device_queue_family_properties(*pdevice)
                .iter()
                .enumerate()
                .find_map(|(index, info)| {
                    let supports_graphic_and_surface = info
                        .queue_flags
                        .contains(vk::QueueFlags::GRAPHICS)
                        && surface_loader
                            .get_physical_device_surface_support(*pdevice, index as u32, *surface)
                            .unwrap();
                    if supports_graphic_and_surface {
                        Some((*pdevice, index))
                    } else {
                        None
                    }
                })
        })
        .expect("Couldn't find suitable physical device.");


    // Logical device construction
    // Single queue with priority 1, supporting graphics as found above
    let queue_info = vk::DeviceQueueCreateInfo::builder()
        .queue_family_index(queue_family_index as u32)
        .queue_priorities(&[1.0]);

    // enable swapchain extension here (possibly unchecked?)
    let device_extension_names_raw = [Swapchain::name().as_ptr()];

    // Info for creating the device with enabled extensions and queue info
    let device_create_info = vk::DeviceCreateInfo::builder()
        .queue_create_infos(std::slice::from_ref(&queue_info))
        .enabled_extension_names(&device_extension_names_raw);

    // Create the physical device!
    let device: ash::Device = instance
        .create_device(pdevice, &device_create_info, None)
        .unwrap();
    
    // Queue construction
    let queue = device.get_device_queue(queue_family_index.try_into().unwrap(), 0);

    (device, queue)
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
