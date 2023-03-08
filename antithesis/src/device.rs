use std::{collections::HashSet, ffi::{CStr, c_char}};

use ash::{
    vk,
    Instance, extensions::khr::Swapchain,
};

use crate::{app::SurfaceInfo, swapchain::SwapChainSupportDetail};

pub struct QueueFamilyIndices {
    pub graphics_family: Option<u32>,
    pub present_family: Option<u32>,
}

impl QueueFamilyIndices {
    pub fn new() -> QueueFamilyIndices {
        QueueFamilyIndices {
            graphics_family: None,
            present_family: None,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.graphics_family.is_some() && self.present_family.is_some()
    }
}


fn find_queue_family(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    surface_info: &SurfaceInfo,
) -> QueueFamilyIndices {
    let queue_families =
        unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

    let mut queue_family_indices = QueueFamilyIndices::new();

    let mut index = 0;
    for queue_family in queue_families.iter() {
        if queue_family.queue_count > 0
            && queue_family.queue_flags.contains(vk::QueueFlags::GRAPHICS)
        {
            queue_family_indices.graphics_family = Some(index);
        }

        let is_present_support = unsafe {
            surface_info
                .surface_loader
                .get_physical_device_surface_support(
                    physical_device,
                    index as u32,
                    surface_info.surface,
                ).unwrap()
        };

        if queue_family.queue_count > 0 && is_present_support {
            queue_family_indices.present_family = Some(index);
        }

        if queue_family_indices.is_complete() {
            break;
        }

        index += 1;
    }

    queue_family_indices
}

pub struct DeviceExtension {
    pub names: [&'static str; 1],
    //    pub raw_names: [*const i8; 1],
}

const DEVICE_EXTENSIONS: DeviceExtension = DeviceExtension {
    names: ["VK_KHR_swapchain"],
};

/// Helper function to convert [c_char; SIZE] to string
pub fn vk_to_string(raw_string_array: &[c_char]) -> String {
    // Implementation 2
    let raw_string = unsafe {
        let pointer = raw_string_array.as_ptr();
        CStr::from_ptr(pointer)
    };

    raw_string
        .to_str()
        .expect("Failed to convert vulkan raw string.")
        .to_owned()
}

fn check_device_extension_support(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
) -> bool {
    let available_extensions = unsafe {
        instance
            .enumerate_device_extension_properties(physical_device)
            .expect("Failed to get device extension properties.")
    };

    let mut available_extension_names = vec![];

    println!("\tAvailable Device Extensions: ");
    for extension in available_extensions.iter() {
        let extension_name: String = vk_to_string(&extension.extension_name);
        println!(
            "\t\tName: {}, Version: {}",
            extension_name, extension.spec_version
        );

        available_extension_names.push(extension_name);
    }

    let mut required_extensions = HashSet::new();
    for extension in DEVICE_EXTENSIONS.names.iter() {
        required_extensions.insert(extension.to_string());
    }

    for extension_name in available_extension_names.iter() {
        required_extensions.remove(extension_name);
    }

    return required_extensions.is_empty();
}

fn is_physical_device_suitable(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    surface_info: &SurfaceInfo,
) -> bool {
    let _device_features = unsafe { instance.get_physical_device_features(physical_device) };

    let indices = find_queue_family(instance, physical_device, surface_info);

    let is_queue_family_supported = indices.is_complete();
    let is_device_extension_supported =
        check_device_extension_support(instance, physical_device);
    let is_swapchain_supported = if is_device_extension_supported {
        let swapchain_support = SwapChainSupportDetail::query(&physical_device, surface_info);
        !swapchain_support.formats.is_empty() && !swapchain_support.present_modes.is_empty()
    } else {
        false
    };

    return is_queue_family_supported
        && is_device_extension_supported
        && is_swapchain_supported;
}

// todo: split to physical & logical device construction
// todo: isolate unsafe blocks instead of making this fn unsafe
pub fn pick_physical_device(
    instance: &Instance,
    surface_info: &SurfaceInfo,
) -> vk::PhysicalDevice {
    let physical_devices = unsafe {
        instance
            .enumerate_physical_devices()
            .expect("Failed to enumerate physical devices!")
    };

    let result = physical_devices.iter().find(|physical_device| {
        is_physical_device_suitable(instance, **physical_device, surface_info)
    });

    *result.expect("Failed to find a suitable GPU!")

}

pub fn create_logical_device(
    instance: &ash::Instance,
    physical_device: &vk::PhysicalDevice,
    surface_info: &SurfaceInfo
) -> (ash::Device, QueueFamilyIndices) {
    let indices = find_queue_family(instance, *physical_device, surface_info);

    let mut unique_queue_families = HashSet::new();
    unique_queue_families.insert(indices.graphics_family.unwrap());
    unique_queue_families.insert(indices.present_family.unwrap());

    // Single queue with priority 1, supporting graphics as found above
    let queue_create_infos = unique_queue_families.iter().map(|queue_family| {
        *vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(*queue_family)
            .queue_priorities(&[1.0])
    }).collect::<Vec<_>>();

    // enable swapchain extension here (possibly unchecked?)
    let device_extension_names_raw = [Swapchain::name().as_ptr()];

    // Info for creating the device with enabled extensions and queue info
    let device_create_info = vk::DeviceCreateInfo::builder()
        .queue_create_infos(&queue_create_infos)
        .enabled_extension_names(&device_extension_names_raw);

    // Create the physical device!
    let device: ash::Device = unsafe { instance
        .create_device(*physical_device, &device_create_info, None)
        .expect("Failed to create logical device!") };

    (device, indices)
}
