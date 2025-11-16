use std::{rc::Rc, time::Instant};

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{DeviceEvent, DeviceId, ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};

#[cfg(target_os = "linux")]
use winit::platform::x11::ActiveEventLoopExtX11;

#[cfg(target_os = "android")]
use winit::platform::android::{EventLoopBuilderExtAndroid, activity::AndroidApp};

use crate::rasterizer::{Camera, Engine, Settings};

const BLENDING_RATIO: f32 = 0.01;

/// App data infos to be used and displayed, mostly for debugging
// TODO: check what is actually used here
#[derive(Debug, Clone, Copy)]
pub struct WindowStats {
    last_frame_start_time: Instant,
    last_full_render_loop_micros: u128,
    last_frame_micros: u128,
    frame_avg_micros: u128,
    fps_avg: f32,
    last_rasterize_micros: u128,
}

impl Default for WindowStats {
    fn default() -> Self {
        Self {
            last_frame_start_time: Instant::now(),
            last_full_render_loop_micros: Default::default(),
            last_frame_micros: Default::default(),
            frame_avg_micros: Default::default(),
            fps_avg: Default::default(),
            last_rasterize_micros: Default::default(),
        }
    }
}

impl WindowStats {
    pub fn last_rasterize_micros(&self) -> u128 {
        self.last_rasterize_micros
    }

    pub fn last_full_render_loop_micros(&self) -> u128 {
        self.last_full_render_loop_micros
    }

    pub fn frame_avg_micros(&self) -> u128 {
        self.frame_avg_micros
    }

    pub fn fps_avg(&self) -> f32 {
        self.fps_avg
    }

    pub fn update_last_frame_micros(&mut self) {
        let last_frame_start_time = Instant::now();
        self.last_frame_micros = last_frame_start_time
            .duration_since(self.last_frame_start_time)
            .as_micros();
        self.last_frame_start_time = last_frame_start_time;

        let fps = 1_000_000. / (self.last_frame_micros as f32);
        if (self.fps_avg - fps).abs() > 10. {
            self.frame_avg_micros = self.last_frame_micros;
            self.fps_avg = fps;
        } else {
            self.frame_avg_micros = (self.frame_avg_micros as f32 * (1. - BLENDING_RATIO)
                + self.last_frame_micros as f32 * BLENDING_RATIO)
                as u128;
            self.fps_avg = self.fps_avg * (1. - BLENDING_RATIO) + BLENDING_RATIO * fps;
        }
    }
}

pub struct InitializedWindow<'a> {
    window: Rc<Window>,
    engine: Engine<'a>,
}

impl InitializedWindow<'_> {
    pub fn new(window: Rc<Window>) -> Self {
        Self {
            engine: Engine::new(window.clone()),
            window,
        }
    }
}

#[derive(Default)]
pub struct App<'a> {
    window: Option<InitializedWindow<'a>>,
    camera: Camera,
    cursor_grabbed: bool,
    settings: Settings,
    stats: WindowStats,
}

impl App<'_> {
    pub fn run() {
        let event_loop = EventLoop::new().unwrap();
        // ControlFlow::Poll : Run in a loop (game)
        // Wait : Runs only on event (apps)
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut app = App::default();
        event_loop.run_app(&mut app).unwrap();
    }

    #[cfg(target_os = "android")]
    pub fn run_android(app: AndroidApp) {
        let mut event_loop = EventLoop::builder();
        event_loop.with_android_app(app);
        let event_loop = event_loop.build().unwrap();

        // ControlFlow::Poll : Run in a loop (game)
        // Wait : Runs only on event (apps)
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut app = App::default();
        event_loop.run_app(&mut app).unwrap();
    }
}

impl ApplicationHandler for App<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Rc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("gfx"))
                .unwrap(),
        );

        self.window = Some(InitializedWindow::new(window));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        self.camera.on_window_event(&event);
        self.window.as_mut().unwrap().engine.on_window_event(&event);
        match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        state: ElementState::Pressed,
                        repeat: false,
                        ..
                    },
                ..
            } => {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Character(c),
                        state: ElementState::Pressed,
                        repeat: false,
                        ..
                    },
                ..
            } if c.eq("q") => {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                let w = self.window.as_mut().unwrap();
                match key {
                    KeyCode::Space => {
                        if !self.cursor_grabbed {
                            w.window
                                .set_cursor_grab(CursorGrabMode::Confined)
                                .expect("Can't grab cursor.");
                            w.window.set_cursor_visible(false);
                            // Not all platforms support Confined or Locked
                            // X11 doesn't support Locked and Wayland doesn't support setting cursor position without locking
                            // .or_else(|_| window.set_cursor_grab(CursorGrabMode::Locked))

                            #[cfg(target_os = "linux")]
                            if event_loop.is_x11() {
                                let size = w.window.inner_size();
                                w.window
                                    .set_cursor_position(PhysicalPosition::new(
                                        size.width / 2,
                                        size.height / 2,
                                    ))
                                    .expect("Could not center cursor");
                            }
                            self.cursor_grabbed = true;
                        } else {
                            w.window
                                .set_cursor_grab(CursorGrabMode::None)
                                .expect("Can't release grab on cursor.");
                            w.window.set_cursor_visible(true);
                            self.cursor_grabbed = false;
                        }
                    }
                    KeyCode::Digit0 => self.camera = Default::default(),
                    // KeyCode::Space => self.world.camera.pos = Vec3f::new(4., 1., -10.),
                    // KeyCode::KeyH => self.world.triangles.iter().nth(4).iter().for_each(|f| {
                    _ => (),
                }
            }
            WindowEvent::RedrawRequested => {
                self.stats.update_last_frame_micros();
                self.camera.update(self.stats.last_frame_micros);

                let w = self.window.as_mut().unwrap();

                let t = Instant::now();
                w.engine
                    .rasterize(&self.settings, &self.camera, &self.stats);
                self.stats.last_rasterize_micros = t.elapsed().as_micros();

                // Queue a RedrawRequested event.
                //
                // You only need to call this if you've determined that you need to redraw in
                // applications which do not always need to. Applications that redraw continuously
                // can render here instead.
                w.window.request_redraw();

                self.stats.last_full_render_loop_micros =
                    self.stats.last_frame_start_time.elapsed().as_micros();
            }
            _ => (),
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.camera.on_mouse_motion(delta, self.cursor_grabbed);
            self.window
                .as_mut()
                .unwrap()
                .engine
                .on_mouse_motion(delta, self.cursor_grabbed);
        }
    }
}
