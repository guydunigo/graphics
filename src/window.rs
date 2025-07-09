use std::{rc::Rc, time::Instant};

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{DeviceEvent, DeviceId, ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};

#[cfg(feature = "cpu")]
use glam::Mat4;

#[cfg(target_os = "linux")]
use winit::platform::x11::ActiveEventLoopExtX11;

#[cfg(target_os = "android")]
use winit::platform::android::{EventLoopBuilderExtAndroid, activity::AndroidApp};

#[cfg(feature = "stats")]
use crate::rasterizer::Stats;
use crate::{
    rasterizer::{Engine, Settings},
    scene::World,
};

const BLENDING_RATIO: f32 = 0.01;

/// App data infos to be used and displayed, mostly for debugging
#[derive(Default, Debug, Clone, Copy)]
pub struct AppObserver {
    cursor: Option<PhysicalPosition<f64>>,
    last_full_render_loop_micros: u128,
    last_frame_micros: u128,
    frame_avg_micros: u128,
    fps_avg: f32,
    pub last_buffer_fill_micros: u128,
    pub last_rendering_micros: u128,
    pub last_buffer_copy_micros: u128,
}

impl AppObserver {
    pub fn cursor(&self) -> &Option<PhysicalPosition<f64>> {
        &self.cursor
    }

    pub fn last_full_render_loop_micros(&self) -> u128 {
        self.last_full_render_loop_micros
    }

    pub fn last_frame_micros(&self) -> u128 {
        self.last_frame_micros
    }

    pub fn frame_avg_micros(&self) -> u128 {
        self.frame_avg_micros
    }

    pub fn fps_avg(&self) -> f32 {
        self.fps_avg
    }

    fn from(value: &App) -> Self {
        AppObserver {
            cursor: value.cursor,
            last_full_render_loop_micros: value.last_full_render_loop_micros,
            last_frame_micros: value.last_frame_micros,
            frame_avg_micros: value.frame_avg_micros,
            fps_avg: value.fps_avg,
            last_buffer_fill_micros: value.last_buffer_fill_micros,
            last_rendering_micros: value.last_rendering_micros,
            last_buffer_copy_micros: value.last_buffer_copy_micros,
        }
    }

    fn update_app(&self, app: &mut App) {
        app.cursor = self.cursor;
        app.last_full_render_loop_micros = self.last_full_render_loop_micros;
        app.last_frame_micros = self.last_frame_micros;
        app.frame_avg_micros = self.frame_avg_micros;
        app.fps_avg = self.fps_avg;
        app.last_buffer_fill_micros = self.last_buffer_fill_micros;
        app.last_rendering_micros = self.last_rendering_micros;
        app.last_buffer_copy_micros = self.last_buffer_copy_micros;
    }
}

pub struct InitializedWindow<'a> {
    window: Rc<Window>,
    settings: Settings,
    engine: Engine<'a>,
}

impl InitializedWindow<'_> {
    pub fn new(window: Rc<Window>) -> Self {
        Self {
            window: window.clone(),
            settings: Default::default(),
            engine: Engine::new(window),
        }
    }

    pub fn rasterize(
        &mut self,
        #[cfg(feature = "cpu")] world: &World,
        app: &mut AppObserver,
        #[cfg(feature = "stats")] stats: &mut Stats,
    ) {
        self.engine.rasterize(
            &self.settings,
            #[cfg(feature = "cpu")]
            world,
            app,
            #[cfg(feature = "stats")]
            stats,
        );
    }

    pub fn set_next_engine(&mut self) {
        self.engine.set_next();
        self.settings.engine_type = self.engine.as_engine_type();
    }
}

pub struct App<'a> {
    window: Option<InitializedWindow<'a>>,
    #[cfg(feature = "cpu")]
    world: World,
    cursor: Option<PhysicalPosition<f64>>,
    cursor_grabbed: bool,
    last_full_render_loop_micros: u128,
    last_frame_start_time: Instant,
    last_frame_micros: u128,
    frame_avg_micros: u128,
    fps_avg: f32,
    last_buffer_fill_micros: u128,
    last_rendering_micros: u128,
    last_buffer_copy_micros: u128,
}

