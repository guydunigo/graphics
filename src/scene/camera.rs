use crate::maths::{Rotation, Vec3f};

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub pos: Vec3f,
    pub z_near: f32,
    pub canvas_side: f32,
    /// Rotation matrix that will turn objects based on sight.
    /// It is made of opposite angles : if I turn to the left,
    /// the objects move to the right in my vision.
    ///
    /// This is the inverse of the actual rotation matrix of the camera "object" : `rot * sight_rot == identity`. See [`rot()`]
    ///
    /// We store this one because it is easier to manipulate and more used.
    sight_rot: Rotation,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            pos: Vec3f::new(1., 1., 12.),
            z_near: 0.5,
            canvas_side: 0.1,
            sight_rot: Default::default(),
        }
    }
}

impl Camera {
    const MOVE_STEP: f32 = 0.1;
    const ROT_STEP: f32 = 0.001;

    /// Rotation of the camera "object".
    ///
    /// The camera points towards `-rot().w` and the "up" is `rot().v`.
    ///
    /// It is calculated from [`sight_rot`] (inverse matrix), because it is only needed for
    /// movement.
    pub fn rot(&self) -> Rotation {
        self.sight_rot().inv()
    }

    pub fn sight_rot(&self) -> &Rotation {
        &self.sight_rot
    }

    pub fn reset_rot(&mut self) {
        self.sight_rot = Default::default();
    }

    pub fn rotate_from_mouse(&mut self, delta_x: f32, delta_y: f32) {
        // Objects rotate opposite direction from camera, so double negative.
        self.sight_rot = Rotation::from_angles(0., delta_x * Self::ROT_STEP, 0.)
            * &self.sight_rot
            * &Rotation::from_angles(delta_y * Self::ROT_STEP, 0., 0.);
    }

    /// Move along view direction
    /// `delta_x` : left->right
    /// `delta_y` : bottom->up
    /// `delta_z` : back->forward
    ///
    /// Z goes backwards so we reverse it.
    pub fn move_sight(&mut self, delta_x: f32, delta_y: f32, delta_z: f32) {
        let rot = self.rot();
        self.pos += (rot.u() * delta_x + rot.v() * delta_y - rot.w() * delta_z) * Self::MOVE_STEP;
    }

    pub fn world_to_sight(&self, point: Vec3f) -> Vec3f {
        (point - self.pos) * &self.sight_rot
    }
}
