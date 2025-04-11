use crate::maths::{Rotation, Vec3f};

#[derive(Debug, Clone, Copy)]
pub struct Vertice {
    pub pos: Vec3f,
    pub color: u32,
}

impl Vertice {
    pub fn new(x: f64, y: f64, z: f64, color: u32) -> Self {
        Self {
            pos: Vec3f::new(x, y, z),
            color,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Triangle {
    pub p0: Vertice,
    pub p1: Vertice,
    pub p2: Vertice,
}

impl Default for Triangle {
    fn default() -> Self {
        Triangle::new(
            Vertice::new(0., 1., -12., 0xffff0000),
            Vertice::new(0., 0., -10., 0xff00ff00),
            Vertice::new(0., 0., -14., 0xff0000ff),
        )
    }
}

impl Triangle {
    fn new(p0: Vertice, p1: Vertice, p2: Vertice) -> Self {
        Triangle { p0, p1, p2 }
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
                    Vertice::new(3., 0., -19., 0xffff0000),
                    Vertice::new(4., 0., -19., 0xffff0000),
                    Vertice::new(4., 1., -10., 0xffff0000),
                ),
                Triangle::new(
                    Vertice::new(4., 0., -19., 0xffff0000),
                    Vertice::new(5., 0., -19., 0xffff0000),
                    Vertice::new(4., 1., -10., 0xffff0000),
                ),
                Triangle::new(
                    Vertice::new(3., 2., -19., 0xff0000ff),
                    Vertice::new(4., 1., -10., 0xff0000ff),
                    Vertice::new(4., 2., -19., 0xff0000ff),
                ),
                Triangle::new(
                    Vertice::new(4., 1., -10., 0xff0000ff),
                    Vertice::new(5., 2., -19., 0xff0000ff),
                    Vertice::new(4., 2., -19., 0xff0000ff),
                ),
                Triangle::new(
                    Vertice::new(3., 0., -19., 0xff00ff00),
                    Vertice::new(4., 1., -10., 0xff00ff00),
                    Vertice::new(3., 1., -19., 0xff00ff00),
                ),
                Triangle::new(
                    Vertice::new(3., 2., -19., 0xff00ff00),
                    Vertice::new(3., 1., -19., 0xff00ff00),
                    Vertice::new(4., 1., -10., 0xff00ff00),
                ),
                Triangle::new(
                    Vertice::new(5., 1., -19., 0xffffff00),
                    Vertice::new(4., 1., -10., 0xffffff00),
                    Vertice::new(5., 0., -19., 0xffffff00),
                ),
                Triangle::new(
                    Vertice::new(4., 1., -10., 0xffffff00),
                    Vertice::new(5., 1., -19., 0xffffff00),
                    Vertice::new(5., 2., -19., 0xffffff00),
                ),
                Triangle::new(
                    Vertice::new(2., 0.5, -19., 0xff00ffff),
                    Vertice::new(4., 0.5, -15., 0xff00ffff),
                    Vertice::new(2., 1.5, -19., 0xff00ffff),
                ),
                Triangle::new(
                    Vertice::new(4., 0.5, -15., 0xff00ffff),
                    Vertice::new(4., 1.5, -15., 0xff00ffff),
                    Vertice::new(2., 1.5, -19., 0xff00ffff),
                ),
                Triangle::new(
                    Vertice::new(0.7, 0.7, -12., 0xffff00ff),
                    Vertice::new(1.3, 0.7, -12., 0xffff00ff),
                    Vertice::new(0.7, 1.3, -12., 0xffff00ff),
                ),
                Triangle::new(
                    Vertice::new(1.3, 0.7, -12., 0xffff00ff),
                    Vertice::new(1.3, 1.3, -12., 0xffff00ff),
                    Vertice::new(0.7, 1.3, -12., 0xffff00ff),
                ),
            ],
            camera: Default::default(),
        }
    }
}
