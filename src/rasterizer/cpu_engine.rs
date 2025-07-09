use softbuffer::{Context, Surface};
use std::{num::NonZeroU32, ops::DerefMut, rc::Rc};
use winit::{dpi::PhysicalSize, window::Window};

use crate::{font::TextWriter, scene::World, window::AppObserver};

use super::{
    // parallel::{ParIterEngine, ParIterEngine2, ParIterEngine3, ParIterEngine4, ParIterEngine5},
    settings::{EngineType, Settings},
    single_threaded::{IteratorEngine, OriginalEngine, SingleThreadedEngine},
};

#[cfg(feature = "stats")]
use super::Stats;

pub struct CPUEngine {
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
    engine: AnyEngine,
    text_writer: TextWriter,
}

impl CPUEngine {
    pub fn new(window: Rc<Window>) -> Self {
        let context = Context::new(window.clone()).expect("Failed to create a softbuffer context");
        let surface =
            Surface::new(&context, window.clone()).expect("Failed to create a softbuffer surface");

        Self {
            window,
            surface,
            engine: AnyEngine::default(),
            text_writer: TextWriter::default(),
        }
    }

    pub fn window(&self) -> &Rc<Window> {
        &self.window
    }

    pub fn as_engine_type(&self) -> EngineType {
        self.engine.as_engine_type()
    }

    pub fn set_next(&mut self) -> bool {
        self.engine.set_next()
    }

    pub fn rasterize(
        &mut self,
        settings: &Settings,
        world: &World,
        app: &mut AppObserver,
        #[cfg(feature = "stats")] stats: &mut Stats,
    ) {
        let size = self.window.inner_size();
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return;
        };

        self.surface
            .resize(width, height)
            .expect("Failed to resize the softbuffer surface");

        let mut buffer = self
            .surface
            .buffer_mut()
            .expect("Failed to get the softbuffer buffer");

        self.engine.rasterize(
            settings,
            &self.text_writer,
            world,
            &mut buffer,
            size,
            app,
            #[cfg(feature = "stats")]
            stats,
        );

        buffer
            .present()
            .expect("Failed to present the softbuffer buffer");
    }
}

enum AnyEngine {
    Original(OriginalEngine),
    Iterator(IteratorEngine),
    // ParIter2(ParIterEngine2),
    // ParIter3(ParIterEngine3),
    // ParIter4(ParIterEngine4),
    // ParIter5(ParIterEngine5),
}

impl Default for AnyEngine {
    fn default() -> Self {
        AnyEngine::Original(Default::default())
    }
}

impl AnyEngine {
    /// Returns true if looping back to first
    pub fn set_next(&mut self) -> bool {
        match self {
            AnyEngine::Original(_) => *self = AnyEngine::Iterator(Default::default()),
            // AnyEngine::Iterator(_) => *self = AnyEngine::ParIter2(Default::default()),
            // AnyEngine::ParIter2(_) => *self = AnyEngine::ParIter3(Default::default()),
            // AnyEngine::ParIter3(_) => *self = AnyEngine::ParIter4(Default::default()),
            // AnyEngine::ParIter4(_) => *self = AnyEngine::ParIter5(Default::default()),
            AnyEngine::Iterator(_) => {
                *self = AnyEngine::Original(Default::default());
                return true;
            }
        }

        false
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
            // AnyEngine::ParIter2(e) => e.rasterize(
            //     settings,
            //     text_writer,
            //     world,
            //     buffer,
            //     size,
            //     app,
            //     #[cfg(feature = "stats")]
            //     stats,
            // ),
            // AnyEngine::ParIter3(e) => e.rasterize(
            //     settings,
            //     text_writer,
            //     world,
            //     buffer,
            //     size,
            //     app,
            //     #[cfg(feature = "stats")]
            //     stats,
            // ),
            // AnyEngine::ParIter4(e) => e.rasterize(
            //     settings,
            //     text_writer,
            //     world,
            //     buffer,
            //     size,
            //     app,
            //     #[cfg(feature = "stats")]
            //     stats,
            // ),
            // AnyEngine::ParIter5(e) => e.rasterize(
            //     settings,
            //     text_writer,
            //     world,
            //     buffer,
            //     size,
            //     app,
            //     #[cfg(feature = "stats")]
            //     stats,
            // ),
        }
    }

    pub fn as_engine_type(&self) -> EngineType {
        match self {
            AnyEngine::Original(_) => EngineType::Original,
            AnyEngine::Iterator(_) => EngineType::Iterator,
            // AnyEngine::ParIter2(_) => EngineType::ParIter2,
            // AnyEngine::ParIter3(_) => EngineType::ParIter3,
            // AnyEngine::ParIter4(_) => EngineType::ParIter4,
            // AnyEngine::ParIter5(_) => EngineType::ParIter5,
        }
    }
}
