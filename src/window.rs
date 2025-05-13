use std::{num::NonZeroU32, rc::Rc, time::Instant};

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
use crate::{maths::Rotation, rasterizer::Rasterizer, scene::World};

pub struct WindowSurface {
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
}

impl WindowSurface {
    pub fn new(event_loop: &ActiveEventLoop) -> Self {
        let window = Rc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );

        let context = Context::new(window.clone()).expect("Failed to create a softbuffer context");
        let surface =
            Surface::new(&context, window.clone()).expect("Failed to create a softbuffer surface");

        WindowSurface { window, surface }
    }

    pub fn surface(&mut self) -> &mut Surface<Rc<Window>, Rc<Window>> {
        &mut self.surface
    }
}

/// App data infos to be used and displayed, mostly for debugging
#[derive(Default, Debug, Clone, Copy)]
pub struct AppObserver {
    cursor: Option<PhysicalPosition<f64>>,
    last_full_render_loop_micros: u128,
    last_frame_micros: u128,
    pub last_buffer_fill_micros: u128,
    pub last_rendering_micros: u128,
    pub last_buffer_copy_micros: u128,
    fps_avg: u32,
    frame_avg_micros: u128,
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

    fn from(value: &App) -> Self {
        AppObserver {
            cursor: value.cursor,
            last_full_render_loop_micros: value.last_full_render_loop_micros,
            last_frame_micros: value.last_frame_micros,
            last_buffer_fill_micros: value.last_buffer_fill_micros,
            last_rendering_micros: value.last_rendering_micros,
            last_buffer_copy_micros: value.last_buffer_copy_micros,
        }
    }

    fn update_app(&self, app: &mut App) {
        app.cursor = self.cursor;
        app.last_full_render_loop_micros = self.last_full_render_loop_micros;
        app.last_frame_micros = self.last_frame_micros;
        app.last_buffer_fill_micros = self.last_buffer_fill_micros;
        app.last_rendering_micros = self.last_rendering_micros;
        app.last_buffer_copy_micros = self.last_buffer_copy_micros;
    }
}

pub struct App {
    window_surface: Option<WindowSurface>,
    rasterizer: Rasterizer,
    world: World,
    cursor: Option<PhysicalPosition<f64>>,
    mouse_left_held: bool,
    last_full_render_loop_micros: u128,
    last_frame_start_time: Instant,
    last_frame_micros: u128,
    last_buffer_fill_micros: u128,
    last_rendering_micros: u128,
    last_buffer_copy_micros: u128,
}

impl Default for App {
    fn default() -> Self {
        Self {
            window_surface: Default::default(),
            rasterizer: Default::default(),
            world: Default::default(),
            cursor: Default::default(),
            last_full_render_loop_micros: Default::default(),
            last_frame_start_time: Instant::now(),
            last_frame_micros: Default::default(),
            last_buffer_fill_micros: Default::default(),
            last_rendering_micros: Default::default(),
            last_buffer_copy_micros: Default::default(),
            mouse_left_held: Default::default(),
        }
    }
}

