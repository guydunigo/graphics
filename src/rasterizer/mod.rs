#[cfg(feature = "cpu")]
mod cpu;
mod settings;
#[cfg(feature = "vulkan")]
mod vulkan;

use std::rc::Rc;
use winit::{event::WindowEvent, window::Window};

use crate::window::AppObserver;
#[cfg(feature = "stats")]
pub use cpu::Stats;
use settings::EngineType;
pub use settings::Settings;

#[cfg(feature = "vulkan")]
use vulkan::VulkanEngine;

#[cfg(feature = "cpu")]
use crate::scene::World;
#[cfg(feature = "cpu")]
use cpu::CPUEngine;
#[cfg(feature = "cpu")]
use std::marker::PhantomData;

pub enum Engine<'a> {
    #[cfg(feature = "cpu")]
    Cpu(Box<CPUEngine>, PhantomData<&'a ()>),
    #[cfg(feature = "vulkan")]
    Vulkan(Box<VulkanEngine<'a>>),
}

impl Engine<'_> {
    #[cfg(feature = "vulkan")]
    pub fn new(window: Rc<Window>) -> Self {
        Self::Vulkan(Box::new(VulkanEngine::new(window)))
    }

    #[cfg(all(feature = "cpu", not(feature = "vulkan")))]
    pub fn new(window: Rc<Window>) -> Self {
        Self::Cpu(Box::new(CPUEngine::new(window)), Default::default())
    }

    #[cfg(all(feature = "vulkan", not(feature = "cpu")))]
    pub fn set_next(&mut self) {}

    #[cfg(all(not(feature = "vulkan"), feature = "cpu"))]
    pub fn set_next(&mut self) {
        let Engine::Cpu(e, _) = self;
        e.set_next();
    }

    #[cfg(all(feature = "vulkan", feature = "cpu"))]
    pub fn set_next(&mut self) {
        match self {
            Engine::Cpu(e, _) => {
                if e.set_next() {
                    *self = Engine::Vulkan(Box::new(VulkanEngine::new(e.window().clone())));
                }
            }
            Engine::Vulkan(e) => {
                *self = Engine::Cpu(
                    Box::new(CPUEngine::new(e.window().clone())),
                    Default::default(),
                )
            }
        }
    }

    pub fn as_engine_type(&self) -> EngineType {
        match self {
            #[cfg(feature = "cpu")]
            Self::Cpu(e, _) => e.as_engine_type(),
            #[cfg(feature = "vulkan")]
            Self::Vulkan(_) => EngineType::Vulkan,
        }
    }

    pub fn rasterize(
        &mut self,
        settings: &Settings,
        #[cfg(feature = "cpu")] world: &World,
        app: &mut AppObserver,
        #[cfg(feature = "stats")] stats: &mut Stats,
    ) {
        match self {
            #[cfg(feature = "cpu")]
            Self::Cpu(e, _) => e.rasterize(
                settings,
                world,
                app,
                #[cfg(feature = "stats")]
                stats,
            ),
            #[cfg(feature = "vulkan")]
            Self::Vulkan(e) => e.rasterize(
                settings,
                #[cfg(feature = "cpu")]
                world,
                app,
                #[cfg(feature = "stats")]
                stats,
            ),
        }
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) {
        #[cfg(feature = "cpu")]
        let _ = event;
        match self {
            #[cfg(feature = "cpu")]
            Self::Cpu(_, _) => (),
            #[cfg(feature = "vulkan")]
            Self::Vulkan(e) => e.on_window_event(event),
        }
    }

    pub fn on_mouse_motion(&mut self, delta: (f64, f64), cursor_grabbed: bool) {
        #[cfg(feature = "cpu")]
        let _ = (delta, cursor_grabbed);
        match self {
            #[cfg(feature = "cpu")]
            Self::Cpu(_, _) => (),
            #[cfg(feature = "vulkan")]
            Self::Vulkan(e) => e.on_mouse_motion(delta, cursor_grabbed),
        }
    }
}
