use std::ptr;

use ash::vk;

use crate::app::SurfaceInfo;

pub struct SwapchainInfo {
    pub swapchain_loader: ash::extensions::khr::Swapchain,
    pub swapchain: vk::SwapchainKHR,
    pub swapchain_images: Vec<vk::Image>,
    pub swapchain_format: vk::Format,
    pub swapchain_extent: vk::Extent2D,
    pub swapchain_imageviews: Vec<vk::ImageView>
}

pub struct SwapChainSupportDetail {
    pub capabilities: vk::SurfaceCapabilitiesKHR,
    pub formats: Vec<vk::SurfaceFormatKHR>,
    pub present_modes: Vec<vk::PresentModeKHR>,
}

impl SwapChainSupportDetail {
    pub fn query(
        physical_device: &vk::PhysicalDevice,
        surface_info: &SurfaceInfo,
    ) -> Self {
        unsafe {
            let capabilities = surface_info.surface_loader
                .get_physical_device_surface_capabilities(*physical_device, surface_info.surface)
                .expect("Failed to query for surface capabilities.");
            let formats = surface_info.surface_loader
                .get_physical_device_surface_formats(*physical_device, surface_info.surface)
                .expect("Failed to query for surface formats.");
            let present_modes = surface_info.surface_loader
                .get_physical_device_surface_present_modes(*physical_device, surface_info.surface)
                .expect("Failed to query for surface present mode.");

            SwapChainSupportDetail {
                capabilities,
                formats,
                present_modes,
            }
        }
    }

    fn choose_format(&self) -> vk::SurfaceFormatKHR {
        // check if list contains most widely used R8G8B8A8 format with nonlinear color space
        for available_format in &self.formats {
            if available_format.format == vk::Format::B8G8R8A8_SRGB
                && available_format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            {
                return available_format.clone();
            }
        }

        // return the first format from the list
        return self.formats.first().unwrap().clone();
    }

    fn choose_present_mode(&self) -> vk::PresentModeKHR {
        for &available_present_mode in self.present_modes.iter() {
            // "Triple buffering" mailbox mode if possible
            if available_present_mode == vk::PresentModeKHR::MAILBOX {
                return available_present_mode;
            }
        }

        // fallback to "vertical blank"
        vk::PresentModeKHR::FIFO
    }

    fn choose_extent(&self) -> vk::Extent2D {
        if self.capabilities.current_extent.width != u32::max_value() {
            self.capabilities.current_extent
        } else {
            // TODO: remove hard-coded window size
            vk::Extent2D {
                width: 1280.max(self.capabilities.min_image_extent.width).min(self.capabilities.max_image_extent.width),
                height: 720.max(self.capabilities.min_image_extent.height).min(self.capabilities.max_image_extent.height)
            }
        }
    }
}

pub fn create_swapchain(
    instance: &ash::Instance,
    device: &ash::Device,
    physical_device: &vk::PhysicalDevice,
    surface_info: &SurfaceInfo,
) -> SwapchainInfo {
    let swapchain_support = SwapChainSupportDetail::query(physical_device, surface_info);

    let surface_format = swapchain_support.choose_format(); 

    let present_mode = swapchain_support.choose_present_mode();

    let swapchain_extent = swapchain_support.choose_extent();

    // Just a kinda weird way of getting the image count of the swapchain
    let image_count = swapchain_support.capabilities.min_image_count + 1;
        let image_count = if swapchain_support.capabilities.max_image_count > 0 {
            image_count.min(swapchain_support.capabilities.max_image_count)
        } else {
            image_count
        };

    // let's do it all on one queue for now :(
    let (image_sharing_mode, queue_family_index_count, queue_family_indices) = (vk::SharingMode::EXCLUSIVE, 0, vec![]);

    // let (image_sharing_mode, queue_family_index_count, queue_family_indices) =
    //     if queue_family.graphics_family != queue_family.present_family {
    //         (
    //             vk::SharingMode::CONCURRENT,
    //             2,
    //             vec![
    //                 queue_family.graphics_family.unwrap(),
    //                 queue_family.present_family.unwrap(),
    //             ],
    //         )
    //     } else {
    //         (vk::SharingMode::EXCLUSIVE, 0, vec![])
    //     };


    // TODO: Construct with a builder!
    let create_info = vk::SwapchainCreateInfoKHR::builder()
        .surface(surface_info.surface)
        .min_image_count(image_count)
        .image_color_space(surface_format.color_space)
        .image_format(surface_format.format)
        .image_extent(swapchain_extent)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
        .image_sharing_mode(image_sharing_mode)
        .queue_family_indices(&queue_family_indices)
        .pre_transform(swapchain_support.capabilities.current_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(present_mode)
        .clipped(true)
        .image_array_layers(1);

    let swapchain_loader = ash::extensions::khr::Swapchain::new(instance, device);
    let swapchain = unsafe {
        swapchain_loader
            .create_swapchain(&create_info, None)
            .expect("Failed to create the Swapchain!")
    };

    let swapchain_images = unsafe {
        swapchain_loader
            .get_swapchain_images(swapchain)
            .expect("Failed to get Swapchain images.")
    };

    let swapchain_imageviews = create_image_views(device, surface_format.format, &swapchain_images);

    SwapchainInfo { swapchain_loader, swapchain, swapchain_images, swapchain_format: surface_format.format, swapchain_extent, swapchain_imageviews }
} 

fn create_image_views(
    device: &ash::Device,
    surface_format: vk::Format,
    images: &Vec<vk::Image>,
) -> Vec<vk::ImageView> {
    let swapchain_imageviews: Vec<vk::ImageView> = images
        .iter()
        .map(|&image| {
            create_image_view(
                device,
                image,
                surface_format,
                vk::ImageAspectFlags::COLOR,
                1,
            )
        })
        .collect();

    swapchain_imageviews
}

fn create_image_view(
    device: &ash::Device,
    image: vk::Image,
    format: vk::Format,
    aspect_flags: vk::ImageAspectFlags,
    mip_levels: u32,
) -> vk::ImageView {
    let imageview_create_info = vk::ImageViewCreateInfo {
        s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
        p_next: ptr::null(),
        flags: vk::ImageViewCreateFlags::empty(),
        view_type: vk::ImageViewType::TYPE_2D,
        format,
        components: vk::ComponentMapping {
            r: vk::ComponentSwizzle::IDENTITY,
            g: vk::ComponentSwizzle::IDENTITY,
            b: vk::ComponentSwizzle::IDENTITY,
            a: vk::ComponentSwizzle::IDENTITY,
        },
        subresource_range: vk::ImageSubresourceRange {
            aspect_mask: aspect_flags,
            base_mip_level: 0,
            level_count: mip_levels,
            base_array_layer: 0,
            layer_count: 1,
        },
        image,
    };

    unsafe {
        device
            .create_image_view(&imageview_create_info, None)
            .expect("Failed to create Image View!")
    }
}
