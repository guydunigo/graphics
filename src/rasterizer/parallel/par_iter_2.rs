//! TODO: Delete : bad perf !

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use rayon::prelude::*;
use winit::dpi::PhysicalSize;

use crate::{
    font::TextWriter,
    maths::{Vec3f, Vec4u},
    rasterizer::{
        Engine, MINIMAL_AMBIANT_LIGHT, Rect, bounding_box_triangle, buffer_index, edge_function,
        settings::Settings, world_to_raster_triangle,
    },
    scene::{Texture, Triangle, World},
    window::AppObserver,
};

const DEPTH_PRECISION: f32 = 2048.;

#[derive(Default, Debug, Clone)]
pub struct ParIterEngine2 {
    depth_color_buffer: Arc<[AtomicU64]>,
    last_copy_buffer: u128,
}

impl ParIterEngine2 {
    const DEFAULT_COLOR: u32 = 0xff181818;
    const DEFAULT_DEPTH: u32 = u32::MAX;
    const DEFAULT_DEPTH_COLOR: u64 =
        ((Self::DEFAULT_DEPTH as u64) << 32) | (Self::DEFAULT_COLOR as u64);

    // let size = window.inner_size();
    // let tot_size = (size.width * size.height) as usize;

    fn init_buffer<T, F: Fn() -> T>(tot_size: usize, f: F) -> Arc<[T]> {
        let mut v = Vec::with_capacity(tot_size);
        v.resize_with(tot_size, f);
        v.into()
    }

    /// Resize `depth_color_buffer` if necessary and fills it with [`Self::DEFAULT_DEPTH_COLOR`]
    fn clean_resize_buffer(&mut self, size: PhysicalSize<u32>) -> u128 {
        let t = Instant::now();
        let tot_size = (size.width * size.height) as usize;

        if self.depth_color_buffer.len() >= tot_size {
            self.depth_color_buffer
                .par_iter()
                .take(tot_size)
                .for_each(|v| v.store(Self::DEFAULT_DEPTH_COLOR, Ordering::Relaxed))
        } else {
            self.depth_color_buffer =
                Self::init_buffer(tot_size, || AtomicU64::new(Self::DEFAULT_DEPTH_COLOR));
        }
        Instant::now().duration_since(t).as_millis()
    }
}

impl Engine for ParIterEngine2 {
    fn rasterize<B: std::ops::DerefMut<Target = [u32]>>(
        &mut self,
        settings: &Settings,
        text_writer: &TextWriter,
        world: &crate::scene::World,
        buffer: &mut B,
        size: PhysicalSize<u32>,
        app: AppObserver,
        #[cfg(feature = "stats")] stats: &mut Stats,
    ) {
        let buffer_fill_time = self.clean_resize_buffer(size);

        let t = Instant::now();
        rasterize_world(
            settings,
            world,
            &self.depth_color_buffer,
            size,
            #[cfg(feature = "stats")]
            &stats,
        );
        let rendering_time = Instant::now().duration_since(t).as_millis();

        // TODO: copy from single thread
        // TODO: extract to function
        {
            let cam_rot = world.camera.rot();
            #[cfg(feature = "stats")]
            let stats = format!("{:#?}", stats);
            #[cfg(not(feature = "stats"))]
            let stats = "Stats disabled";
            let display = format!(
                "fps : {} | {}ms - {}ms - {}ms / {}ms{}\n{} {} {} {}\n{:?}\n{}",
                (1000. / app.last_rendering_duration as f32).round(),
                buffer_fill_time,
                rendering_time,
                self.last_copy_buffer,
                app.last_rendering_duration,
                app.cursor
                    .and_then(|cursor| self
                        .depth_color_buffer
                        .get(cursor.x as usize + cursor.y as usize * size.width as usize)
                        .map(|c| format!(
                            "\n({},{}) 0x{:x}",
                            cursor.x.floor(),
                            cursor.y.floor(),
                            u64_to_color(c.load(Ordering::Relaxed))
                        )))
                    .unwrap_or(String::from("\nNo cursor position")),
                world.camera.pos,
                cam_rot.u(),
                cam_rot.v(),
                cam_rot.w(),
                settings,
                stats,
            );
            text_writer.rasterize_par(&self.depth_color_buffer, size, &display[..]);
        }

        let copy_buffer = Instant::now();
        (0..(size.width * size.height) as usize).for_each(|i| {
            buffer[i] = u64_to_color(self.depth_color_buffer[i].load(Ordering::Relaxed));
        });

        self.last_copy_buffer = Instant::now().duration_since(copy_buffer).as_millis();
    }
}

