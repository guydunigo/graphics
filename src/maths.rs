use std::ops::{Add, Sub};

#[derive(Debug, Clone, Copy)]
struct Vec2f {
    pub x: f64,
    pub y: f64,
}

impl Vec2f {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Vec3f {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3f {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn world_to_raster(
        &self,
        cam_p: Vec3f,
        z_near: f64,
        canvas_side: f64,
        screen_width: u32,
        screen_height: u32,
    ) -> Self {
        let p_cam = *self - cam_p;
        let p_screen = Vec3f {
            x: p_cam.x * z_near / -p_cam.z,
            y: p_cam.y * z_near / -p_cam.z,
            z: -p_cam.z,
        };
        // [-1,1]
        let p_ndc = Vec2f {
            x: p_screen.x / canvas_side,
            y: p_screen.y / canvas_side,
        };
        // [0,1]
        Self {
            x: (p_ndc.x + 1.) / 2. * (screen_width as f64),
            y: (1. - p_ndc.y) / 2. * (screen_height as f64),
            z: p_screen.z,
        }
    }

    pub fn buffer_index(&self, width: u32, height: u32) -> Option<usize> {
        if self.x >= 0. && self.x < (width as f64) && self.y >= 0. && self.y < (height as f64) {
            Some(self.x.round() as usize + self.y.round() as usize * width as usize)
        } else {
            None
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
