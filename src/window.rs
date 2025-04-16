use std::{num::NonZeroU32, rc::Rc, time::Instant};

use softbuffer::{Context, Surface};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{ElementState, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, KeyCode, PhysicalKey},
    platform::x11::ActiveEventLoopExtX11,
    window::{CursorGrabMode, Window, WindowId},
};

use crate::{
    font::TextWriter,
    rasterizer::{Settings, Stats, TriangleSorting},
    scene::World,
};
use crate::{
    maths::{PI, Rotation},
    rasterizer::rasterize,
};

struct Graphics {
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
}

#[derive(Default)]
pub struct App {
    graphics: Option<Graphics>,
    world: World,
    cursor: Option<PhysicalPosition<f64>>,
    mouse_left_held: bool,
    depth_buffer: Vec<f32>,
    settings: Settings,
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
        let window = Rc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );

        let context = Context::new(window.clone()).expect("Failed to create a softbuffer context");
        let surface =
            Surface::new(&context, window.clone()).expect("Failed to create a softbuffer surface");

        self.graphics = Some(Graphics { window, surface });
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
                    KeyCode::Space => self.world.camera.pos.y += 0.1,
                    KeyCode::ShiftLeft => self.world.camera.pos.y -= 0.1,
                    KeyCode::KeyW => self.world.camera.pos.z -= 0.1,
                    KeyCode::KeyS => self.world.camera.pos.z += 0.1,
                    KeyCode::KeyA => self.world.camera.pos.x -= 0.1,
                    KeyCode::KeyD => self.world.camera.pos.x += 0.1,
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
                    KeyCode::Backquote => {
                        self.settings.show_vertices = !self.settings.show_vertices
                    }
                    KeyCode::Digit1 => self.settings.sort_triangles = TriangleSorting::FrontToBack,
                    KeyCode::Digit2 => self.settings.sort_triangles = TriangleSorting::BackToFront,
                    KeyCode::Digit3 => self.settings.sort_triangles = TriangleSorting::None,
                    KeyCode::Digit4 => {
                        self.settings.back_face_culling = !self.settings.back_face_culling
                    }
                    KeyCode::Digit0 => self.world = Default::default(),
                    // KeyCode::Space => self.world.camera.pos = Vec3f::new(4., 1., -10.),
                    // KeyCode::KeyH => self.world.triangles.iter().nth(4).iter().for_each(|f| {
                    _ => (),
                }
            }
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
                if self.mouse_left_held {
                    let size = &self.graphics.as_ref().unwrap().window.inner_size();
                    self.world.camera.rot = Rotation::from_angles(
                        (position.y as f32 / size.height as f32 / 2. - 0.25) * PI,
                        (position.x as f32 / size.width as f32 / 2. - 0.25) * PI,
                        0.,
                    );
                }
            }
            WindowEvent::RedrawRequested => {
                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in AboutToWait, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.

                let gfx = self.graphics.as_mut().unwrap();

                // Draw.
                let size = gfx.window.inner_size();
                {
                    let (Some(width), Some(height)) =
                        (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                    else {
                        return;
                    };

                    gfx.surface
                        .resize(width, height)
                        .expect("Failed to resize the softbuffer surface");
                }

                let mut buffer = gfx
                    .surface
                    .buffer_mut()
                    .expect("Failed to get the softbuffer buffer");

                // Fill a buffer with a solid color
                buffer.fill(0xff181818);

                let inst = Instant::now();
                let mut stats = Stats::default();

                self.depth_buffer
                    .resize(size.width as usize * size.height as usize, f32::INFINITY);
                self.depth_buffer.fill(f32::INFINITY);
                rasterize(
                    &self.world,
                    &mut buffer,
                    &mut self.depth_buffer[..],
                    &size,
                    &self.settings,
                    &mut stats,
                );

                let mut tw = TextWriter::default();

                let inst = Instant::now().duration_since(inst).as_millis();

                let display = format!(
                    "fps : {} | {}ms{}\n{:?}\n{:#?}",
                    (1000. / inst as f32).round(),
                    inst,
                    self.cursor
                        .and_then(|cursor| buffer
                            .get(cursor.x as usize + cursor.y as usize * size.width as usize)
                            .map(|c| format!(
                                "\n({},{}) 0x{:x}",
                                cursor.x.floor(),
                                cursor.y.floor(),
                                c
                            )))
                        .unwrap_or(String::from("\nNo cursor position")),
                    self.settings,
                    stats
                );
                tw.rasterize(&mut buffer, size, &display[..]);

                buffer
                    .present()
                    .expect("Failed to present the softbuffer buffer");

                // Queue a RedrawRequested event.
                //
                // You only need to call this if you've determined that you need to redraw in
                // applications which do not always need to. Applications that redraw continuously
                // can render here instead.
                gfx.window.request_redraw();
            }
            _ => (),
        }
    }
}
