use std::{
    fmt,
    ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign},
};

pub use std::f32::consts::PI;

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct Vec3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3f {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn norm(self) -> f32 {
        f32::sqrt(self.x * self.x + self.y * self.y + self.z * self.z)
    }

    pub fn normalize(mut self) -> Self {
        let norm = self.norm();
        if norm != 1. {
            self.x /= norm;
            self.y /= norm;
            self.z /= norm;
        }
        self
    }

    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Self) -> Vec3f {
        Vec3f {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }

    /// 1. Scale
    /// 2. Rotate around (0.0.0) axis
    /// 3. Move along given vector
    pub fn scale_rot_move(self, scale: f32, new_base: &Rotation, move_vect: Vec3f) -> Self {
        (self * scale) * new_base + move_vect
    }
}

impl Add for Vec3f {
    type Output = Self;

    fn add(mut self, other: Self) -> Self::Output {
        self.x += other.x;
        self.y += other.y;
        self.z += other.z;
        self
    }
}

impl AddAssign for Vec3f {
    fn add_assign(&mut self, other: Self) {
        self.x += other.x;
        self.y += other.y;
        self.z += other.z;
    }
}

impl Sub for Vec3f {
    type Output = Self;

    fn sub(mut self, other: Self) -> Self::Output {
        self.x -= other.x;
        self.y -= other.y;
        self.z -= other.z;
        self
    }
}

impl SubAssign for Vec3f {
    fn sub_assign(&mut self, other: Self) {
        self.x -= other.x;
        self.y -= other.y;
        self.z -= other.z;
    }
}

impl Mul<f32> for Vec3f {
    type Output = Self;

    fn mul(mut self, other: f32) -> Self::Output {
        self.x *= other;
        self.y *= other;
        self.z *= other;
        self
    }
}

impl Mul<&Rotation> for Vec3f {
    type Output = Self;

    fn mul(self, other: &Rotation) -> Self::Output {
        Self {
            x: self.x * other.u.x + self.y * other.v.x + self.z * other.w.x,
            y: self.x * other.u.y + self.y * other.v.y + self.z * other.w.y,
            z: self.x * other.u.z + self.y * other.v.z + self.z * other.w.z,
        }
    }
}

impl Neg for Vec3f {
    type Output = Self;

    fn neg(mut self) -> Self::Output {
        self.x = -self.x;
        self.y = -self.y;
        self.z = -self.z;
        self
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Vec4u {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vec4u {
    pub fn from_color_u32(c: u32) -> Self {
        Vec4u {
            x: (c >> 24) as f32,
            y: ((c >> 16) & 0xff) as f32,
            z: ((c >> 8) & 0xff) as f32,
            w: (c & 0xff) as f32,
        }
    }

    pub fn as_color_u32(&self) -> u32 {
        ((self.x as u32) << 24) | ((self.y as u32) << 16) | ((self.z as u32) << 8) | (self.w as u32)
    }
}

impl Add for Vec4u {
    type Output = Self;

    fn add(mut self, other: Self) -> Self::Output {
        self.x += other.x;
        self.y += other.y;
        self.z += other.z;
        self.w += other.w;
        self
    }
}

impl Sub for Vec4u {
    type Output = Self;

    fn sub(mut self, other: Self) -> Self::Output {
        self.x -= other.x;
        self.y -= other.y;
        self.z -= other.z;
        self.w -= other.w;
        self
    }
}

impl Mul<f32> for Vec4u {
    type Output = Self;

    fn mul(mut self, other: f32) -> Self::Output {
        self.x *= other;
        self.y *= other;
        self.z *= other;
        self.w *= other;
        self
    }
}

impl MulAssign<f32> for Vec3f {
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
        self.z *= rhs;
    }
}

impl Div<f32> for Vec4u {
    type Output = Self;

    fn div(mut self, other: f32) -> Self::Output {
        self.x /= other;
        self.y /= other;
        self.z /= other;
        self.w /= other;
        self
    }
}
impl MulAssign<f32> for Vec4u {
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
        self.z *= rhs;
        self.w *= rhs;
    }
}

impl fmt::Display for Vec3f {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:5.2},{:5.2},{:5.2})", self.x, self.y, self.z)
    }
}

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

    pub fn u(&self) -> Vec3f {
        self.u
    }

    pub fn v(&self) -> Vec3f {
        self.v
    }

    pub fn w(&self) -> Vec3f {
        self.w
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
