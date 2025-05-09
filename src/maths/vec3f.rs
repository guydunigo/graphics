use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

use super::Rotation;

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

    pub fn cross(self, other: Self) -> Self {
        Vec3f {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }

    pub fn cross_z(self, other: Self) -> f32 {
        self.x * other.y - self.y * other.x
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
            x: self.x * other.u().x + self.y * other.v().x + self.z * other.w().x,
            y: self.x * other.u().y + self.y * other.v().y + self.z * other.w().y,
            z: self.x * other.u().z + self.y * other.v().z + self.z * other.w().z,
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
