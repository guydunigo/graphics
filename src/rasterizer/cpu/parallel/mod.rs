mod par_iter_0;
mod par_iter_1;
mod par_iter_2;
mod par_iter_3;
mod par_iter_4;
mod par_iter_5;
mod thread_pool;

use glam::Vec3;
use rayon::prelude::*;
use std::{
    ops::DerefMut,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

pub use par_iter_0::ParIterEngine0;
pub use par_iter_1::ParIterEngine1;
pub use par_iter_2::ParIterEngine2;
pub use par_iter_3::ParIterEngine3;
pub use par_iter_4::ParIterEngine4;
pub use par_iter_5::ParIterEngine5;
pub use thread_pool::ThreadPoolEngine;
use winit::dpi::PhysicalSize;

#[cfg(feature = "stats")]
use super::Stats;
#[cfg(feature = "stats")]
use std::sync::atomic::{AtomicBool, AtomicUsize};

use crate::{
    font::{self, TextWriter},
    maths::ColorF32,
    rasterizer::{Settings, cpu::populate_nodes},
    scene::{BoundingBox, Camera, DEFAULT_BACKGROUND_COLOR, Texture},
    window::AppObserver,
};

use super::{Triangle, buffer_index, cursor_buffer_index, edge_function, format_debug};

const DEPTH_PRECISION: f32 = 2048.;
const DEFAULT_DEPTH: u32 = u32::MAX;
const DEFAULT_DEPTH_COLOR: u64 = ((DEFAULT_DEPTH as u64) << 32) | (DEFAULT_BACKGROUND_COLOR as u64);

pub trait ParIterEngine {
    fn depth_color_buffer(&self) -> &Arc<[AtomicU64]>;
    fn depth_color_buffer_mut(&mut self) -> &mut Arc<[AtomicU64]>;

    fn triangles_mut(&mut self) -> &mut Vec<Triangle>;

    fn rasterize_world(
        &mut self,
        settings: &Settings,
        camera: &Camera,
        sun_direction: Vec3,
        size: PhysicalSize<u32>,
        ratio_w_h: f32,
        #[cfg(feature = "stats")] stats: &ParStats,
    );

    fn rasterize<B: DerefMut<Target = [u32]>>(
        &mut self,
        settings: &Settings,
        text_writer: &TextWriter,
        world: &crate::scene::World,
        buffer: &mut B,
        mut size: PhysicalSize<u32>,
        app: &mut AppObserver,
        #[cfg(feature = "stats")] stats: &mut Stats,
    ) {
        let original_size = size;

        let mut font_size = font::PX;
        size.width *= settings.oversampling as u32;
        size.height *= settings.oversampling as u32;
        if settings.parallel_text {
            font_size *= settings.oversampling as f32;
        }

        app.last_buffer_fill_micros = clean_resize_buffer(self.depth_color_buffer_mut(), size);

        {
            // TODO: let t = Instant::now();
            let triangles = self.triangles_mut();
            triangles.clear();
            world.scene.if_present(|s| {
                s.top_nodes()
                    .iter()
                    .for_each(|n| populate_nodes(triangles, &n.read().unwrap()))
            });

            /*
            // TODO: Can't sort, not from camera view
            match settings.sort_triangles {
                TriangleSorting::BackToFront => {
                    triangles.sort_by_key(|t| -t.min_z() as u32);
                }
                TriangleSorting::FrontToBack => {
                    triangles.sort_by_key(|t| t.min_z() as u32);
                }
                _ => (),
            }
            */
            // TODO: println!("{}", t.elapsed().as_micros()); // ~100 micros
        }

        let ratio_w_h = size.width as f32 / size.height as f32;

        {
            let t = Instant::now();
            #[cfg(feature = "stats")]
            let par_stats = ParStats::from(&*stats);
            self.rasterize_world(
                settings,
                &world.camera,
                world.sun_direction,
                size,
                ratio_w_h,
                #[cfg(feature = "stats")]
                &par_stats,
            );
            #[cfg(feature = "stats")]
            par_stats.update_stats(stats);
            app.last_rendering_micros = t.elapsed().as_micros();
        }

        let depth_color_buffer = self.depth_color_buffer();

        if settings.parallel_text {
            let cursor_color = cursor_buffer_index(app.cursor(), size)
                .map(|index| u64_to_color(depth_color_buffer[index].load(Ordering::Relaxed)));
            let display = format_debug(
                settings,
                world,
                app,
                size,
                cursor_color,
                #[cfg(feature = "stats")]
                stats,
            );
            text_writer.rasterize_par(depth_color_buffer, size, font_size, &display[..]);
        }

        // TODO: parallel (safe split ref vec)
        let t = Instant::now();
        if settings.oversampling > 1 {
            let oversampling_2 = settings.oversampling * settings.oversampling;

            (0..((original_size.height * original_size.width) as usize))
                .step_by(original_size.width as usize)
                .for_each(|j| {
                    let jx = j * oversampling_2;
                    (0..(original_size.width as usize)).for_each(|i| {
                        // println!(
                        //     "{jx4},{ix4} {} {} {} {}",
                        //     jx4 * 4 + ix4 * 2,
                        //     jx4 * 4 + ix4 * 2 + 1,
                        //     jx4 * 4 + size.width as usize + ix4 * 2,
                        //     jx4 * 4 + size.width as usize + ix4 * 2 + 1,
                        // );
                        let ix = i * settings.oversampling;

                        let color_avg: ColorF32 = (0..(settings.oversampling
                            * size.width as usize))
                            .step_by(size.width as usize)
                            .flat_map(|jo| {
                                (0..settings.oversampling).map(move |io| {
                                    ColorF32::from_argb_u32(u64_to_color(
                                        depth_color_buffer[jx + jo + ix + io]
                                            .load(Ordering::Relaxed),
                                    ))
                                })
                            })
                            .sum();
                        let color_avg = color_avg / oversampling_2 as f32;
                        buffer[j + i] = color_avg.as_color_u32();
                    });
                });
        } else {
            (0..(size.width * size.height) as usize).for_each(|i| {
                buffer[i] = u64_to_color(depth_color_buffer[i].load(Ordering::Relaxed));
            });
        }
        app.last_buffer_copy_micros = t.elapsed().as_micros();

        if !settings.parallel_text {
            let cursor_color =
                cursor_buffer_index(app.cursor(), original_size).map(|index| buffer[index]);
            let display = format_debug(
                settings,
                world,
                app,
                original_size,
                cursor_color,
                #[cfg(feature = "stats")]
                stats,
            );
            text_writer.rasterize(buffer, original_size, font_size, &display[..]);
        }
    }
}

fn rasterize_triangle(
    tri_raster: &Triangle,
    depth_color_buffer: &[AtomicU64],
    z_near: f32,
    size: PhysicalSize<u32>,
    settings: &Settings,
    #[cfg(feature = "stats")] stats: &ParStats,
    bb: &BoundingBox<u32>,
    p01: Vec3,
    p20: Vec3,
) {
    #[cfg(feature = "stats")]
    let was_drawn = AtomicBool::new(false);

    let p12 = tri_raster.p2 - tri_raster.p1;

    let tri_area = edge_function(p20, p01);

    (bb.min_x..=bb.max_x)
        .flat_map(|x| {
            (bb.min_y..=bb.max_y).map(move |y| Vec3 {
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

            let col = match tri_raster.material {
                Texture::Color(col) => col,
                Texture::VertexColor(c0, c1, c2) => {
                    let col_2 = ColorF32::from_argb_u32(c2) / tri_raster.p2.z;

                    ((col_2
                        + (ColorF32::from_argb_u32(c0) / tri_raster.p0.z - col_2) * a12
                        + (ColorF32::from_argb_u32(c1) / tri_raster.p1.z - col_2) * a20)
                        * depth)
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
        draw_vertice_basic(
            depth_color_buffer,
            size,
            tri_raster.p0,
            &tri_raster.material,
        );
        draw_vertice_basic(
            depth_color_buffer,
            size,
            tri_raster.p1,
            &tri_raster.material,
        );
        draw_vertice_basic(
            depth_color_buffer,
            size,
            tri_raster.p2,
            &tri_raster.material,
        );
    }
}

fn init_buffer<T, F: Fn() -> T>(tot_size: usize, f: F) -> Arc<[T]> {
    let mut v = Vec::with_capacity(tot_size);
    v.resize_with(tot_size, f);
    v.into()
}

/// Resize `depth_color_buffer` if necessary and fills it with [`Self::DEFAULT_DEPTH_COLOR`]
///
/// Returns time as microseconds.
fn clean_resize_buffer(depth_color_buffer: &mut Arc<[AtomicU64]>, size: PhysicalSize<u32>) -> u128 {
    let t = Instant::now();
    let tot_size = (size.width * size.height) as usize;

    if depth_color_buffer.len() >= tot_size {
        depth_color_buffer
            .par_iter()
            .take(tot_size)
            .for_each(|v| v.store(DEFAULT_DEPTH_COLOR, Ordering::Relaxed))
    } else {
        *depth_color_buffer = init_buffer(tot_size, || AtomicU64::new(DEFAULT_DEPTH_COLOR));
    }
    t.elapsed().as_micros()
}

const fn depth_to_u64(depth: f32) -> u64 {
    ((depth * DEPTH_PRECISION) as u64) << 32
}

const fn u64_to_color(depth_color: u64) -> u32 {
    (0xffffffff & depth_color) as u32
}

#[cfg(feature = "stats")]
#[derive(Default, Debug)]
pub struct ParStats {
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

#[cfg(feature = "stats")]
impl ParStats {
    fn update_stats(self, stats: &mut Stats) {
        stats.nb_triangles_tot = self.nb_triangles_tot.into_inner();
        stats.nb_triangles_sight = self.nb_triangles_sight.into_inner();
        stats.nb_triangles_facing = self.nb_triangles_facing.into_inner();
        stats.nb_triangles_drawn = self.nb_triangles_drawn.into_inner();
        stats.nb_pixels_tested = self.nb_pixels_tested.into_inner();
        stats.nb_pixels_in = self.nb_pixels_in.into_inner();
        stats.nb_pixels_front = self.nb_pixels_front.into_inner();
        stats.nb_pixels_written = self.nb_pixels_written.into_inner();
    }
}

#[cfg(feature = "stats")]
impl From<&Stats> for ParStats {
    fn from(value: &Stats) -> Self {
        Self {
            nb_triangles_tot: value.nb_triangles_tot.into(),
            nb_triangles_sight: value.nb_triangles_sight.into(),
            nb_triangles_facing: value.nb_triangles_facing.into(),
            nb_triangles_drawn: value.nb_triangles_drawn.into(),
            nb_pixels_tested: value.nb_pixels_tested.into(),
            nb_pixels_in: value.nb_pixels_in.into(),
            nb_pixels_front: value.nb_pixels_front.into(),
            nb_pixels_written: value.nb_pixels_written.into(),
        }
    }
}

fn draw_vertice_basic(
    depth_color_buffer: &[AtomicU64],
    size: PhysicalSize<u32>,
    v: Vec3,
    texture: &Texture,
) {
    if v.x >= 1.
        && v.x < (size.width as f32) - 1.
        && v.y >= 1.
        && v.y < (size.height as f32) - 1.
        && let Some(i) = buffer_index(v, size)
    {
        let color = match texture {
            Texture::Color(col) => *col,
            // TODO: Better color calculus
            Texture::VertexColor(c0, c1, c2) => ((ColorF32::from_argb_u32(*c0)
                + ColorF32::from_argb_u32(*c1)
                + ColorF32::from_argb_u32(*c2))
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
