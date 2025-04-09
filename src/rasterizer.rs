use std::ops::DerefMut;

use winit::dpi::PhysicalSize;

use crate::{
    maths::{Vec2f, Vec3f},
    scene::{Camera, Triangle, Vertice, World},
};

fn world_to_raster(p_world: &Vec3f, cam: &Camera, size: &PhysicalSize<u32>) -> Vec3f {
    let p_cam = *p_world - cam.pos;
    let p_screen = Vec3f {
        x: p_cam.x * cam.z_near / -p_cam.z,
        y: p_cam.y * cam.z_near / -p_cam.z,
        z: -p_cam.z,
    };
    // [-1,1]
    let p_ndc = Vec2f {
        x: p_screen.x / cam.canvas_side,
        y: p_screen.y / cam.canvas_side,
    };
    // [0,1]
    Vec3f {
        x: (p_ndc.x + 1.) / 2. * (size.width as f64),
        y: (1. - p_ndc.y) / 2. * (size.height as f64),
        z: p_screen.z,
    }
}

fn world_to_raster_vertice(vertice: &Vertice, cam: &Camera, size: &PhysicalSize<u32>) -> Vertice {
    Vertice {
        pos: world_to_raster(&vertice.pos, cam, size),
        color: vertice.color,
    }
}

fn world_to_raster_triangle(
    triangle: &Triangle,
    cam: &Camera,
    size: &PhysicalSize<u32>,
) -> Triangle {
    Triangle {
        p0: world_to_raster_vertice(&triangle.p0, cam, size),
        p1: world_to_raster_vertice(&triangle.p1, cam, size),
        p2: world_to_raster_vertice(&triangle.p2, cam, size),
    }
}

fn buffer_index(p: Vec3f, size: &PhysicalSize<u32>) -> Option<usize> {
    if p.x >= 0. && p.x < (size.width as f64) && p.y >= 0. && p.y < (size.height as f64) {
        Some(p.x as usize + p.y as usize * size.width as usize)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

fn bounding_box_triangle(t: &Triangle, size: &PhysicalSize<u32>) -> Rect {
    let min_x =
        (f64::min(f64::min(t.p0.pos.x, t.p1.pos.x), t.p2.pos.x) as u32).clamp(0, size.width - 1);
    let max_x =
        (f64::max(f64::max(t.p0.pos.x, t.p1.pos.x), t.p2.pos.x) as u32).clamp(0, size.width - 1);
    let min_y =
        (f64::min(f64::min(t.p0.pos.y, t.p1.pos.y), t.p2.pos.y) as u32).clamp(0, size.width - 1);
    let max_y =
        (f64::max(f64::max(t.p0.pos.y, t.p1.pos.y), t.p2.pos.y) as u32).clamp(0, size.width - 1);

    Rect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}

fn edge_function(tri_a: Vec3f, tri_b: Vec3f, p: Vec3f) -> f64 {
    let ab = tri_b - tri_a;
    let ap = p - tri_a;

    ap.x * ab.y - ap.y * ab.x
}

fn draw_vertice_basic<B: DerefMut<Target = [u32]>>(
    buffer: &mut B,
    size: &PhysicalSize<u32>,
    v: &Vertice,
) {
    if v.pos.x >= 1.
        && v.pos.x < (size.width as f64) - 1.
        && v.pos.y >= 1.
        && v.pos.y < (size.height as f64) - 1.
    {
        if let Some(i) = buffer_index(v.pos, size) {
            buffer[i] = v.color;
            buffer[i - 1] = v.color;
            buffer[i + 1] = v.color;
            buffer[i - (size.width as usize)] = v.color;
            buffer[i + (size.width as usize)] = v.color;
        }
    }
}

fn rasterize_triangle<B: DerefMut<Target = [u32]>>(
    triangle: &Triangle,
    buffer: &mut B,
    cam: &Camera,
    size: &PhysicalSize<u32>,
) {
    let tri_raster = world_to_raster_triangle(triangle, cam, size);

    let bb = dbg!(bounding_box_triangle(&tri_raster, size));
    (bb.x..(bb.x + bb.width))
        .flat_map(|x| {
            (bb.y..(bb.y + bb.height)).map(move |y| Vec3f {
                x: x as f64,
                y: y as f64,
                z: 0.,
            })
        })
        .for_each(|pixel| {
            if edge_function(tri_raster.p0.pos, tri_raster.p1.pos, pixel) >= 0.
                && edge_function(tri_raster.p1.pos, tri_raster.p2.pos, pixel) >= 0.
                && edge_function(tri_raster.p2.pos, tri_raster.p0.pos, pixel) >= 0.
            {
                buffer[(pixel.x + pixel.y * size.width as f64) as usize] = tri_raster.p1.color;
            }
        });

    draw_vertice_basic(buffer, size, &tri_raster.p0);
    draw_vertice_basic(buffer, size, &tri_raster.p1);
    draw_vertice_basic(buffer, size, &tri_raster.p2);
}

pub fn rasterize<B: DerefMut<Target = [u32]>>(
    world: &World,
    buffer: &mut B,
    size: &PhysicalSize<u32>,
) {
    world
        .triangles
        .iter()
        .for_each(|f| rasterize_triangle(f, buffer, &world.camera, size));
}
