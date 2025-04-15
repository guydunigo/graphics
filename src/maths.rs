use std::ops::{Add, Div, Mul, Neg, Sub};

#[derive(Debug, Clone, Copy)]
pub struct Vec2f {
    pub x: f32,
    pub y: f32,
}

#[derive(Default, Debug, Clone, Copy)]
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

    pub fn rotate(self, new_base: &Rotation) -> Self {
        Self {
            x: self.x * new_base.u.x + self.y * new_base.v.x + self.z * new_base.w.x,
            y: self.x * new_base.u.y + self.y * new_base.v.y + self.z * new_base.w.y,
            z: self.x * new_base.u.z + self.y * new_base.v.z + self.z * new_base.w.z,
        }
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

    pub fn trans_rot_scale(self, pos: Vec3f, rot: &Rotation, scale: f32) -> Self {
        (self + pos).rotate(rot) * scale
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

impl Sub for Vec3f {
    type Output = Self;

    fn sub(mut self, other: Self) -> Self::Output {
        self.x -= other.x;
        self.y -= other.y;
        self.z -= other.z;
        self
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

#[derive(Debug, Clone, Copy)]
pub struct Rotation {
    pub u: Vec3f,
    pub v: Vec3f,
    pub w: Vec3f,
}

impl Default for Rotation {
    fn default() -> Self {
        Self {
            u: Vec3f::new(1., 0., 0.),
            v: Vec3f::new(0., 1., 0.),
            w: Vec3f::new(0., 0., 1.),
        }
    }
}

impl Rotation {
    /// Rotation around x axis, y axis, z axis.
    pub fn from_angles(angles: Vec3f) -> Self {
        let x_cos = angles.x.cos();
        let x_sin = angles.x.sin();
        let y_cos = angles.y.cos();
        let y_sin = angles.y.sin();
        let z_cos = angles.z.cos();
        let z_sin = angles.z.sin();

        Self {
            u: Vec3f::new(y_cos * z_cos, z_sin, -y_sin),
            v: Vec3f::new(-z_sin, x_cos * z_cos, x_sin),
            w: Vec3f::new(y_sin, -x_sin, x_cos * y_cos),
        }
    }
}
