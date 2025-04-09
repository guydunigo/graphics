use std::{num::NonZeroU32, rc::Rc};

use softbuffer::{Context, Surface};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use crate::{maths::Vec3f, rasterizer::rasterize};
use crate::{rasterizer::world_to_raster_triangle, scene::World};

struct Graphics {
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
}

#[derive(Default)]
pub struct App {
    graphics: Option<Graphics>,
    world: World,
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
            } => match key {
                KeyCode::ArrowLeft | KeyCode::KeyA => self.world.camera.pos.x -= 0.1,
                KeyCode::ArrowRight | KeyCode::KeyD => self.world.camera.pos.x += 0.1,
                KeyCode::ArrowUp => self.world.camera.pos.y += 0.1,
                KeyCode::ArrowDown => self.world.camera.pos.y -= 0.1,
                KeyCode::KeyW => self.world.camera.pos.z -= 0.1,
                KeyCode::KeyS => self.world.camera.pos.z += 0.1,
                KeyCode::Space => self.world.camera.pos = Vec3f::new(4., 1., -9.),
                KeyCode::KeyH => self.world.triangles.iter().nth(4).iter().for_each(|f| {
                    dbg!(world_to_raster_triangle(
                        &f,
                        &self.world.camera,
                        &self.graphics.as_ref().unwrap().window.inner_size()
                    ));
                }),
                _ => (),
            },
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

                rasterize(&self.world, &mut buffer, &size);

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