pub const fn depth_to_u64(depth: f32) -> u64 {
    ((depth * DEPTH_PRECISION) as u64) << 32
}

pub const fn u64_to_color(depth_color: u64) -> u32 {
    (0xffffffff & depth_color) as u32
}

#[cfg(feature = "stats")]
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

fn draw_vertice_basic(
    depth_color_buffer: &[AtomicU64],
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
            } as u64;

            depth_color_buffer[i].store(color, Ordering::Relaxed);
            depth_color_buffer[i - 1].store(color, Ordering::Relaxed);
            depth_color_buffer[i + 1].store(color, Ordering::Relaxed);
            depth_color_buffer[i - (size.width as usize)].store(color, Ordering::Relaxed);
            depth_color_buffer[i + (size.width as usize)].store(color, Ordering::Relaxed);
        }
    }
}

pub fn rasterize_world(
    settings: &Settings,
    world: &World,
    depth_color_buffer: &[AtomicU64],
    size: PhysicalSize<u32>,
    #[cfg(feature = "stats")] stats: &Stats,
) {
    let ratio_w_h = size.width as f32 / size.height as f32;

    world
        .meshes
        .par_iter()
        // TODO: or par_bridge or direct to par_iterator
        .flat_map_iter(|m| m.to_world_triangles())
        .inspect(|_| {
            #[cfg(feature = "stats")]
            stats.nb_triangles_tot.fetch_add(1, Ordering::Relaxed);
        })
        .map(|t| {
            // TODO: explode ?
            let t_raster = world_to_raster_triangle(&t, &world.camera, size, ratio_w_h);
            let bb = bounding_box_triangle(&t_raster, size);
            (t, t_raster, bb)
        })
        .filter(|(_, _, bb)| {
            // TODO: max_z >= MAX_DEPTH ?
            !(bb.min_x == bb.max_x || bb.min_y == bb.max_y || bb.max_z <= world.camera.z_near)
        })
        .inspect(|_| {
            #[cfg(feature = "stats")]
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
            raster_normale.z >= 0.
        })
        .inspect(|_| {
            #[cfg(feature = "stats")]
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
                depth_color_buffer,
                world.camera.z_near,
                size,
                settings,
                #[cfg(feature = "stats")]
                stats,
                &bb,
                light,
                p01,
                p20,
            )
        });
}

fn rasterize_triangle(
    tri_raster: &Triangle,
    depth_color_buffer: &[AtomicU64],
    z_near: f32,
    size: PhysicalSize<u32>,
    settings: &Settings,
    #[cfg(feature = "stats")] stats: &Stats,
    bb: &Rect,
    light: f32,
    p01: Vec3f,
    p20: Vec3f,
) {
    #[cfg(feature = "stats")]
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
        .for_each(|pixel| {
            #[cfg(feature = "stats")]
            stats.nb_pixels_tested.fetch_add(1, Ordering::Relaxed);

            let e01 = edge_function(p01, pixel - tri_raster.p0);
            let e12 = edge_function(p12, pixel - tri_raster.p1);
            let e20 = edge_function(p20, pixel - tri_raster.p2);

            // If negative for the 3 : we're outside the triangle.
            if e01 < 0. || e12 < 0. || e20 < 0. {
                return;
            }

            #[cfg(feature = "stats")]
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

            #[cfg(feature = "stats")]
            stats.nb_pixels_front.fetch_add(1, Ordering::Relaxed);

            let index = (pixel.x as usize) + (pixel.y as usize) * size.width as usize;

            let depth_u64 = depth_to_u64(depth);

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
            } as u64
                | depth_u64;

            #[cfg(not(feature = "stats"))]
            depth_color_buffer[index].fetch_min(col, Ordering::Relaxed);
            #[cfg(feature = "stats")]
            let res = depth_color_buffer[index].fetch_min(col, Ordering::Relaxed) > depth_u64;
            #[cfg(feature = "stats")]
            was_drawn.store(res, Ordering::Relaxed);
            #[cfg(feature = "stats")]
            stats
                .nb_pixels_written
                .fetch_add(res as usize, Ordering::Relaxed);
        });

    #[cfg(feature = "stats")]
    if was_drawn.load(Ordering::Relaxed) {
        stats.nb_triangles_drawn.fetch_add(1, Ordering::Relaxed);
    }

    if settings.show_vertices {
        draw_vertice_basic(depth_color_buffer, size, tri_raster.p0, &tri_raster.texture);
        draw_vertice_basic(depth_color_buffer, size, tri_raster.p1, &tri_raster.texture);
        draw_vertice_basic(depth_color_buffer, size, tri_raster.p2, &tri_raster.texture);
    }
}
