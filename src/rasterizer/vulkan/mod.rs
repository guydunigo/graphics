use std::{
    ffi::{CStr, CString},
    ops::DerefMut,
    rc::Rc,
};

use ash::{Entry, Instance, LoadingError, vk};
use winit::{dpi::PhysicalSize, window::Window};

use crate::{font::TextWriter, scene::World, window::AppObserver};

use super::settings::Settings;

#[cfg(feature = "stats")]
use super::Stats;

const APP_NAME: &CStr = c"My rasterizer";

pub struct VulkanEngine {
    window: Window,

    // TODO: convert to Rust way...
    frame_number: usize,
    window_extent: vk::Extent2D,

    instance: Instance,
    debug_messenger: vk::DebugUtilsMessengerEXT,
    chosen_gpu: vk::PhysicalDevice,
    device: vk::Device,
    surface: vk::SurfaceKHR,
}

impl VulkanEngine {
    pub fn rasterize<B: DerefMut<Target = [u32]>>(
        &mut self,
        _settings: &Settings,
        _text_writer: &TextWriter,
        _world: &World,
        _buffer: &mut B,
        mut _size: PhysicalSize<u32>,
        _app: &mut AppObserver,
        #[cfg(feature = "stats")] _stats: &mut Stats,
    ) {
        todo!();
    }

    fn init(window: Window) -> Self {
        let instance = Self::init_vulkan();

        // let mut res = Self {
        //     window,

        //     frame_number: 0,
        //     // window_extent: vk::Extent2D,

        //     // instance: vk::Instance,
        //     // debug_messenger: vk::DebugUtilsMessengerEXT,
        //     // chosen_gpu: vk::PhysicalDevice,
        //     // device: vk::Device,
        //     // surface: vk::SurfaceKHR,
        // };
        todo!();

        // Self::init_vulkan();
        // Self::init_swapchain();
        // Self::init_commands();
        // Self::init_sync_structures();
    }

    fn init_vulkan() -> vk::Instance {
        let entry = unsafe { Entry::load().unwrap() };
        let app_info = vk::ApplicationInfo::default()
            .api_version(vk::make_api_version(1, 3, 0, 0))
            .application_name(APP_NAME);
        // TODO: other parameters : engine name + version, ...
        let create_info = vk::InstanceCreateInfo {
            p_application_info: &app_info,
            ..Default::default()
        };
        let instance = unsafe { entry.create_instance(&create_info, None).unwrap() };

        // TODO: look into ash-examples/src/lib
        // TODO: validation layers + default debug messenger

        todo!();
    }

    fn init_swapchain(&mut self) {
        todo!();
    }

    fn init_commands(&mut self) {
        todo!();
    }

    fn init_sync_structures(&mut self) {
        todo!();
    }
}

impl Drop for VulkanEngine {
    fn drop(&mut self) {
        unsafe { self.instance.destroy_instance(None) };
        todo!()
    }
}
