mod iterator;
mod original;

use std::ops::{Deref, DerefMut};

use iterator::EngineIterator;
use winit::dpi::PhysicalSize;

use crate::{
    font::TextWriter,
    maths::{Vec3f, Vec4u},
    scene::{Camera, Texture, Triangle, World},
    window::App,
};

const MINIMAL_AMBIANT_LIGHT: f32 = 0.2;

#[cfg(feature = "stats")]
#[derive(Default, Debug, Clone)]
pub struct Stats {
    pub nb_triangles_tot: usize,
    pub nb_triangles_sight: usize,
    pub nb_triangles_facing: usize,
    pub nb_triangles_drawn: usize,
    pub nb_pixels_tested: usize,
    pub nb_pixels_in: usize,
    pub nb_pixels_front: usize,
    pub nb_pixels_written: usize,
    // pub misc: String,
}

#[derive(Default, Debug, Clone)]
pub struct Rasterizer {
    /// Over-print all vertices
    pub show_vertices: bool,
    pub engine: AnyEngine,
    /// Sort triangles by point with mininum Z value
    pub sort_triangles: TriangleSorting,
    text_writer: TextWriter,
}

impl Rasterizer {
    pub fn rasterize<B: DerefMut<Target = [u32]>>(
        &self,
        world: &World,
        buffer: &mut B,
        depth_buffer: &mut [f32],
        size: PhysicalSize<u32>,
        #[cfg(feature = "stats")] stats: &mut Stats,
    ) {
        self.engine.rasterize(
            world,
            buffer,
            depth_buffer,
            size,
            #[cfg(feature = "stats")]
            stats,
        );
    }
}

trait Engine {
    fn rasterize<B: DerefMut<Target = [u32]>>(app: &App, buffer: &mut B, size: PhysicalSize<u32>);
}

#[derive(Default, Debug, Clone, Copy)]
pub enum TriangleSorting {
    #[default]
    None,
    BackToFront,
    FrontToBack,
}

impl TriangleSorting {
    pub fn next(&mut self) {
        match self {
            TriangleSorting::None => *self = TriangleSorting::BackToFront,
            TriangleSorting::BackToFront => *self = TriangleSorting::FrontToBack,
            TriangleSorting::FrontToBack => *self = TriangleSorting::None,
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub enum AnyEngine {
    Original(EngineOriginal),
    #[default]
    Iterator(EngineIterator),
}

impl AnyEngine {
    pub fn next(&mut self) {
        match self {
            AnyEngine::Original(_) => *self = AnyEngine::Iterator(Default::default()),
            AnyEngine::Iterator(_) => *self = AnyEngine::Original(Default::default()),
        }
    }
}

impl<T: Engine> Deref for AnyEngine {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            AnyEngine::Original(e) => e,
            AnyEngine::Iterator(e) => e,
        }
    }
}

fn world_to_raster(p_world: Vec3f, cam: &Camera, size: PhysicalSize<u32>, ratio_w_h: f32) -> Vec3f {
    // Camera space
    let mut p = cam.world_to_sight(p_world);

    // Screen space : perspective correct
    if p.z < -0.001 {
        p.x *= cam.z_near / -p.z;
        p.y *= cam.z_near / -p.z;
    } else {
        // TODO: 0 divide getting too near the camera and reversing problem behind...
        p.x *= cam.z_near / 0.1;
        p.y *= cam.z_near / 0.1;
    };
    p.z = -p.z;

    // Near-Clipping-Plane
    // [-1,1]
    p.x /= cam.canvas_side;
    p.y /= cam.canvas_side;

    if size.width > size.height {
        p.x /= ratio_w_h;
    } else {
        p.y *= ratio_w_h;
    }

    // Raster space
    // [0,1]
    p.x = (p.x + 1.) / 2. * (size.width as f32);
    p.y = (1. - p.y) / 2. * (size.height as f32);

    p
}

fn world_to_raster_triangle(
    triangle: &Triangle,
    cam: &Camera,
    size: PhysicalSize<u32>,
    ratio_w_h: f32,
) -> Triangle {
    Triangle {
        p0: world_to_raster(triangle.p0, cam, size, ratio_w_h),
        p1: world_to_raster(triangle.p1, cam, size, ratio_w_h),
        p2: world_to_raster(triangle.p2, cam, size, ratio_w_h),
        texture: triangle.texture,
    }
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    pub min_x: u32,
    pub min_y: u32,
    pub max_x: u32,
    pub max_y: u32,
    pub max_z: f32,
}

fn bounding_box_triangle(t: &Triangle, size: PhysicalSize<u32>) -> Rect {
    Rect {
        min_x: (f32::min(f32::min(t.p0.x, t.p1.x), t.p2.x) as u32).clamp(0, size.width - 1),
        max_x: (f32::max(f32::max(t.p0.x, t.p1.x), t.p2.x) as u32).clamp(0, size.width - 1),
        min_y: (f32::min(f32::min(t.p0.y, t.p1.y), t.p2.y) as u32).clamp(0, size.height - 1),
        max_y: (f32::max(f32::max(t.p0.y, t.p1.y), t.p2.y) as u32).clamp(0, size.height - 1),
        max_z: f32::max(f32::max(t.p0.z, t.p1.z), t.p2.z),
    }
}

// Calculates the area of the parallelogram from vectors ab and ap
// Positive if p is "right" of ab
fn edge_function(ab: Vec3f, ap: Vec3f) -> f32 {
    ap.x * ab.y - ap.y * ab.x
}

fn buffer_index(p: Vec3f, size: PhysicalSize<u32>) -> Option<usize> {
    if p.x >= 0. && p.x < (size.width as f32) && p.y >= 0. && p.y < (size.height as f32) {
        Some(p.x as usize + p.y as usize * size.width as usize)
    } else {
        None
    }
}

fn draw_vertice_basic<B: DerefMut<Target = [u32]>>(
    buffer: &mut B,
    size: PhysicalSize<u32>,
    v: Vec3f,
    texture: &Texture,
) {
    if v.x >= 1. && v.x < (size.width as f32) - 1. && v.y >= 1. && v.y < (size.height as f32) - 1. {
        if let Some(i) = buffer_index(v, size) {
            let color = match texture {
                Texture::Color(col) => *col,
                // TODO: Better color calculus
                Texture::VertexColor(c0, c1, c2) => ((Vec4u::from_color_u32(*c0)
                    + Vec4u::from_color_u32(*c1)
                    + Vec4u::from_color_u32(*c2))
                    / 3.)
                    .as_color_u32(),
            };

            buffer[i] = color;
            buffer[i - 1] = color;
            buffer[i + 1] = color;
            buffer[i - (size.width as usize)] = color;
            buffer[i + (size.width as usize)] = color;
        }
    }
}
