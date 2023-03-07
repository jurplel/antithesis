use ash::{
    extensions::khr::{Surface, Swapchain},
    vk,
    Instance,
};


// fn is_physical_device_suitable(
//     instance: &Instance,
//     physical_device: vk::PhysicalDevice,
// ) -> bool {
//     // todo: replace crappy find_map logic below
//     false
// }

// todo: split to physical & logical device construction
// todo: isolate unsafe blocks instead of making this fn unsafe
pub unsafe fn get_device(
    instance: &Instance,
    surface_loader: &Surface,
    surface: &vk::SurfaceKHR,
) -> (vk::PhysicalDevice, ash::Device, vk::Queue) {
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

    (pdevice, device, queue)
}

