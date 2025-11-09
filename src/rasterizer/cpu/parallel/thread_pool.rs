//! Like steps2 but splitting work across fixed manual threads.
//!
//! Each thread acts like separate steps2.
//! Then we merge resulting buffers based on depth buffers comp.
use glam::{Mat4, Vec3, Vec4Swizzles};
use rayon::prelude::*;
use std::{
    ops::DerefMut,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{Receiver, SyncSender, TryRecvError, sync_channel},
    },
    thread::{JoinHandle, spawn},
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
use crate::rasterizer::Stats;

const NB_THREADS: usize = 3;

pub struct ThreadPoolEngine {
    /// List of spawned threads with :
    /// - Orders channel : Sender to send orders (start_index, count). Thread-side recv blocking.
    /// - Sync channel : Receiver to syncronize end of frame. Main-thread recv-blocking
    ///
    /// The thread should only block on the order channel.
    /// When sending count=0, asks the thread to stop, to be joined with the `JoinHandle`
    ///
    /// After an order, there should be an sync channel recv before the next order, or a quit command.
    thread_sync: Vec<(JoinHandle<()>, SyncSender<(usize, usize)>, Receiver<()>)>,

    // triangles: Vec<(Vec3, Vec3, Vec3)>,
    // world_trs: Vec<Mat4>,
    // to_cam_trs: Vec<Mat4>,
    // textures: Vec<Texture>,

    // t_raster: Vec<(Vec3, Vec3, Vec3)>,
    // bounding_boxes: Vec<BoundingBox<u32>>,
    // p01p20: Vec<(Vec3, Vec3)>,
    depth_color_buffer: Arc<[AtomicU64]>,
}

impl Default for ThreadPoolEngine {
    fn default() -> Self {
        // TODO: based on cpu count ?
        let thread_sync = (0..NB_THREADS)
            .map(|i| {
                let (trig_tx, trig_rx) = sync_channel(1);
                let (end_tx, end_rx) = sync_channel(1);

                let jh = spawn(move || thread_step2(i, trig_rx, end_tx));

                return (jh, trig_tx, end_rx);
            })
            .collect();

        Self {
            thread_sync,
            depth_color_buffer: Default::default(),
        }
    }
}

impl Drop for ThreadPoolEngine {
    fn drop(&mut self) {
        self.thread_sync.drain(..).for_each(|(jh, tx, _)| {
            // Sending count=0 to tell it to quit.
            // Returns Err() if already disconnected : no need to report error.
            let _ = tx.send((0, 0));
            jh.join().unwrap();
        });
    }
}

impl ThreadPoolEngine {
    fn rasterize_world(
        &mut self,
        _settings: &Settings,
        _world: &World,
        _size: PhysicalSize<u32>,
        _ratio_w_h: f32,
        #[cfg(feature = "stats")] stats: &ParStats,
    ) {
        self.thread_sync
            .iter()
            .for_each(|(_, tx, _)| tx.send((0, 10)).unwrap());
        self.thread_sync
            .iter()
            .for_each(|(_, _, rx)| rx.recv().unwrap());
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

fn thread_step2(thread_i: usize, trig_rx: Receiver<(usize, usize)>, end_tx: SyncSender<()>) {
    loop {
        let (index, count) = trig_rx.recv().unwrap();
        println!("Thread {thread_i} recieved (index, count) = ({index}, {count})");
        if count == 0 {
            println!("Thread {thread_i} count == 0, quitting...");
            break;
        }
        end_tx.send(()).unwrap();
    }
    println!("Thread {thread_i} dying...");
}
