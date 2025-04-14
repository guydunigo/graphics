use std::ops::DerefMut;

use winit::dpi::PhysicalSize;

use crate::{
    maths::{Vec2f, Vec3f, Vec4u},
    scene::{Camera, Triangle, Vertex, World},
};

const SUN_DIRECTION: Vec3f = Vec3f::new(-1., -1., -1.);
const MINIMAL_AMBIANT_LIGHT: f64 = 0.1;

fn world_to_raster(p_world: &Vec3f, cam: &Camera, size: &PhysicalSize<u32>) -> Vec3f {
    let p_cam = (*p_world - cam.pos).rotate(cam.rot);
    let p_screen = if p_cam.z < -0.001 {
        Vec3f {
            x: p_cam.x * cam.z_near / -p_cam.z,
            y: p_cam.y * cam.z_near / -p_cam.z,
            z: -p_cam.z,
        }
    } else {
        // 0 divide getting too near the camera and reversing problem behind...
        Vec3f {
            x: p_cam.x * cam.z_near / 0.1,
            y: p_cam.y * cam.z_near / 0.1,
            z: -p_cam.z,
        }
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

fn world_to_raster_vertice(vertice: &Vertex, cam: &Camera, size: &PhysicalSize<u32>) -> Vertex {
    Vertex {
        pos: world_to_raster(&vertice.pos, cam, size),
        color: vertice.color,
    }
}

pub fn world_to_raster_triangle(
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
        (f64::min(f64::min(t.p0.pos.y, t.p1.pos.y), t.p2.pos.y) as u32).clamp(0, size.height - 1);
    let max_y =
        (f64::max(f64::max(t.p0.pos.y, t.p1.pos.y), t.p2.pos.y) as u32).clamp(0, size.height - 1);

    Rect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}

// Calculates the area of the parallelogram from vectors ab and ap
// Positive if p is "right" of ab
fn edge_function(ab: Vec3f, tri_a: Vec3f, p: Vec3f) -> f64 {
    let ap = p - tri_a;
    ap.x * ab.y - ap.y * ab.x
}

fn buffer_index(p: Vec3f, size: &PhysicalSize<u32>) -> Option<usize> {
    if p.x >= 0. && p.x < (size.width as f64) && p.y >= 0. && p.y < (size.height as f64) {
        Some(p.x as usize + p.y as usize * size.width as usize)
    } else {
        None
    }
}

fn draw_vertice_basic<B: DerefMut<Target = [u32]>>(
    buffer: &mut B,
    size: &PhysicalSize<u32>,
    v: &Vertex,
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
    depth_buffer: &mut [f64],
    cam: &Camera,
    size: &PhysicalSize<u32>,
    show_vertices: bool,
) {
    let tri_raster = world_to_raster_triangle(triangle, cam, size);

    let bb = bounding_box_triangle(&tri_raster, size);

    let p01 = tri_raster.p1.pos - tri_raster.p0.pos;
    let p12 = tri_raster.p2.pos - tri_raster.p1.pos;
    let p20 = tri_raster.p0.pos - tri_raster.p2.pos;

    // TODO: not efficient ?
    let tri_area = edge_function(p01, Vec3f::default(), -p20);

    // Dot product gives negative if two vectors are opposed, so we compare light vector to
    // face normal vector to see if they are opposed (face is lit).
    let sun_norm = SUN_DIRECTION.normalize();
    let triangle_normal = (triangle.p1.pos - triangle.p0.pos)
        .cross(&(triangle.p0.pos - triangle.p2.pos))
        .normalize();
    let light = sun_norm
        .dot(&triangle_normal)
        .clamp(MINIMAL_AMBIANT_LIGHT, 1.);

    // TODO: Paralléliser
    (bb.x..=(bb.x + bb.width))
        .flat_map(|x| {
            (bb.y..=(bb.y + bb.height)).map(move |y| Vec3f {
                x: x as f64,
                y: y as f64,
                z: 0.,
            })
        })
        .for_each(|pixel| {
            let e01 = edge_function(p01, tri_raster.p0.pos, pixel);
            let e12 = edge_function(p12, tri_raster.p1.pos, pixel);
            let e20 = edge_function(p20, tri_raster.p2.pos, pixel);
            if e01 >= 0. && e12 >= 0. && e20 >= 0. {
                let a12 = e12 / tri_area;
                let a20 = e20 / tri_area;

                // let depth_2 = 1.
                //     / (1. / tri_raster.p2.pos.z * (e01 / tri_area)
                //         + 1. / tri_raster.p0.pos.z * a12
                //         + 1. / tri_raster.p1.pos.z * a20);
                // Because a01 + a12 + a20 = 1., we can avoid a division and not compute a01.
                // The terms Z1-Z0 and Z2-Z0 can generally be precomputed, which simplifies the computation of Z to two additions and two multiplications. This optimization is worth mentioning because GPUs utilize it, and it's often discussed for essentially this reason.

                // Depth doesn't evolve linearly (its inverse does).
                let p2_z_inv = 1. / tri_raster.p2.pos.z;
                let depth = 1.
                    / (p2_z_inv
                        + (1. / tri_raster.p0.pos.z - p2_z_inv) * a12
                        + (1. / tri_raster.p1.pos.z - p2_z_inv) * a20);

                // Depth correction of other properties :
                // Divide each value by the point Z coord and finally multiply by depth.

                if depth > 0. {
                    let index = (pixel.x as usize) + (pixel.y as usize) * size.width as usize;

                    if depth < depth_buffer[index] {
                        let col_2 =
                            Vec4u::from_color_u32(tri_raster.p2.color) / tri_raster.p2.pos.z;
                        let col = (col_2
                            + (Vec4u::from_color_u32(tri_raster.p0.color) / tri_raster.p0.pos.z
                                - col_2)
                                * a12
                            + (Vec4u::from_color_u32(tri_raster.p1.color) / tri_raster.p1.pos.z
                                - col_2)
                                * a20)
                            * depth
                            * light;

                        buffer[index] = col.as_color_u32();
                        depth_buffer[index] = depth;
                    }
                }
            }
        });

    if show_vertices {
        draw_vertice_basic(buffer, size, &tri_raster.p0);
        draw_vertice_basic(buffer, size, &tri_raster.p1);
        draw_vertice_basic(buffer, size, &tri_raster.p2);
    }
}

pub fn rasterize<B: DerefMut<Target = [u32]>>(
    world: &World,
    buffer: &mut B,
    depth_buffer: &mut [f64],
    size: &PhysicalSize<u32>,
    show_vertices: bool,
) {
    // TODO: paralléliser
    world.triangles.iter().for_each(|f| {
        rasterize_triangle(f, buffer, depth_buffer, &world.camera, size, show_vertices)
    });
}
