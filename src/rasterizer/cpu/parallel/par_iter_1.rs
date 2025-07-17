//! Like steps2 but parallel : par_drain
use glam::{Mat4, Vec3, Vec4Swizzles};
use rayon::prelude::*;
use std::{
    ops::DerefMut,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};
use winit::dpi::PhysicalSize;

use crate::{
    font::{self, TextWriter},
    maths::ColorF32,
    rasterizer::{
        cpu::{
            MINIMAL_AMBIANT_LIGHT, cursor_buffer_index, format_debug,
            parallel::{clean_resize_buffer, u64_to_color},
            single_threaded::populate_nodes_split,
            vec_cross_z,
        },
        settings::Settings,
    },
    scene::{BoundingBox, Texture, Triangle, World, to_raster},
    window::AppObserver,
};

use super::rasterize_triangle;

#[cfg(feature = "stats")]
use super::ParStats;
#[cfg(feature = "stats")]
use std::sync::atomic::Ordering;

#[derive(Default, Clone)]
pub struct ParIterEngine1 {
    triangles: Vec<(Vec3, Vec3, Vec3)>,
    world_trs: Vec<Mat4>,
    to_cam_trs: Vec<Mat4>,
    textures: Vec<Texture>,

    t_raster: Vec<(Vec3, Vec3, Vec3)>,
    bounding_boxes: Vec<BoundingBox<u32>>,
    p01p20: Vec<(Vec3, Vec3)>,
    depth_color_buffer: Arc<[AtomicU64]>,
}

impl ParIterEngine1 {
    fn rasterize_world(
        &mut self,
        settings: &Settings,
        world: &World,
        size: PhysicalSize<u32>,
        ratio_w_h: f32,
        #[cfg(feature = "stats")] stats: &ParStats,
    ) {
        // self.triangles.clear();
        // self.world_trs.clear();
        // self.to_cam_trs.clear();
        // self.textures.clear();
        let t = Instant::now();
        world.scene.if_present(|s| {
            let t = Instant::now();
            s.top_nodes().iter().for_each(|n| {
                populate_nodes_split(
                    settings,
                    &world.camera,
                    size,
                    ratio_w_h,
                    &mut self.triangles,
                    &mut self.world_trs,
                    &mut self.to_cam_trs,
                    &mut self.textures,
                    &n.read().unwrap(),
                )
            });
            println!("Populated nodes in : {}μs", t.elapsed().as_micros());
        });
        if !self.triangles.is_empty() {
            println!("  -> After node closure : {}μs", t.elapsed().as_micros());
        }

        #[cfg(feature = "stats")]
        {
            stats
                .nb_triangles_tot
                .store(self.triangles.len(), Ordering::Relaxed);
        }

        let camera = &world.camera;
        // self.t_raster.clear();
        // self.t_raster.reserve(self.triangles.len());
        self.t_raster.par_extend(
            self.triangles
                .par_iter()
                .zip(self.to_cam_trs.par_drain(..))
                .map(|((p0, p1, p2), tr)| {
                    (
                        to_raster(*p0, camera, &tr, size, ratio_w_h),
                        to_raster(*p1, camera, &tr, size, ratio_w_h),
                        to_raster(*p2, camera, &tr, size, ratio_w_h),
                    )
                }),
        );
        // No need for self.to_cam_trs anymore.

        // self.bounding_boxes.clear();
        // self.bounding_boxes.reserve(self.triangles.len());
        while self.bounding_boxes.len() < self.triangles.len() {
            let i = self.bounding_boxes.len();
            let bb = BoundingBox::new_2(self.t_raster[i], size);
            if !settings.culling_triangles || bb.is_visible(camera.z_near) {
                self.bounding_boxes.push(bb);
            } else {
                self.triangles.swap_remove(i);
                self.world_trs.swap_remove(i);
                self.textures.swap_remove(i);
                self.t_raster.swap_remove(i);
            }
        }

        #[cfg(feature = "stats")]
        {
            stats
                .nb_triangles_sight
                .store(self.triangles.len(), Ordering::Relaxed);
        }

        ////////////////////////////////
        // Back face culling
        // If triangle normal and camera sight are in same direction (cross product > 0),
        // it's invisible.
        // self.p01p20.clear();
        // self.p01p20.reserve(self.triangles.len());
        while self.p01p20.len() < self.triangles.len() {
            let i = self.p01p20.len();
            let (p0, p1, p2) = &self.t_raster[i];
            let (p01, p20) = (p1 - p0, p0 - p2);
            if vec_cross_z(p01, p20) >= 0. {
                self.p01p20.push((p01, p20));
            } else {
                self.triangles.swap_remove(i);
                self.world_trs.swap_remove(i);
                self.textures.swap_remove(i);
                self.t_raster.swap_remove(i);
                self.bounding_boxes.swap_remove(i);
            }
        }

        #[cfg(feature = "stats")]
        {
            stats
                .nb_triangles_facing
                .store(self.triangles.len(), Ordering::Relaxed);
        }

        self.triangles
            .par_iter_mut()
            .zip(self.world_trs.par_drain(..))
            .for_each(|((p0, p1, p2), tr)| {
                *p0 = (tr * p0.extend(1.)).xyz();
                *p1 = (tr * p1.extend(1.)).xyz();
                *p2 = (tr * p2.extend(1.)).xyz();
            });
        // No need for self.world_trs anymore.

        ////////////////////////////////
        // Sunlight
        // Dot product gives negative if two vectors are opposed, so we compare light
        // vector to face normal vector to see if they are opposed (face is lit).
        //
        // Also simplifying colours.
        let sun_direction = world.sun_direction;
        self.textures
            .par_iter_mut()
            .zip(self.triangles.par_drain(..))
            .for_each(|(texture, (p0, p1, p2))| {
                let triangle_normal = (p1 - p0).cross(p0 - p2).normalize();
                let light = sun_direction
                    .dot(triangle_normal)
                    .clamp(MINIMAL_AMBIANT_LIGHT, 1.);

                // TODO: remove this test, just load correctly ?
                // If a `Texture::VertexColor` has the same color for all triangles, then we can
                // consider it like a `Texture::Color`.
                if let Texture::VertexColor(c0, c1, c2) = texture
                    && c0 == c1
                    && c1 == c2
                {
                    *texture = Texture::Color(*c0);
                }

                match texture {
                    Texture::Color(col) => {
                        *texture =
                            Texture::Color((ColorF32::from_argb_u32(*col) * light).as_color_u32());
                    }
                    Texture::VertexColor(c0, c1, c2) => {
                        *c0 = (ColorF32::from_argb_u32(*c0) * light).as_color_u32();
                        *c1 = (ColorF32::from_argb_u32(*c1) * light).as_color_u32();
                        *c2 = (ColorF32::from_argb_u32(*c2) * light).as_color_u32();
                    }
                }
            });
        // No need for self.triangles anymore.

        self.t_raster
            .par_drain(..)
            .zip(self.textures.par_drain(..))
            .zip(self.bounding_boxes.par_drain(..))
            .zip(self.p01p20.par_drain(..))
            .for_each(|((((p0, p1, p2), material), bb), (p01, p20))| {
                rasterize_triangle(
                    &Triangle {
                        p0,
                        p1,
                        p2,
                        material,
                    },
                    &self.depth_color_buffer[..],
                    camera.z_near,
                    size,
                    settings,
                    #[cfg(feature = "stats")]
                    stats,
                    &bb,
                    p01,
                    p20,
                )
            });
    }

