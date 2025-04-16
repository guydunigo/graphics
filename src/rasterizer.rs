use std::ops::DerefMut;

use winit::dpi::PhysicalSize;

use crate::{
    maths::{Vec3f, Vec4u},
    scene::{Camera, Mesh, Texture, Triangle, World},
};

const SUN_DIRECTION: Vec3f = Vec3f::new(-1., -1., -1.);
const MINIMAL_AMBIANT_LIGHT: f32 = 0.2;

fn world_to_raster(p_world: Vec3f, cam: &Camera, size: &PhysicalSize<u32>) -> Vec3f {
    // Camera space
    let mut p = p_world.seen_from(cam.pos, &cam.rot);

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

    // Raster space
    // [0,1]
    Vec3f {
        x: (p.x + 1.) / 2. * (size.width as f32),
        y: (1. - p.y) / 2. * (size.height as f32),
        z: p.z,
    }
}

pub fn world_to_raster_triangle(
    triangle: &Triangle,
    cam: &Camera,
    size: &PhysicalSize<u32>,
) -> Triangle {
    Triangle {
        p0: world_to_raster(triangle.p0, cam, size),
        p1: world_to_raster(triangle.p1, cam, size),
        p2: world_to_raster(triangle.p2, cam, size),
        texture: triangle.texture,
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
    let min_x = (f32::min(f32::min(t.p0.x, t.p1.x), t.p2.x) as u32).clamp(0, size.width - 1);
    let max_x = (f32::max(f32::max(t.p0.x, t.p1.x), t.p2.x) as u32).clamp(0, size.width - 1);
    let min_y = (f32::min(f32::min(t.p0.y, t.p1.y), t.p2.y) as u32).clamp(0, size.height - 1);
    let max_y = (f32::max(f32::max(t.p0.y, t.p1.y), t.p2.y) as u32).clamp(0, size.height - 1);

    Rect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}

// Calculates the area of the parallelogram from vectors ab and ap
// Positive if p is "right" of ab
fn edge_function(ab: Vec3f, tri_a: Vec3f, p: Vec3f) -> f32 {
    let ap = p - tri_a;
    ap.x * ab.y - ap.y * ab.x
}

fn buffer_index(p: Vec3f, size: &PhysicalSize<u32>) -> Option<usize> {
    if p.x >= 0. && p.x < (size.width as f32) && p.y >= 0. && p.y < (size.height as f32) {
        Some(p.x as usize + p.y as usize * size.width as usize)
    } else {
        None
    }
}

fn draw_vertice_basic<B: DerefMut<Target = [u32]>>(
    buffer: &mut B,
    size: &PhysicalSize<u32>,
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

fn rasterize_triangle<B: DerefMut<Target = [u32]>>(
    triangle: &Triangle,
    buffer: &mut B,
    depth_buffer: &mut [f32],
    cam: &Camera,
    size: &PhysicalSize<u32>,
    show_vertices: bool,
) {
    let tri_raster = world_to_raster_triangle(triangle, cam, size);

    let bb = bounding_box_triangle(&tri_raster, size);

    let p01 = tri_raster.p1 - tri_raster.p0;
    let p12 = tri_raster.p2 - tri_raster.p1;
    let p20 = tri_raster.p0 - tri_raster.p2;

    let tri_area = edge_function(p01, tri_raster.p0, tri_raster.p2);

    // TODO: calculate before ?
    // Dot product gives negative if two vectors are opposed, so we compare light vector to
    // face normal vector to see if they are opposed (face is lit).
    let sun_norm = SUN_DIRECTION.normalize();
    let triangle_normal = (triangle.p1 - triangle.p0)
        .cross(triangle.p0 - triangle.p2)
        .normalize();
    let light = sun_norm
        .dot(triangle_normal)
        .clamp(MINIMAL_AMBIANT_LIGHT, 1.);

    // TODO: Optimize color calculus
    let texture = match tri_raster.texture {
        Texture::Color(col) => Texture::Color((Vec4u::from_color_u32(col) * light).as_color_u32()),
        _ => tri_raster.texture,
    };

    // TODO: Paralléliser
    (bb.x..=(bb.x + bb.width))
        .flat_map(|x| {
            (bb.y..=(bb.y + bb.height)).map(move |y| Vec3f {
                x: x as f32,
                y: y as f32,
                z: 0.,
            })
        })
        .for_each(|pixel| {
            let e01 = edge_function(p01, tri_raster.p0, pixel);
            let e12 = edge_function(p12, tri_raster.p1, pixel);
            let e20 = edge_function(p20, tri_raster.p2, pixel);

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
                let p2_z_inv = 1. / tri_raster.p2.z;
                let depth = 1.
                    / (p2_z_inv
                        + (1. / tri_raster.p0.z - p2_z_inv) * a12
                        + (1. / tri_raster.p1.z - p2_z_inv) * a20);

                // Depth correction of other properties :
                // Divide each value by the point Z coord and finally multiply by depth.

                if depth > 0. {
                    let index = (pixel.x as usize) + (pixel.y as usize) * size.width as usize;

                    if depth < depth_buffer[index] {
                        let col = match texture {
                            Texture::Color(col) => col,
                            Texture::VertexColor(c0, c1, c2) => {
                                // TODO: Optimize color calculus
                                let col_2 = Vec4u::from_color_u32(c2) / tri_raster.p2.z;

                                ((col_2
                                    + (Vec4u::from_color_u32(c0) / tri_raster.p0.z - col_2) * a12
                                    + (Vec4u::from_color_u32(c1) / tri_raster.p1.z - col_2) * a20)
                                    * (depth * light))
                                    .as_color_u32()
                            }
                        };

                        buffer[index] = col;
                        depth_buffer[index] = depth;
                    }
                }
            }
        });

    if show_vertices {
        draw_vertice_basic(buffer, size, tri_raster.p0, &tri_raster.texture);
        draw_vertice_basic(buffer, size, tri_raster.p1, &tri_raster.texture);
        draw_vertice_basic(buffer, size, tri_raster.p2, &tri_raster.texture);
    }
}

pub fn rasterize<B: DerefMut<Target = [u32]>>(
    world: &World,
    buffer: &mut B,
    depth_buffer: &mut [f32],
    size: &PhysicalSize<u32>,
    show_vertices: bool,
) {
    // TODO: paralléliser
    world
        .meshes
        .iter()
        .flat_map(Mesh::to_world_triangles)
        .for_each(|f| {
            rasterize_triangle(&f, buffer, depth_buffer, &world.camera, size, show_vertices)
        });
}
