use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};

use ash::{Device, vk};
use vk_mem::Allocator;

use super::{commands::VulkanCommands, swapchain::AllocatedImage};

pub struct Textures {
    device_copy: Rc<Device>,

    white: AllocatedImage,
    grey: AllocatedImage,
    black: AllocatedImage,
    error_checkerboard: AllocatedImage,

    default_sampler_linear: vk::Sampler,
    default_sampler_nearest: vk::Sampler,
}

impl Textures {
    pub fn new(
        commands: &VulkanCommands,
        device: Rc<Device>,
        allocator: Arc<Mutex<Allocator>>,
    ) -> Self {
        let extent = vk::Extent3D {
            width: 1,
            height: 1,
            depth: 1,
        };
        let format = vk::Format::R8G8B8A8_UNORM;
        let usages = vk::ImageUsageFlags::SAMPLED;

        let white = {
            let data = glam::U8Vec4::splat(255).to_array();
            AllocatedImage::new_image_with_data(
                commands,
                device.clone(),
                allocator.clone(),
                extent,
                format,
                usages,
                false,
                &data[..],
            )
        };

        let black_data = glam::U8Vec4::splat(0).to_array();
        let black = {
            AllocatedImage::new_image_with_data(
                commands,
                device.clone(),
                allocator.clone(),
                extent,
                format,
                usages,
                false,
                &black_data[..],
            )
        };

        let grey = {
            let data = glam::U8Vec4::splat((255. * 0.66) as u8).to_array();
            AllocatedImage::new_image_with_data(
                commands,
                device.clone(),
                allocator.clone(),
                extent,
                format,
                usages,
                false,
                &data[..],
            )
        };

        let error_checkerboard = {
            let extent = vk::Extent3D {
                width: 16,
                height: 16,
                depth: 1,
            };
            let magenta_data = glam::u8vec4(255, 0, 255, 255).to_array();
            let mut pixels: [u8; 16 * 16 * 4] = [0; 1024];
            for x in 0..16 {
                for y in 0..16 {
                    if (x % 2) ^ (y % 2) == 0 {
                        pixels[y * 16 + x] = black_data[0];
                        pixels[y * 16 + x + 1] = black_data[1];
                        pixels[y * 16 + x + 2] = black_data[2];
                        pixels[y * 16 + x + 3] = black_data[3];
                    } else {
                        pixels[y * 16 + x] = magenta_data[0];
                        pixels[y * 16 + x + 1] = magenta_data[1];
                        pixels[y * 16 + x + 2] = magenta_data[2];
                        pixels[y * 16 + x + 3] = magenta_data[3];
                    };
                }
            }

            AllocatedImage::new_image_with_data(
                commands,
                device.clone(),
                allocator,
                extent,
                format,
                usages,
                false,
                &pixels[..],
            )
        };

        let default_sampler_linear = {
            let create_info = vk::SamplerCreateInfo::default()
                .mag_filter(vk::Filter::LINEAR)
                .min_filter(vk::Filter::LINEAR);
            unsafe { device.create_sampler(&create_info, None).unwrap() }
        };
        let default_sampler_nearest = {
            let create_info = vk::SamplerCreateInfo::default()
                .mag_filter(vk::Filter::NEAREST)
                .min_filter(vk::Filter::NEAREST);
            unsafe { device.create_sampler(&create_info, None).unwrap() }
        };

        Self {
            device_copy: device,

            white,
            grey,
            black,
            error_checkerboard,
            default_sampler_linear,
            default_sampler_nearest,
        }
    }
}

impl Drop for Textures {
    fn drop(&mut self) {
        println!("drop Textures");
        unsafe {
            self.device_copy
                .destroy_sampler(self.default_sampler_linear, None);
            self.device_copy
                .destroy_sampler(self.default_sampler_nearest, None);
        }
    }
}
