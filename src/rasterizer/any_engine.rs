use std::ops::DerefMut;
use winit::dpi::PhysicalSize;

use crate::{font::TextWriter, scene::World, window::AppObserver};

use super::{
    Engine,
    parallel::{ParIterEngine, ParIterEngine2, ParIterEngine3, ParIterEngine4},
    settings::Settings,
    single_threaded::{IteratorEngine, OriginalEngine},
};

#[derive(Debug, Clone)]
pub enum AnyEngine {
    Original(OriginalEngine),
    Iterator(IteratorEngine),
    ParIter(ParIterEngine),
    ParIter2(ParIterEngine2),
    ParIter3(ParIterEngine3),
    ParIter4(ParIterEngine4),
}

impl Default for AnyEngine {
    fn default() -> Self {
        AnyEngine::Iterator(Default::default())
    }
}

impl AnyEngine {
    pub fn set_next(&mut self) {
        match self {
            AnyEngine::Original(_) => *self = AnyEngine::Iterator(Default::default()),
            AnyEngine::Iterator(_) => *self = AnyEngine::ParIter(Default::default()),
            // Skip 2
            AnyEngine::ParIter(_) => *self = AnyEngine::ParIter3(Default::default()),
            AnyEngine::ParIter2(_) => *self = AnyEngine::ParIter3(Default::default()),
            AnyEngine::ParIter3(_) => *self = AnyEngine::ParIter4(Default::default()),
            AnyEngine::ParIter4(_) => *self = AnyEngine::Original(Default::default()),
        }
    }
}

impl Engine for AnyEngine {
    fn rasterize<B: DerefMut<Target = [u32]>>(
        &mut self,
        settings: &Settings,
        text_writer: &TextWriter,
        world: &World,
        buffer: &mut B,
        size: PhysicalSize<u32>,
        app: AppObserver,
        #[cfg(feature = "stats")] stats: &mut Stats,
    ) {
        match self {
            AnyEngine::Original(e) => e.rasterize(settings, text_writer, world, buffer, size, app),
            AnyEngine::Iterator(e) => e.rasterize(settings, text_writer, world, buffer, size, app),
            AnyEngine::ParIter(e) => e.rasterize(settings, text_writer, world, buffer, size, app),
            AnyEngine::ParIter2(e) => e.rasterize(settings, text_writer, world, buffer, size, app),
            AnyEngine::ParIter3(e) => e.rasterize(settings, text_writer, world, buffer, size, app),
            AnyEngine::ParIter4(e) => e.rasterize(settings, text_writer, world, buffer, size, app),
        }
    }
}