impl App {
    pub fn run() {
        let event_loop = EventLoop::new().unwrap();
        // ControlFlow::Poll : Run in a loop (game)
        // Wait : Runs only on event (apps)
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut app = App::default();
        event_loop.run_app(&mut app).unwrap();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.window_surface = Some(WindowSurface::new(event_loop));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
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
                match key {
                    KeyCode::Space => self.world.camera.move_sight(0., 1., 0.),
                    KeyCode::ShiftLeft => self.world.camera.move_sight(0., -1., 0.),
                    KeyCode::KeyW => self.world.camera.move_sight(0., 0., 1.),
                    KeyCode::KeyS => self.world.camera.move_sight(0., 0., -1.),
                    KeyCode::KeyA => self.world.camera.move_sight(-1., 0., 0.),
                    KeyCode::KeyD => self.world.camera.move_sight(1., 0., 0.),
                    KeyCode::ArrowLeft => self
                        .world
                        .meshes
                        .iter_mut()
                        .for_each(|m| m.rot *= &Rotation::from_angles(0., -0.1, 0.)),
                    KeyCode::ArrowRight => self
                        .world
                        .meshes
                        .iter_mut()
                        .for_each(|m| m.rot *= &Rotation::from_angles(0., 0.1, 0.)),
                    KeyCode::ArrowUp => self
                        .world
                        .meshes
                        .iter_mut()
                        .for_each(|m| m.rot *= &Rotation::from_angles(-0.1, 0., 0.)),
                    KeyCode::ArrowDown => self
                        .world
                        .meshes
                        .iter_mut()
                        .for_each(|m| m.rot *= &Rotation::from_angles(0.1, 0., 0.)),
                    // TODO: parallel structures
                    // KeyCode::ArrowLeft => self.world.meshes().iter().for_each(|m| {
                    //     m.write().unwrap().rot *= &Rotation::from_angles(0., -0.1, 0.)
                    // }),
                    // KeyCode::ArrowRight => self.world.meshes().iter().for_each(|m| {
                    //     m.write().unwrap().rot *= &Rotation::from_angles(0., 0.1, 0.)
                    // }),
                    // KeyCode::ArrowUp => self.world.meshes().iter().for_each(|m| {
                    //     m.write().unwrap().rot *= &Rotation::from_angles(-0.1, 0., 0.)
                    // }),
                    // KeyCode::ArrowDown => self.world.meshes().iter().for_each(|m| {
                    //     m.write().unwrap().rot *= &Rotation::from_angles(0.1, 0., 0.)
                    // }),
                    KeyCode::Backquote => {
                        self.rasterizer.settings.show_vertices =
                            !self.rasterizer.settings.show_vertices
                    }
                    KeyCode::Digit1 => self.rasterizer.next_engine(),
                    KeyCode::Digit2 => self.rasterizer.settings.sort_triangles.next(),
                    KeyCode::Digit3 => {
                        self.rasterizer.settings.parallel_text =
                            !self.rasterizer.settings.parallel_text
                    }
                    KeyCode::Digit0 => self.world = Default::default(),
                    // KeyCode::Space => self.world.camera.pos = Vec3f::new(4., 1., -10.),
                    // KeyCode::KeyH => self.world.triangles.iter().nth(4).iter().for_each(|f| {
                    _ => (),
                }
            }
            WindowEvent::MouseInput {
                button: MouseButton::Right,
                state: ElementState::Pressed,
                ..
            } => self.world.camera.reset_rot(),
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                let window = &self.window_surface.as_ref().unwrap().window;
                match state {
                    ElementState::Pressed => {
                        window
                            .set_cursor_grab(CursorGrabMode::Confined)
                            .expect("Can't grab cursor.");
                        window.set_cursor_visible(false);
                        // Not all platforms support Confined or Locked
                        // X11 doesn't support Locked and Wayland doesn't support setting cursor position without locking
                        // .or_else(|_| window.set_cursor_grab(CursorGrabMode::Locked))

                        if event_loop.is_x11() {
                            let size = window.inner_size();
                            window
                                .set_cursor_position(PhysicalPosition::new(
                                    size.width / 2,
                                    size.height / 2,
                                ))
                                .expect("Could not center cursor");
                            self.mouse_left_held = true;
                        }
                    }
                    ElementState::Released => {
                        window
                            .set_cursor_grab(CursorGrabMode::None)
                            .expect("Can't release grab on cursor.");
                        window.set_cursor_visible(true);
                        self.mouse_left_held = false;
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = Some(position);
            }
            WindowEvent::RedrawRequested => {
                {
                    let last_frame_start_time = Instant::now();
                    self.last_frame_micros = last_frame_start_time
                        .duration_since(self.last_frame_start_time)
                        .as_micros();
                    self.last_frame_start_time = last_frame_start_time;
                }

                #[cfg(feature = "stats")]
                let mut stats = Stats::default();

                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in AboutToWait, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.

                let mut obs = AppObserver::from(self);

                // TODO: no unwrap
                let window_surface = self.window_surface.as_mut().unwrap();

                // Draw.
                let size = window_surface.window.inner_size();

                let (Some(width), Some(height)) =
                    (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                else {
                    return;
                };

                window_surface
                    .surface()
                    .resize(width, height)
                    .expect("Failed to resize the softbuffer surface");

                let mut buffer = window_surface
                    .surface()
                    .buffer_mut()
                    .expect("Failed to get the softbuffer buffer");
                self.rasterizer
                    .rasterize(&self.world, &mut buffer, size, &mut obs);

                buffer
                    .present()
                    .expect("Failed to present the softbuffer buffer");

                // Queue a RedrawRequested event.
                //
                // You only need to call this if you've determined that you need to redraw in
                // applications which do not always need to. Applications that redraw continuously
                // can render here instead.
                window_surface.window.request_redraw();

                obs.update_app(self);

                self.last_full_render_loop_micros = Instant::now()
                    .duration_since(self.last_frame_start_time)
                    .as_micros();
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
        if let DeviceEvent::MouseMotion { delta: (x, y) } = event {
            if self.mouse_left_held {
                self.world.camera.rotate_from_mouse(x as f32, y as f32);
            }
        }
    }
}
