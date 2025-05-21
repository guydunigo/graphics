use std::{ops::DerefMut, rc::Rc};
use winit::{dpi::PhysicalSize, window::Window};

use crate::{font::TextWriter, scene::World, window::AppObserver};

use super::{
    parallel::{ParIterEngine, ParIterEngine2, ParIterEngine3, ParIterEngine4, ParIterEngine5},
    settings::Settings,
    single_threaded::{IteratorEngine, OriginalEngine, SingleThreadedEngine},
    vulkan::VulkanEngine,
};

#[cfg(feature = "stats")]
use super::Stats;

pub enum AnyEngine {
    Original(OriginalEngine),
    Iterator(IteratorEngine),
    ParIter2(ParIterEngine2),
    ParIter3(ParIterEngine3),
    ParIter4(ParIterEngine4),
    ParIter5(ParIterEngine5),
    Vulkan(VulkanEngine),
}

impl Default for AnyEngine {
    fn default() -> Self {
        AnyEngine::ParIter5(Default::default())
    }
}

impl AnyEngine {
    pub fn set_next(&mut self, window: &Rc<Window>) {
        match self {
            AnyEngine::Original(_) => *self = AnyEngine::Iterator(Default::default()),
            AnyEngine::Iterator(_) => *self = AnyEngine::ParIter2(Default::default()),
            AnyEngine::ParIter2(_) => *self = AnyEngine::ParIter3(Default::default()),
            AnyEngine::ParIter3(_) => *self = AnyEngine::ParIter4(Default::default()),
            AnyEngine::ParIter4(_) => *self = AnyEngine::ParIter5(Default::default()),
            AnyEngine::ParIter5(_) => *self = AnyEngine::Vulkan(VulkanEngine::new(window.clone())),
            AnyEngine::Vulkan(_) => *self = AnyEngine::Original(Default::default()),
        }
    }

    pub fn rasterize<B: DerefMut<Target = [u32]>>(
        &mut self,
        settings: &Settings,
        text_writer: &TextWriter,
        world: &World,
        buffer: &mut B,
        size: PhysicalSize<u32>,
        app: &mut AppObserver,
        #[cfg(feature = "stats")] stats: &mut Stats,
    ) {
        match self {
            AnyEngine::Original(e) => e.rasterize(
                settings,
                text_writer,
                world,
                buffer,
                size,
                app,
                #[cfg(feature = "stats")]
                stats,
            ),
            AnyEngine::Iterator(e) => e.rasterize(
                settings,
                text_writer,
                world,
                buffer,
                size,
                app,
                #[cfg(feature = "stats")]
                stats,
            ),
            AnyEngine::ParIter2(e) => e.rasterize(
                settings,
                text_writer,
                world,
                buffer,
                size,
                app,
                #[cfg(feature = "stats")]
                stats,
            ),
            AnyEngine::ParIter3(e) => e.rasterize(
                settings,
                text_writer,
                world,
                buffer,
                size,
                app,
                #[cfg(feature = "stats")]
                stats,
            ),
            AnyEngine::ParIter4(e) => e.rasterize(
                settings,
                text_writer,
                world,
                buffer,
                size,
                app,
                #[cfg(feature = "stats")]
                stats,
            ),
            AnyEngine::ParIter5(e) => e.rasterize(
                settings,
                text_writer,
                world,
                buffer,
                size,
                app,
                #[cfg(feature = "stats")]
                stats,
            ),
            AnyEngine::Vulkan(e) => e.rasterize(
                settings,
                text_writer,
                world,
                buffer,
                size,
                app,
                #[cfg(feature = "stats")]
                stats,
            ),
        }
    }
}
