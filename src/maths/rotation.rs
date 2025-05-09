use super::Vec3f;
use std::{
    fmt,
    ops::{Mul, MulAssign},
};

/// Rotation matrix that can be used to rotate vectors and other matrices.
///
/// It is made of the unitary vectors of the new rotated base.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rotation {
    u: Vec3f,
    v: Vec3f,
    w: Vec3f,
}

impl Rotation {
    pub fn u(&self) -> Vec3f {
        self.u
    }

    pub fn v(&self) -> Vec3f {
        self.v
    }

    pub fn w(&self) -> Vec3f {
        self.w
    }

    /// Rotation around (in order) : y axis, x axis (yaw), z axis (roll).
    pub fn from_angles(x: f32, y: f32, z: f32) -> Self {
        let x_cos = x.cos();
        let x_sin = x.sin();
        let y_cos = y.cos();
        let y_sin = y.sin();
        let z_cos = z.cos();
        let z_sin = z.sin();

        // let rot_y = Self {
        //     u: Vec3f::new(y_cos, 0., -y_sin),
        //     v: Vec3f::new(0., 1., 0.),
        //     w: Vec3f::new(y_sin, 0., y_cos),
        // };
        // let rot_x = Self {
        //     u: Vec3f::new(1., 0., 0.),
        //     v: Vec3f::new(0., x_cos, x_sin),
        //     w: Vec3f::new(0., -x_sin, x_cos),
        // };
        // let rot_z = Self {
        //     u: Vec3f::new(z_cos, z_sin, 0.),
        //     v: Vec3f::new(-z_sin, z_cos, 0.),
        //     w: Vec3f::new(0., 0., 1.),
        // };
        // let mult = rot_z * &rot_x * &rot_y;

        // rot_x          * rot_y          =
        // 1 0     0      | y_cos  0 y_sin | y_cos  x_sin*y_sin x_cos*y_sin
        // 0 x_cos -x_sin | 0      1 0     | 0      x_cos       -x_sin
        // 0 x_sin x_cos  | -y_sin 0 y_cos | -y_sin x_sin*y_cos x_cos*y_cos
        //
        // rot_z          * (rot_x * rot_y)                =
        // z_cos -z_sin 0 | y_cos  x_sin*y_sin x_cos*y_sin
        // z_sin z_cos  0 | 0      x_cos       -x_sin
        // 0     0      1 | -y_sin x_sin*y_cos x_cos*y_cos
        //
        // z_cos*y_cos + z_sin*x_sin*y_sin    -z_sin*y_cos+z_cos*x_sin*y_sin    x_cos*y_sin
        // z_sin*x_cos                        z_cos*x_cos                       -x_sin
        // z_cos * -y_sin + z_sin*x_sin*y_cos -z_sin*-y_sin + z_cos*x_sin*y_cos x_cos*y_cos
        //
        Self {
            u: Vec3f::new(
                z_cos * y_cos + z_sin * x_sin * y_sin,
                z_sin * x_cos,
                z_cos * -y_sin + z_sin * x_sin * y_cos,
            ),
            v: Vec3f::new(
                -z_sin * y_cos + z_cos * x_sin * y_sin,
                z_cos * x_cos,
                -z_sin * -y_sin + z_cos * x_sin * y_cos,
            ),
            w: Vec3f::new(x_cos * y_sin, -x_sin, x_cos * y_cos),
        }
    }

    /// Identity matrix
    pub fn id() -> Self {
        Self {
            u: Vec3f::new(1., 0., 0.),
            v: Vec3f::new(0., 1., 0.),
            w: Vec3f::new(0., 0., 1.),
        }
    }

    /// Matrixes determinent
    pub fn det(&self) -> f32 {
        self.u.x * self.v.y * self.w.z
            + self.v.x * self.w.y * self.u.z
            + self.w.x * self.u.y * self.v.z
            - self.w.x * self.v.y * self.u.z
            - self.w.y * self.v.z * self.u.x
            - self.w.z * self.v.x * self.u.y
    }

    /// Inverse matrix
    pub fn inv(&self) -> Self {
        Self {
            u: Vec3f::new(
                self.v.y * self.w.z - self.w.y * self.v.z,
                self.w.y * self.u.z - self.u.y * self.w.z,
                self.u.y * self.v.z - self.v.y * self.u.z,
            ),
            v: Vec3f::new(
                self.w.x * self.v.z - self.v.x * self.w.z,
                self.u.x * self.w.z - self.u.z * self.w.x,
                self.v.x * self.u.z - self.u.x * self.v.z,
            ),
            w: Vec3f::new(
                self.v.x * self.w.y - self.v.y * self.w.x,
                self.w.x * self.u.y - self.u.x * self.w.y,
                self.u.x * self.v.y - self.v.x * self.u.y,
            ),
        } * self.det()
    }
}

impl Default for Rotation {
    fn default() -> Self {
        Self::id()
    }
}

impl Mul<&Self> for Rotation {
    type Output = Self;

    fn mul(self, other: &Self) -> Self::Output {
        Self {
            u: self.u * other,
            v: self.v * other,
            w: self.w * other,
        }
    }
}

impl MulAssign<&Rotation> for Rotation {
    fn mul_assign(&mut self, other: &Rotation) {
        *self = *self * other;
    }
}

impl Mul<f32> for Rotation {
    type Output = Self;

    fn mul(mut self, other: f32) -> Self::Output {
        self.u *= other;
        self.v *= other;
        self.w *= other;
        self
    }
}

impl fmt::Display for Rotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:5.2} {:5.2} {:5.2}\n{:5.2} {:5.2} {:5.2}\n{:5.2} {:5.2} {:5.2}",
            (self.u.x * 100.).round() / 100.,
            (self.v.x * 100.).round() / 100.,
            (self.w.x * 100.).round() / 100.,
            (self.u.y * 100.).round() / 100.,
            (self.v.y * 100.).round() / 100.,
            (self.w.y * 100.).round() / 100.,
            (self.u.z * 100.).round() / 100.,
            (self.v.z * 100.).round() / 100.,
            (self.w.z * 100.).round() / 100.,
        )
    }
}
