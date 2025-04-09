use crate::maths::Vec3f;

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

    pub fn world_to_raster(
        &self,
        cam_p: Vec3f,
        z_near: f64,
        canvas_side: f64,
        screen_width: f64,
        screen_height: f64,
    ) -> Self {
        Self {
            pos: self
                .pos
                .world_to_raster(cam_p, z_near, canvas_side, screen_width, screen_height),
            color: self.color,
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
        Triangle {
            p0: Vertice::new(1., 1., 10., 0xffff0000),
            p1: Vertice::new(0., 1., 10., 0xff00ff00),
            p2: Vertice::new(0., 0., 12., 0xff0000ff),
        }
    }
}

impl Triangle {
    pub fn world_to_raster(
        &self,
        cam_p: Vec3f,
        z_near: f64,
        canvas_side: f64,
        screen_width: f64,
        screen_height: f64,
    ) -> Self {
        Triangle {
            p0: self
                .p0
                .world_to_raster(cam_p, z_near, canvas_side, screen_width, screen_height),
            p1: self
                .p1
                .world_to_raster(cam_p, z_near, canvas_side, screen_width, screen_height),
            p2: self
                .p2
                .world_to_raster(cam_p, z_near, canvas_side, screen_width, screen_height),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub pos: Vec3f,
    pub dir: Vec3f,
    // TODO: not focale
    // pub focale: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            pos: Vec3f::new(0., 1., 0.),
            dir: Vec3f::new(0., 0., 1.),
            // focale: 1.,
        }
    }
}

#[derive(Debug, Clone)]
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
