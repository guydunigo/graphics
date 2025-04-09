use crate::maths::Vec3;

pub type Vec3d = Vec3<f64>;

pub struct Vertice {
    pub pos: Vec3d,
    pub color: u32,
}

impl Vertice {
    pub fn new(x: f64, y: f64, z: f64, color: u32) -> Self {
        Vertice {
            pos: Vec3d::new(x, y, z),
            color,
        }
    }
}

pub struct Triangle {
    pub p0: Vertice,
    pub p1: Vertice,
    pub p2: Vertice,
}

pub struct Camera {
    pub pos: Vec3d,
    pub dir: Vec3d,
    // TODO: not focale
    // pub focale: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            pos: Vec3d::new(0., 1., 0.),
            dir: Vec3d::new(0., 0., 1.),
            // focale: 1.,
        }
    }
}

pub struct World {
    pub faces: Vec<Triangle>,
    pub camera: Camera,
}

impl Default for World {
    fn default() -> Self {
        let t0 = Triangle {
            p0: Vertice::new(1., 1., 10., 0xffff0000),
            p1: Vertice::new(0., 1., 10., 0xff00ff00),
            p2: Vertice::new(0., 0., 12., 0xff0000ff),
        };
        World {
            faces: vec![t0],
            camera: Default::default(),
        }
    }
}
