use std::{
    num::NonZeroU32,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use rayon::prelude::*;
use softbuffer::{Context, Surface};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{DeviceEvent, DeviceId, ElementState, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, KeyCode, PhysicalKey},
    platform::x11::ActiveEventLoopExtX11,
    window::{CursorGrabMode, Window, WindowId},
};

#[cfg(feature = "stats")]
use crate::rasterizer::Stats;
use crate::{
    font::TextWriter,
    rasterizer::{Settings, u64_to_color},
    scene::World,
};
use crate::{maths::Rotation, rasterizer::rasterize};

// TODO

#[derive(Default)]
pub struct App {
    last_rendering_duration: u128,
    last_copy_buffer: u128,
    graphics: Option<Graphics>,
    text_writer: TextWriter,
    world: World,
    cursor: Option<PhysicalPosition<f64>>,
    mouse_left_held: bool,
    settings: Settings,
}

impl App {
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::RedrawRequested => {}
            _ => (),
        }
    }
}
