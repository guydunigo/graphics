use std::ops::DerefMut;
use winit::dpi::PhysicalSize;

use crate::{font::TextWriter, scene::World, window::AppObserver};

use super::{
    Engine,
    parallel::ParIterEngine,
    settings::Settings,
    single_threaded::{IteratorEngine, OriginalEngine},
};

#[derive(Debug, Clone)]
pub enum AnyEngine {
    Original(OriginalEngine),
    Iterator(IteratorEngine),
    ParIter(ParIterEngine),
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
            AnyEngine::ParIter(_) => *self = AnyEngine::Original(Default::default()),
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
        }
    }
}
