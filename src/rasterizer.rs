use std::{
    hint,
    sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
};

use rayon::prelude::*;
use winit::dpi::PhysicalSize;

use crate::{
    maths::{Vec3f, Vec4u},
    scene::{Camera, Texture, Triangle, World},
};

const MINIMAL_AMBIANT_LIGHT: f32 = 0.2;

pub const fn depth_to_atomic_u32(depth: f32) -> u32 {
    u32::from_le_bytes(depth.to_le_bytes())
}

pub const fn depth_from_atomic_u32(depth: u32) -> f32 {
    f32::from_le_bytes(depth.to_le_bytes())
}

#[derive(Default, Debug)]
pub struct Stats {
    pub nb_triangles_tot: AtomicUsize,
    pub nb_triangles_sight: AtomicUsize,
    pub nb_triangles_facing: AtomicUsize,
    pub nb_triangles_drawn: AtomicUsize,
    pub nb_pixels_tested: AtomicUsize,
    pub nb_pixels_in: AtomicUsize,
    pub nb_pixels_front: AtomicUsize,
    pub nb_pixels_written: AtomicUsize,
    // pub misc: String,
}

#[derive(Debug, Clone, Copy)]
pub struct Settings {
    /// Over-print all vertices
    pub show_vertices: bool,
    /// Sort triangles by point with mininum Z value
    pub sort_triangles: TriangleSorting,
    /// Eliminate back-facing faces early
    pub back_face_culling: bool,
    pub lock_buffers: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            show_vertices: false,
            sort_triangles: TriangleSorting::None,
            back_face_culling: true,
            lock_buffers: true,
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub enum TriangleSorting {
    #[default]
    None,
    BackToFront,
    FrontToBack,
}

fn world_to_raster(
    p_world: Vec3f,
    cam: &Camera,
    size: &PhysicalSize<u32>,
    ratio_w_h: f32,
) -> Vec3f {
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

pub fn world_to_raster_triangle(
    triangle: &Triangle,
    cam: &Camera,
    size: &PhysicalSize<u32>,
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
pub struct Rect {
    pub min_x: u32,
    pub min_y: u32,
    pub max_x: u32,
    pub max_y: u32,
    pub max_z: f32,
}

fn bounding_box_triangle(t: &Triangle, size: &PhysicalSize<u32>) -> Rect {
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

fn buffer_index(p: Vec3f, size: &PhysicalSize<u32>) -> Option<usize> {
    if p.x >= 0. && p.x < (size.width as f32) && p.y >= 0. && p.y < (size.height as f32) {
        Some(p.x as usize + p.y as usize * size.width as usize)
    } else {
        None
    }
}

fn draw_vertice_basic(buffer: &[AtomicU32], size: &PhysicalSize<u32>, v: Vec3f, texture: &Texture) {
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

            buffer[i].store(color, Ordering::Relaxed);
            buffer[i - 1].store(color, Ordering::Relaxed);
            buffer[i + 1].store(color, Ordering::Relaxed);
            buffer[i - (size.width as usize)].store(color, Ordering::Relaxed);
            buffer[i + (size.width as usize)].store(color, Ordering::Relaxed);
        }
    }
}

fn rasterize_triangle(
    tri_raster: &Triangle,
    buffer: &[AtomicU32],
    depth_buffer: &[AtomicU32],
    lock_buffer: &[AtomicBool],
    z_near: f32,
    size: &PhysicalSize<u32>,
    settings: &Settings,
    stats: &Stats,
    bb: &Rect,
    light: f32,
    p01: Vec3f,
    p20: Vec3f,
) {
    let was_drawn = AtomicBool::new(false);

    let p12 = tri_raster.p2 - tri_raster.p1;

    let tri_area = edge_function(p20, p01);

    (bb.min_x..=bb.max_x)
        .flat_map(|x| {
            (bb.min_y..=bb.max_y).map(move |y| Vec3f {
                x: x as f32,
                y: y as f32,
                z: 0.,
            })
        })
        // TODO: ugly
        .collect::<Vec<Vec3f>>()
        .par_chunks(512)
        .for_each(|ps| {
            ps.iter().for_each(|pixel| {
                stats.nb_pixels_tested.fetch_add(1, Ordering::Relaxed);

                let e01 = edge_function(p01, *pixel - tri_raster.p0);
                let e12 = edge_function(p12, *pixel - tri_raster.p1);
                let e20 = edge_function(p20, *pixel - tri_raster.p2);

                // If negative for the 3 : we're outside the triangle.
                if e01 < 0. || e12 < 0. || e20 < 0. {
                    return;
                }

                stats.nb_pixels_in.fetch_add(1, Ordering::Relaxed);

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

                if depth <= z_near {
                    return;
                }

                stats.nb_pixels_front.fetch_add(1, Ordering::Relaxed);

                let index = (pixel.x as usize) + (pixel.y as usize) * size.width as usize;

                if settings.lock_buffers {
                    while lock_buffer[index]
                        .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Acquire)
                        .unwrap_or_else(|x| x)
                    {
                        hint::spin_loop();
                    }
                }

                // TODO: which ordering ?
                if depth < depth_from_atomic_u32(depth_buffer[index].load(Ordering::Relaxed)) {
                    was_drawn.store(true, Ordering::Relaxed);
                    stats.nb_pixels_written.fetch_add(1, Ordering::Relaxed);

                    let col = match tri_raster.texture {
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

                    // TODO: which ordering ?
                    buffer[index].store(col, Ordering::Relaxed);
                    depth_buffer[index].store(depth_to_atomic_u32(depth), Ordering::Relaxed);
                }

                if settings.lock_buffers {
                    lock_buffer[index].store(false, Ordering::Release);
                }
            })
        });

    if was_drawn.load(Ordering::Relaxed) {
        stats.nb_triangles_drawn.fetch_add(1, Ordering::Relaxed);
    }

    if settings.show_vertices {
        draw_vertice_basic(buffer, size, tri_raster.p0, &tri_raster.texture);
        draw_vertice_basic(buffer, size, tri_raster.p1, &tri_raster.texture);
        draw_vertice_basic(buffer, size, tri_raster.p2, &tri_raster.texture);
    }
}

pub fn rasterize(
    world: &World,
    buffer: &[AtomicU32],
    depth_buffer: &[AtomicU32],
    lock_buffer: &[AtomicBool],
    size: &PhysicalSize<u32>,
    settings: &Settings,
    stats: &Stats,
) {
    let ratio_w_h = size.width as f32 / size.height as f32;

    world
        .meshes
        .iter()
        // .map(Mesh::to_world_triangles_par)
        // TODO: par_chunks ?
        .for_each(|m| {
            m.triangles
                .par_iter()
                .map(|t| t.scale_rot_move(m.scale, &m.rot, m.pos))
                .inspect(|_| {
                    stats.nb_triangles_tot.fetch_add(1, Ordering::Relaxed);
                })
                .map(|t| {
                    // TODO: explode ?
                    let t_raster = world_to_raster_triangle(&t, &world.camera, size, ratio_w_h);
                    (t, t_raster, bounding_box_triangle(&t_raster, size))
                })
                .filter(|(_, _, bb)| {
                    // TODO: max_z >= MAX_DEPTH ?
                    !(bb.min_x == bb.max_x
                        || bb.min_y == bb.max_y
                        || bb.max_z <= world.camera.z_near)
                })
                .inspect(|_| {
                    stats.nb_triangles_sight.fetch_add(1, Ordering::Relaxed);
                })
                .map(|(t, t_raster, bb)| {
                    let p01 = t_raster.p1 - t_raster.p0;
                    let p20 = t_raster.p0 - t_raster.p2;
                    (t, t_raster, bb, p01, p20)
                })
                ////////////////////////////////
                // Back face culling
                // If triangle normal and camera sight are in same direction (dot product > 0),
                // it's invisible.
                .filter(|(_, _, _, p01, p20)| {
                    // Calculate only of normal z
                    let raster_normale = p01.cross(*p20);
                    // TODO: remove setting to back_face cull
                    raster_normale.z >= 0. || !settings.back_face_culling
                })
                .inspect(|_| {
                    stats.nb_triangles_facing.fetch_add(1, Ordering::Relaxed);
                })
                ////////////////////////////////
                // Sunlight
                // Dot product gives negative if two vectors are opposed, so we compare light
                // vector to face normal vector to see if they are opposed (face is lit).
                //
                // Also simplifying colours.
                .map(|(t, mut t_raster, bb, p01, p20)| {
                    let triangle_normal = (t.p1 - t.p0).cross(t.p0 - t.p2).normalize();
                    let light = world
                        .sun_direction
                        .dot(triangle_normal)
                        .clamp(MINIMAL_AMBIANT_LIGHT, 1.);

                    // If a `Texture::VertexColor` has the same color for all vertices, then we can
                    // consider it like a `Texture::Color`.
                    if let Texture::VertexColor(c0, c1, c2) = t_raster.texture {
                        if c0 == c1 && c1 == c2 {
                            t_raster.texture = Texture::Color(c0);
                        }
                    }

                    if let Texture::Color(col) = t_raster.texture {
                        t_raster.texture =
                            Texture::Color((Vec4u::from_color_u32(col) * light).as_color_u32());
                    }

                    (t_raster, bb, light, p01, p20)
                })
                .for_each(|(t_raster, bb, light, p01, p20)| {
                    rasterize_triangle(
                        &t_raster,
                        buffer,
                        depth_buffer,
                        lock_buffer,
                        world.camera.z_near,
                        size,
                        settings,
                        stats,
                        &bb,
                        light,
                        p01,
                        p20,
                    )
                })
        });

    // TODO: based on camera (at least screen space)
    // match settings.sort_triangles {
    //     TriangleSorting::None => rasterize_trangles(
    //         world,
    //         buffer,
    //         depth_buffer,
    //         size,
    //         ratio_w_h,
    //         settings,
    //         stats,
    //         triangles,
    //     ),
    //     TriangleSorting::BackToFront => {
    //         let mut array: Vec<Triangle> = triangles.collect();
    //         array.sort_by_key(|t| -t.min_z() as u32);
    //         rasterize_trangles(
    //             world,
    //             buffer,
    //             depth_buffer,
    //             size,
    //             ratio_w_h,
    //             settings,
    //             stats,
    //             array.drain(..),
    //         )
    //     }
    //     TriangleSorting::FrontToBack => {
    //         let mut array: Vec<Triangle> = triangles.collect();
    //         array.sort_by_key(|t| t.min_z() as u32);
    //         rasterize_trangles(
    //             world,
    //             buffer,
    //             depth_buffer,
    //             size,
    //             ratio_w_h,
    //             settings,
    //             stats,
    //             array.drain(..),
    //         )
    //     }
    // }
}

/*
pub fn rasterize_trangles<B: DerefMut<Target = [u32]>, I: Iterator<Item = Triangle>>(
    world: &World,
    buffer: &mut B,
    depth_buffer: &mut [f32],
    size: &PhysicalSize<u32>,
    ratio_w_h: f32,
    settings: &Settings,
    stats: &mut Stats,
    ite: I,
) {
    ite.for_each(|t| {
        stats.nb_triangles_tot.fetch_add(1, Ordering:Relaxed);
        rasterize_triangle(
            &t,
            buffer,
            depth_buffer,
            &world.camera,
            size,
            ratio_w_h,
            settings,
            stats,
        )
    });
}
*/
