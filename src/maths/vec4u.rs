use std::{
    fmt,
    iter::Sum,
    ops::{Add, Div, Mul, MulAssign, Sub},
};

use super::Vec3f;

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

impl Sum for Vec4u {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Vec4u::default(), |a, b| a + b)
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
