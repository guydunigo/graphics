use crate::maths::{Rotation, Vec3f};

const DEFAULT_COLOR: u32 = 0xff999999;

#[derive(Debug, Clone, Copy)]
pub enum Texture {
    /// A simple color for the whole triangle
    Color(u32),
    /// A color per vertex in the same order :
    VertexColor(u32, u32, u32),
    // Texture, // TODO
}

impl Default for Texture {
    fn default() -> Self {
        Self::Color(DEFAULT_COLOR)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Triangle {
    pub p0: Vec3f,
    pub p1: Vec3f,
    pub p2: Vec3f,
    pub texture: Texture,
}

impl Default for Triangle {
    fn default() -> Self {
        Self {
            p0: Vec3f::new(0., 1., -2.),
            p1: Vec3f::new(0., 0., 0.),
            p2: Vec3f::new(0., 0., -4.),
            texture: Texture::VertexColor(0xffff0000, 0xff00ff00, 0xff0000ff),
        }
    }
}

impl Triangle {
    pub const fn new(p0: Vec3f, p1: Vec3f, p2: Vec3f, texture: Texture) -> Self {
        Triangle {
            p0,
            p1,
            p2,
            texture,
        }
    }

    pub fn min_z(&self) -> f32 {
        f32::min(self.p0.z, f32::min(self.p1.z, self.p2.z))
    }

    pub fn scale_rot_move(&self, scale: f32, rot: &Rotation, move_vect: Vec3f) -> Triangle {
        Triangle {
            p0: self.p0.scale_rot_move(scale, rot, move_vect),
            p1: self.p1.scale_rot_move(scale, rot, move_vect),
            p2: self.p2.scale_rot_move(scale, rot, move_vect),
            texture: self.texture,
        }
    }
}
