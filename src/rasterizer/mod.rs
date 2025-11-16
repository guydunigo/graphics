mod camera;
mod settings;
#[cfg(feature = "vulkan")]
mod vulkan;

pub use camera::Camera;
pub use settings::Settings;

#[cfg(feature = "vulkan")]
pub use vulkan::VulkanEngine as Engine;

pub const FONT_NAME: &str = "DejaVuSansMono";
pub const FONT: &[u8] = include_bytes!("../../resources/DejaVuSansMono.ttf") as &[u8];
