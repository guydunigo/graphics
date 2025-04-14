use std::ops::{Add, Div, Mul, Neg, Sub};

#[derive(Debug, Clone, Copy)]
pub struct Vec2f {
    pub x: f64,
    pub y: f64,
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Vec3f {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3f {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /*
    pub fn norm(&self) -> f64 {
        f64::sqrt(self.x * self.x + self.y * self.y + self.z * self.z)
    }

    pub fn normalize(&mut self) {
        let norm = self.norm();
        if norm != 1. {
            self.x /= norm;
            self.y /= norm;
            self.z /= norm;
        }
    }
    */

    pub fn rotate(self, new_base: Rotation) -> Self {
        Self {
            x: self.x * new_base.u.x + self.y * new_base.v.x + self.z * new_base.w.x,
            y: self.x * new_base.u.y + self.y * new_base.v.y + self.z * new_base.w.y,
            z: self.x * new_base.u.z + self.y * new_base.v.z + self.z * new_base.w.z,
        }
    }
}

impl Add for Vec3f {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
}

impl Sub for Vec3f {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }
}

impl Neg for Vec3f {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Vec4u {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

impl Vec4u {
    pub fn from_color_u32(c: u32) -> Self {
        Vec4u {
            x: (c >> 24) as f64,
            y: ((c >> 16) & 0xff) as f64,
            z: ((c >> 8) & 0xff) as f64,
            w: (c & 0xff) as f64,
        }
    }

    pub fn as_color_u32(&self) -> u32 {
        ((self.x as u32) << 24) | ((self.y as u32) << 16) | ((self.z as u32) << 8) | (self.w as u32)
    }
}

impl Add for Vec4u {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
            w: self.w + other.w,
        }
    }
}

impl Sub for Vec4u {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
            w: self.w - other.w,
        }
    }
}

impl Mul<f64> for Vec4u {
    type Output = Self;

    fn mul(self, other: f64) -> Self {
        Self {
            x: self.x * other,
            y: self.y * other,
            z: self.z * other,
            w: self.w * other,
        }
    }
}

impl Div<f64> for Vec4u {
    type Output = Self;

    fn div(self, other: f64) -> Self {
        Self {
            x: self.x / other,
            y: self.y / other,
            z: self.z / other,
            w: self.w / other,
        }
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
    pub fn from_angles(angles: &Vec3f) -> Self {
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
