use std::{
    iter::Sum,
    ops::{Add, Div, Mul, MulAssign, Sub},
};

/// Each value : [0,255]
#[derive(Default, Debug, Clone, Copy)]
pub struct ColorF32 {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl ColorF32 {
    pub fn from_rgba([r, g, b, a]: [f32; 4]) -> Self {
        Self { r, g, b, a } * 255.
    }

    pub fn from_argb_u32(c: u32) -> Self {
        Self {
            a: (c >> 24) as f32,
            r: ((c >> 16) & 0xff) as f32,
            g: ((c >> 8) & 0xff) as f32,
            b: (c & 0xff) as f32,
        }
    }

    pub fn as_color_u32(&self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }
}

impl Add for ColorF32 {
    type Output = Self;

    fn add(mut self, other: Self) -> Self::Output {
        self.r += other.r;
        self.g += other.g;
        self.b += other.b;
        self.a += other.a;
        self
    }
}

impl Sum for ColorF32 {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(ColorF32::default(), |a, b| a + b)
    }
}

impl Sub for ColorF32 {
    type Output = Self;

    fn sub(mut self, other: Self) -> Self::Output {
        self.r -= other.r;
        self.g -= other.g;
        self.b -= other.b;
        self.a -= other.a;
        self
    }
}

impl Mul<f32> for ColorF32 {
    type Output = Self;

    fn mul(mut self, other: f32) -> Self::Output {
        self.r *= other;
        self.g *= other;
        self.b *= other;
        self.a *= other;
        self
    }
}

impl Div<f32> for ColorF32 {
    type Output = Self;

    fn div(mut self, other: f32) -> Self::Output {
        self.r /= other;
        self.g /= other;
        self.b /= other;
        self.a /= other;
        self
    }
}
impl MulAssign<f32> for ColorF32 {
    fn mul_assign(&mut self, rhs: f32) {
        self.r *= rhs;
        self.g *= rhs;
        self.b *= rhs;
        self.a *= rhs;
    }
}