impl Default for App<'_> {
    fn default() -> Self {
        Self {
            window: Default::default(),
            #[cfg(feature = "cpu")]
            world: Default::default(),
            cursor: Default::default(),
            cursor_grabbed: Default::default(),
            last_full_render_loop_micros: Default::default(),
            last_frame_start_time: Instant::now(),
            last_frame_micros: Default::default(),
            frame_avg_micros: Default::default(),
            fps_avg: 60.,
            last_buffer_fill_micros: Default::default(),
            last_rendering_micros: Default::default(),
            last_buffer_copy_micros: Default::default(),
        }
    }
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

    pub fn update_last_frame_micros(&mut self) {
        let last_frame_start_time = Instant::now();
        self.last_frame_micros = last_frame_start_time
            .duration_since(self.last_frame_start_time)
            .as_micros();
        self.last_frame_start_time = last_frame_start_time;

        self.frame_avg_micros = (self.frame_avg_micros as f32 * (1. - BLENDING_RATIO)
            + self.last_frame_micros as f32 * BLENDING_RATIO)
            as u128;
        self.fps_avg = self.fps_avg * (1. - BLENDING_RATIO)
            + BLENDING_RATIO * 1_000_000. / (self.last_frame_micros as f32);
    }
}

impl ApplicationHandler for App<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Rc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );

        self.window = Some(InitializedWindow::new(window));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        self.world.camera.on_window_event(&event);
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
                // TODO: drop surface = cleaner ?
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
                                self.cursor_grabbed = true;
                            }
                        } else {
                            w.window
                                .set_cursor_grab(CursorGrabMode::None)
                                .expect("Can't release grab on cursor.");
                            w.window.set_cursor_visible(true);
                            self.cursor_grabbed = false;
                        }
                    }
                    #[cfg(feature = "cpu")]
                    KeyCode::ArrowLeft => {
                        if let Some(m) = self.world.scene.get_named_node("suzanne") {
                            m.borrow_mut().transform(&Mat4::from_rotation_y(-0.1));
                        }
                    }
                    #[cfg(feature = "cpu")]
                    KeyCode::ArrowRight => {
                        if let Some(m) = self.world.scene.get_named_node("suzanne") {
                            m.borrow_mut().transform(&Mat4::from_rotation_y(0.1));
                        }
                    }
                    #[cfg(feature = "cpu")]
                    KeyCode::ArrowUp => {
                        if let Some(m) = self.world.scene.get_named_node("suzanne") {
                            m.borrow_mut().transform(&Mat4::from_rotation_x(-0.1));
                        }
                    }
                    #[cfg(feature = "cpu")]
                    KeyCode::ArrowDown => {
                        if let Some(m) = self.world.scene.get_named_node("suzanne") {
                            m.borrow_mut().transform(&Mat4::from_rotation_x(0.1));
                        }
                    }
                    KeyCode::Backquote => w.settings.show_vertices = !w.settings.show_vertices,
                    KeyCode::Digit1 => w.set_next_engine(),
                    KeyCode::Digit2 => w.settings.sort_triangles.next(),
                    KeyCode::Digit3 => w.settings.parallel_text = !w.settings.parallel_text,
                    KeyCode::Digit4 => w.settings.next_oversampling(),
                    #[cfg(feature = "cpu")]
                    KeyCode::Digit0 => self.world = Default::default(),
                    // KeyCode::Space => self.world.camera.pos = Vec3f::new(4., 1., -10.),
                    // KeyCode::KeyH => self.world.triangles.iter().nth(4).iter().for_each(|f| {
                    _ => (),
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = Some(position);
            }
            WindowEvent::RedrawRequested => {
                // TODO: forward update and events to world to manage itself ?
                self.world.camera.update();

                self.update_last_frame_micros();

                #[cfg(feature = "stats")]
                let mut stats = Stats::default();

                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in AboutToWait, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.

                let mut obs = AppObserver::from(self);

                let w = self.window.as_mut().unwrap();

                w.rasterize(
                    #[cfg(feature = "cpu")]
                    &self.world,
                    &mut obs,
                    #[cfg(feature = "stats")]
                    &mut stats,
                );

                // Queue a RedrawRequested event.
                //
                // You only need to call this if you've determined that you need to redraw in
                // applications which do not always need to. Applications that redraw continuously
                // can render here instead.
                w.window.request_redraw();

                obs.update_app(self);

                self.last_full_render_loop_micros =
                    self.last_frame_start_time.elapsed().as_micros();
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
            self.world
                .camera
                .on_mouse_motion(delta, self.cursor_grabbed);
            self.window
                .as_mut()
                .unwrap()
                .engine
                .on_mouse_motion(delta, self.cursor_grabbed);
        }
    }
}
