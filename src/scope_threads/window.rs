use std::{
    num::NonZeroU32,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use rayon::{ThreadPool, ThreadPoolBuilder, prelude::*};
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

struct Graphics {
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
    depth_color_buffer: Arc<[AtomicU64]>,
}

impl Graphics {
    const DEFAULT_COLOR: u32 = 0xff181818;
    const DEFAULT_DEPTH: u32 = u32::MAX;
    const DEFAULT_DEPTH_COLOR: u64 =
        ((Self::DEFAULT_DEPTH as u64) << 32) | (Self::DEFAULT_COLOR as u64);

    pub fn new(event_loop: &ActiveEventLoop) -> Self {
        let window = Rc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );

        let context = Context::new(window.clone()).expect("Failed to create a softbuffer context");
        let surface =
            Surface::new(&context, window.clone()).expect("Failed to create a softbuffer surface");

        let size = window.inner_size();
        let tot_size = (size.width * size.height) as usize;

        Graphics {
            window,
            surface,
            depth_color_buffer: Self::init_buffer(tot_size, || {
                AtomicU64::new(Self::DEFAULT_DEPTH_COLOR)
            }),
        }
    }

    fn init_buffer<T, F: Fn() -> T>(tot_size: usize, f: F) -> Arc<[T]> {
        let mut v = Vec::with_capacity(tot_size);
        v.resize_with(tot_size, f);
        v.into()
    }

    fn resize(&mut self) {
        let size = self.window.inner_size();
        let tot_size = (size.width * size.height) as usize;

        if self.depth_color_buffer.len() >= tot_size {
            self.depth_color_buffer
                .par_iter()
                .take(tot_size)
                .for_each(|v| v.store(Self::DEFAULT_DEPTH_COLOR, Ordering::Relaxed))
        } else {
            self.depth_color_buffer =
                Self::init_buffer(tot_size, || AtomicU64::new(Self::DEFAULT_DEPTH_COLOR));
        }
    }
}

pub struct App {
    last_rendering_duration: u128,
    last_copy_buffer: u128,
    graphics: Option<Graphics>,
    text_writer: TextWriter,
    world: World,
    cursor: Option<PhysicalPosition<f64>>,
    mouse_left_held: bool,
    settings: Settings,
    threads: ThreadPool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            last_rendering_duration: Default::default(),
            last_copy_buffer: Default::default(),
            graphics: Default::default(),
            text_writer: Default::default(),
            world: Default::default(),
            cursor: Default::default(),
            mouse_left_held: Default::default(),
            settings: Default::default(),
            threads: ThreadPoolBuilder::new().build().unwrap(),
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
        self.graphics = Some(Graphics::new(event_loop));
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
                    KeyCode::ArrowLeft => self.world.meshes().iter().for_each(|m| {
                        m.write().unwrap().rot *= &Rotation::from_angles(0., -0.1, 0.)
                    }),
                    KeyCode::ArrowRight => self.world.meshes().iter().for_each(|m| {
                        m.write().unwrap().rot *= &Rotation::from_angles(0., 0.1, 0.)
                    }),
                    KeyCode::ArrowUp => self.world.meshes().iter().for_each(|m| {
                        m.write().unwrap().rot *= &Rotation::from_angles(-0.1, 0., 0.)
                    }),
                    KeyCode::ArrowDown => self.world.meshes().iter().for_each(|m| {
                        m.write().unwrap().rot *= &Rotation::from_angles(0.1, 0., 0.)
                    }),
                    KeyCode::Backquote => {
                        self.settings.show_vertices = !self.settings.show_vertices
                    }
                    // KeyCode::Digit4 => self.settings.back_face_culling = !self.settings.back_face_culling,
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
                let window = &self.graphics.as_ref().unwrap().window;
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
                let frame_start_time = Instant::now();

                #[cfg(feature = "stats")]
                let stats = Stats::default();

                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in AboutToWait, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.

                let gfx = self.graphics.as_mut().unwrap();

                // Draw.
                let size = gfx.window.inner_size();

                let buffers_fill = Instant::now();
                gfx.resize();
                let buffers_fill = t.elapsed().as_millis();

                let rendering_time = Instant::now();
                self.threads.install(|| {
                    rasterize(
                        &self.world,
                        &gfx.depth_color_buffer,
                        &size,
                        &self.settings,
                        #[cfg(feature = "stats")]
                        &stats,
                    );
                });
                let rendering_time = t.elapsed().as_millis();

                self.threads.install(|| {
                    let cam_rot = self.world.camera.rot();
                    #[cfg(feature = "stats")]
                    let stats = format!("{:#?}", stats);
                    #[cfg(not(feature = "stats"))]
                    let stats = "Stats disabled";
                    let display = format!(
                        "fps : {} | {}ms - {}ms - {}ms / {}ms{}\n{} {} {} {}\n{:?}\n{}",
                        (1000. / self.last_rendering_duration as f32).round(),
                        buffers_fill,
                        rendering_time,
                        self.last_copy_buffer,
                        self.last_rendering_duration,
                        self.cursor
                            .and_then(|cursor| gfx
                                .depth_color_buffer
                                .get(cursor.x as usize + cursor.y as usize * size.width as usize)
                                .map(|c| format!(
                                    "\n({},{}) 0x{:x}",
                                    cursor.x.floor(),
                                    cursor.y.floor(),
                                    u64_to_color(c.load(Ordering::Relaxed))
                                )))
                            .unwrap_or(String::from("\nNo cursor position")),
                        self.world.camera.pos,
                        cam_rot.u(),
                        cam_rot.v(),
                        cam_rot.w(),
                        self.settings,
                        stats,
                    );
                    self.text_writer
                        .rasterize_par(&gfx.depth_color_buffer, size, &display[..]);
                });

                let copy_buffer = Instant::now();
                let buffer = {
                    let (Some(width), Some(height)) =
                        (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                    else {
                        return;
                    };

                    gfx.surface
                        .resize(width, height)
                        .expect("Failed to resize the softbuffer surface");

                    let mut buffer = gfx
                        .surface
                        .buffer_mut()
                        .expect("Failed to get the softbuffer buffer");

                    (0..(size.width * size.height) as usize).for_each(|i| {
                        buffer[i] = u64_to_color(gfx.depth_color_buffer[i].load(Ordering::Relaxed));
                    });

                    buffer
                };
                self.last_copy_buffer = t.elapsed().as_millis();

                buffer
                    .present()
                    .expect("Failed to present the softbuffer buffer");

                // Queue a RedrawRequested event.
                //
                // You only need to call this if you've determined that you need to redraw in
                // applications which do not always need to. Applications that redraw continuously
                // can render here instead.
                gfx.window.request_redraw();

                self.last_rendering_duration = t.elapsed().as_millis();
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
