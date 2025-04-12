use crate::maths::{Rotation, Vec3f};

#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub pos: Vec3f,
    pub color: u32,
}

impl Vertex {
    pub fn new(x: f64, y: f64, z: f64, color: u32) -> Self {
        Self {
            pos: Vec3f::new(x, y, z),
            color,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Triangle {
    pub p0: Vertex,
    pub p1: Vertex,
    pub p2: Vertex,
}

impl Default for Triangle {
    fn default() -> Self {
        Triangle::new(
            Vertex::new(0., 1., -12., 0xffff0000),
            Vertex::new(0., 0., -10., 0xff00ff00),
            Vertex::new(0., 0., -14., 0xff0000ff),
        )
    }
}

impl Triangle {
    fn new(p0: Vertex, p1: Vertex, p2: Vertex) -> Self {
        Triangle { p0, p1, p2 }
    }

    pub fn min_z(&self) -> f64 {
        f64::min(self.p0.pos.z, f64::min(self.p1.pos.z, self.p2.pos.z))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub pos: Vec3f,
    pub z_near: f64,
    pub canvas_side: f64,
    pub rot: Rotation,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            pos: Vec3f::new(1., 1., 0.),
            z_near: 0.5,
            canvas_side: 0.1,
            rot: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct World {
    pub triangles: Vec<Triangle>,
    pub camera: Camera,
}

impl Default for World {
    fn default() -> Self {
        World {
            triangles: vec![
                Triangle::default(),
                Triangle::new(
                    Vertex::new(3., 0., -19., 0xffff0000),
                    Vertex::new(4., 0., -19., 0xffff0000),
                    Vertex::new(4., 1., -10., 0xffff0000),
                ),
                Triangle::new(
                    Vertex::new(4., 0., -19., 0xffff0000),
                    Vertex::new(5., 0., -19., 0xffff0000),
                    Vertex::new(4., 1., -10., 0xffff0000),
                ),
                Triangle::new(
                    Vertex::new(3., 2., -19., 0xff0000ff),
                    Vertex::new(4., 1., -10., 0xff0000ff),
                    Vertex::new(4., 2., -19., 0xff0000ff),
                ),
                Triangle::new(
                    Vertex::new(4., 1., -10., 0xff0000ff),
                    Vertex::new(5., 2., -19., 0xff0000ff),
                    Vertex::new(4., 2., -19., 0xff0000ff),
                ),
                Triangle::new(
                    Vertex::new(3., 0., -19., 0xff00ff00),
                    Vertex::new(4., 1., -10., 0xff00ff00),
                    Vertex::new(3., 1., -19., 0xff00ff00),
                ),
                Triangle::new(
                    Vertex::new(3., 2., -19., 0xff00ff00),
                    Vertex::new(3., 1., -19., 0xff00ff00),
                    Vertex::new(4., 1., -10., 0xff00ff00),
                ),
                Triangle::new(
                    Vertex::new(5., 1., -19., 0xffffff00),
                    Vertex::new(4., 1., -10., 0xffffff00),
                    Vertex::new(5., 0., -19., 0xffffff00),
                ),
                Triangle::new(
                    Vertex::new(4., 1., -10., 0xffffff00),
                    Vertex::new(5., 1., -19., 0xffffff00),
                    Vertex::new(5., 2., -19., 0xffffff00),
                ),
                Triangle::new(
                    Vertex::new(2., 0.5, -19., 0xff00ffff),
                    Vertex::new(4., 0.5, -15., 0xff00ffff),
                    Vertex::new(2., 1.5, -19., 0xff00ffff),
                ),
                Triangle::new(
                    Vertex::new(4., 0.5, -15., 0xff00ffff),
                    Vertex::new(4., 1.5, -15., 0xff00ffff),
                    Vertex::new(2., 1.5, -19., 0xff00ffff),
                ),
                Triangle::new(
                    Vertex::new(3.7, 0.7, -12., 0xffff00ff),
                    Vertex::new(4.3, 0.7, -12., 0xffff00ff),
                    Vertex::new(3.7, 1.3, -12., 0xffff00ff),
                ),
                Triangle::new(
                    Vertex::new(4.3, 0.7, -12., 0xffff00ff),
                    Vertex::new(4.3, 1.3, -12., 0xffff00ff),
                    Vertex::new(3.7, 1.3, -12., 0xffff00ff),
                ),
            ],
            camera: Default::default(),
        }
    }
}
