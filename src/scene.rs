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

    pub fn world_to_raster(&self, cam: Camera, screen_width: u32, screen_height: u32) -> Self {
        Self {
            pos: self.pos.world_to_raster(
                cam.pos,
                cam.z_near,
                cam.canvas_side,
                screen_width,
                screen_height,
            ),
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
    pub fn world_to_raster(&self, cam: Camera, screen_width: u32, screen_height: u32) -> Self {
        Triangle {
            p0: self.p0.world_to_raster(cam, screen_width, screen_height),
            p1: self.p1.world_to_raster(cam, screen_width, screen_height),
            p2: self.p2.world_to_raster(cam, screen_width, screen_height),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub pos: Vec3f,
    pub z_near: f64,
    pub canvas_side: f64,
    // Pour commencer, on fixe le regard selon Z qui diminue.
    // TODO: matrice 4x4 : missing double angle (autours + débullé)
    // pub dir: Vec3f,
    // TODO: not focale
    // pub focale: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            pos: Vec3f::new(1., 1., 0.),
            z_near: -0.5,
            canvas_side: 0.1,
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
        World {
            faces: vec![Triangle::default()],
            camera: Default::default(),
        }
    }
}

impl World {
    pub fn world_to_raster(&self, screen_width: u32, screen_height: u32) -> Vec<Triangle> {
        self.faces
            .iter()
            .map(|f| f.world_to_raster(self.camera, screen_width, screen_height))
            .collect()
    }
}
