use std::ops::DerefMut;

use winit::dpi::PhysicalSize;

use crate::{font::TextWriter, scene::World, window::AppObserver};

use super::settings::Settings;

#[cfg(feature = "stats")]
use super::Stats;

#[derive(Default, Debug, Clone, Copy)]
pub struct VulkanEngine {}

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
}