    pub fn rasterize<B: DerefMut<Target = [u32]>>(
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

        // If x4 is set :
        let mut font_size = font::PX;
        size.width *= settings.oversampling as u32;
        size.height *= settings.oversampling as u32;
        if settings.parallel_text {
            font_size *= settings.oversampling as f32;
        }

        app.last_buffer_fill_micros = clean_resize_buffer(&mut self.depth_color_buffer, size);

        let ratio_w_h = size.width as f32 / size.height as f32;

        {
            let t = Instant::now();
            #[cfg(feature = "stats")]
            let par_stats = ParStats::from(&*stats);
            self.rasterize_world(
                settings,
                world,
                size,
                ratio_w_h,
                #[cfg(feature = "stats")]
                &par_stats,
            );
            #[cfg(feature = "stats")]
            par_stats.update_stats(stats);
            app.last_rendering_micros = t.elapsed().as_micros();
        }

        if settings.parallel_text {
            let cursor_color = cursor_buffer_index(app.cursor(), size)
                .map(|index| u64_to_color(self.depth_color_buffer[index].load(Ordering::Relaxed)));
            let display = format_debug(
                settings,
                world,
                app,
                size,
                cursor_color,
                #[cfg(feature = "stats")]
                stats,
            );
            text_writer.rasterize_par(&self.depth_color_buffer[..], size, font_size, &display[..]);
        }

        // TODO: parallel (safe split ref vec)
        let t = Instant::now();
        if settings.oversampling > 1 {
            let oversampling_2 = settings.oversampling * settings.oversampling;

            let depth_color_buffer = &self.depth_color_buffer[..];
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
                buffer[i] = u64_to_color(self.depth_color_buffer[i].load(Ordering::Relaxed));
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
