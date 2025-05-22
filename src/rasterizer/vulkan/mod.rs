use std::{
    borrow::Cow,
    ffi::{self, CStr, c_char},
    ops::{Deref, DerefMut},
    rc::Rc,
};

use ash::{Entry, Instance, ext::debug_utils, khr::surface, vk};
use winit::{
    dpi::PhysicalSize,
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::Window,
};

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

    pub fn new(window: Rc<Window>) -> Self {
        Self::init_vulkan(&window);

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

    // Inspired from vkguide.dev and ash-examples/src/lib.rs since we don't have VkBootstrap
    fn init_vulkan<W: Deref<Target = Window>>(window: &W) -> vk::Instance {
        let entry = unsafe { Entry::load().unwrap() };

        let validation_layers = validation_layers();
        let extension_names = extension_names(window);
        let app_info = app_info();
        let create_flags = instance_create_flags();
        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_layer_names(&validation_layers)
            .enabled_extension_names(&extension_names)
            .flags(create_flags);

        let instance = unsafe { entry.create_instance(&create_info, None).unwrap() };

        let debug_messenger = debug_messenger(&entry, &instance);
        let surface = surface(window, &entry, &instance);

        let (pdevice, queue_family_index) =
            find_physical_device(&entry, &instance, &app_info, &surface);

        todo!();

        // Self {
        //     window,

        //     instance,
        //     debug_messenger,
        // }
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
        // TODO: clean debug, ... ?
        // unsafe { self.instance.destroy_instance(None) };
        // todo!()
    }
}

/// From ash-examples/src/lib.rs
unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    unsafe {
        let callback_data = *p_callback_data;
        let message_id_number = callback_data.message_id_number;

        let message_id_name = if callback_data.p_message_id_name.is_null() {
            Cow::from("")
        } else {
            ffi::CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
        };

        let message = if callback_data.p_message.is_null() {
            Cow::from("")
        } else {
            ffi::CStr::from_ptr(callback_data.p_message).to_string_lossy()
        };

        println!(
            "{message_severity:?}:\n{message_type:?} [{message_id_name} ({message_id_number})] : {message}\n",
        );
    }

    vk::FALSE
}

fn extension_names<W: Deref<Target = Window>>(window: &W) -> Vec<*const c_char> {
    let mut extension_names =
        ash_window::enumerate_required_extensions(window.display_handle().unwrap().as_raw())
            .unwrap()
            .to_vec();
    extension_names.push(debug_utils::NAME.as_ptr());

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        extension_names.push(ash::khr::portability_enumeration::NAME.as_ptr());
        // Enabling this extension is a requirement when using `VK_KHR_portability_subset`
        extension_names.push(ash::khr::get_physical_device_properties2::NAME.as_ptr());
    }

    extension_names
}

fn validation_layers() -> Vec<*const c_char> {
    let layer_names = [c"VK_LAYER_KHRONOS_validation"];
    layer_names
        .iter()
        .map(|raw_name| raw_name.as_ptr())
        .collect()
}

fn app_info() -> vk::ApplicationInfo<'static> {
    // TODO: other parameters : engine name + version, ...
    vk::ApplicationInfo::default()
        .api_version(vk::API_VERSION_1_3)
        .application_name(APP_NAME)
        .application_version(0)
        .engine_name(APP_NAME)
        .engine_version(0)
}

fn instance_create_flags() -> vk::InstanceCreateFlags {
    if cfg!(any(target_os = "macos", target_os = "ios")) {
        vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR
    } else {
        vk::InstanceCreateFlags::default()
    }
}

fn debug_messenger(entry: &Entry, instance: &Instance) -> vk::DebugUtilsMessengerEXT {
    let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
        .message_severity(
            vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
        )
        .message_type(
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
        )
        .pfn_user_callback(Some(vulkan_debug_callback));

    let debug_utils_loader = debug_utils::Instance::new(entry, instance);

    unsafe {
        debug_utils_loader
            .create_debug_utils_messenger(&debug_info, None)
            .unwrap()
    }
}

fn surface<W: Deref<Target = Window>>(
    window: &W,
    entry: &Entry,
    instance: &Instance,
) -> vk::SurfaceKHR {
    unsafe {
        ash_window::create_surface(
            entry,
            instance,
            window.display_handle().unwrap().as_raw(),
            window.window_handle().unwrap().as_raw(),
            None,
        )
        .unwrap()
    }
}

fn find_physical_device(
    entry: &Entry,
    instance: &Instance,
    app_info: &vk::ApplicationInfo,
    surface: &vk::SurfaceKHR,
) -> (vk::PhysicalDevice, usize) {
    let surface_loader = surface::Instance::new(entry, instance);
    let pdevices = unsafe {
        instance
            .enumerate_physical_devices()
            .expect("Physical device error")
    };
    println!("There are {} found GPUs : ", pdevices.len());
    let (pdevice, queue_family_index) = pdevices
        .iter()
        .filter(|pdevice| {
            let properties = unsafe { instance.get_physical_device_properties(**pdevice) };
            println!(
                "- Device Name: {}, id: {}, type: {:?}, API version: {}.{}.{}",
                properties.device_name_as_c_str().unwrap().to_string_lossy(),
                properties.device_id,
                properties.device_type,
                vk::api_version_major(properties.api_version),
                vk::api_version_minor(properties.api_version),
                vk::api_version_patch(properties.api_version),
            );
            if properties.api_version < app_info.api_version {
                eprintln!(
                    "\tDevice Vulkan API version lower than app, required : {}.{}.{}",
                    vk::api_version_major(app_info.api_version),
                    vk::api_version_minor(app_info.api_version),
                    vk::api_version_patch(app_info.api_version),
                );

                false
            } else {
                true
            }
        })
        .filter(|pdevice| {
            let mut features13 = vk::PhysicalDeviceVulkan13Features::default();
            let mut features12 = vk::PhysicalDeviceVulkan12Features::default();
            let mut features = vk::PhysicalDeviceFeatures2::default()
                .push_next(&mut features13)
                .push_next(&mut features12);
            unsafe { instance.get_physical_device_features2(**pdevice, &mut features) };

            let mut has_features = true;
            if features12.buffer_device_address == vk::FALSE {
                eprintln!("\tMissing feature 1.2 : buffer_device_address");
                has_features = false;
            }
            if features12.descriptor_indexing == vk::FALSE {
                eprintln!("\tMissing feature 1.2 : descriptor_indexing");

                has_features = false;
            }
            if features13.dynamic_rendering == vk::FALSE {
                eprintln!("\tMissing feature 1.3 : dynamic_rendering");
                has_features = false;
            }
            if features13.synchronization2 == vk::FALSE {
                eprintln!("\tMissing feature 1.3 : synchronization2");
                has_features = false;
            }

            has_features
        })
        .find_map(|pdevice| {
            // Find a queue that can do graphics and that is supported by surface.
            unsafe { instance.get_physical_device_queue_family_properties(*pdevice) }
                .iter()
                .enumerate()
                .find_map(|(index, info)| {
                    let supports_graphic_and_surface =
                        info.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                            && unsafe {
                                surface_loader
                                    .get_physical_device_surface_support(
                                        *pdevice,
                                        index as u32,
                                        *surface,
                                    )
                                    .unwrap()
                            };
                    if supports_graphic_and_surface {
                        Some((*pdevice, index))
                    } else {
                        None
                    }
                })
        })
        .expect("Couldn't find suitable device.");

    (pdevice, queue_family_index)
}
