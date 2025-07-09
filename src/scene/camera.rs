use glam::{Mat4, Quat, Vec3, Vec4, Vec4Swizzles, vec3};
use winit::{
    event::{ElementState, KeyEvent, MouseButton, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub z_near: f32,
    pub canvas_side: f32,

    pub pos: Vec3,
    pub vel: Vec3,

    pub pitch: f32,
    pub yaw: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            z_near: 0.5,
            canvas_side: 0.1,

            pos: vec3(1., 1., 12.),
            vel: Default::default(),

            pitch: 0.,
            yaw: 0.,
        }
    }
}

impl Camera {
    const MOVE_STEP: f32 = 0.1;
    const ROT_STEP: f32 = 0.001;

    pub fn view_mat(&self) -> Mat4 {
        // to create a correct model view, we need to move the world in opposite
        // direction to the camera
        //  so we will create the camera model matrix and invert
        let tr = Mat4::from_translation(self.pos);
        let rot = self.rot_mat();
        (tr * rot).inverse()
    }

    pub fn rot_mat(&self) -> Mat4 {
        // fairly typical FPS style camera. we join the pitch and yaw rotations into
        // the final rotation matrix
        let pitch = Quat::from_axis_angle(vec3(1., 0., 0.), self.pitch);
        let yaw = Quat::from_axis_angle(vec3(0., -1., 0.), self.yaw);

        Mat4::from_quat(yaw * pitch)
    }

    // TODO: doesn't take into account time delta.
    pub fn update(&mut self) {
        let rot = self.rot_mat();
        self.pos += (rot * Vec4::from((self.vel * 0.5, 0.))).xyz();
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key),
                        state,
                        ..
                    },
                ..
            } => match state {
                ElementState::Pressed => match key {
                    KeyCode::KeyW => self.vel.z = -1.,
                    KeyCode::KeyS => self.vel.z = 1.,
                    KeyCode::KeyA => self.vel.x = -1.,
                    KeyCode::KeyD => self.vel.x = 1.,
                    KeyCode::ShiftLeft => self.vel.y = -1.,
                    KeyCode::ControlLeft => self.vel.y = 1.,
                    _ => (),
                },
                ElementState::Released => match key {
                    KeyCode::KeyW => self.vel.z = 0.,
                    KeyCode::KeyS => self.vel.z = 0.,
                    KeyCode::KeyA => self.vel.x = 0.,
                    KeyCode::KeyD => self.vel.x = 0.,
                    KeyCode::ShiftLeft => self.vel.y = 0.,
                    KeyCode::ControlLeft => self.vel.y = 0.,
                    _ => (),
                },
            },
            WindowEvent::MouseInput {
                button: MouseButton::Right,
                state: ElementState::Pressed,
                ..
            } => {
                self.pitch = 0.;
                self.yaw = 0.;
            }
            _ => (),
        }
    }

    pub fn on_mouse_motion(&mut self, (delta_x, delta_y): (f64, f64), cursor_grabbed: bool) {
        if cursor_grabbed {
            self.yaw += delta_x as f32 / 200.;
            self.pitch -= delta_y as f32 / 200.;
        }
    }

    pub fn world_to_sight(&self, _point: Vec3) -> Vec3 {
        todo!();
    }
}
