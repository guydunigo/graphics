//! Like thread_pool, but we use an atomic array, so each thread merges its
//! own buffers into it.
use glam::{Mat4, Vec3, Vec4Swizzles, vec3};
use std::{
    ops::DerefMut,
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    thread::{JoinHandle, spawn},
    time::Instant,
};
use winit::dpi::PhysicalSize;

#[cfg(feature = "stats")]
use crate::rasterizer::Stats;
use crate::{
    font::{self, TextWriter},
    maths::ColorF32,
    rasterizer::{
        cpu::{
            MINIMAL_AMBIANT_LIGHT, cursor_buffer_index, edge_function, format_debug,
            parallel::{
                clean_resize_buffer, depth_to_u64, draw_vertice_basic, thread_pool::NB_THREADS,
                u64_to_color,
            },
            vec_cross_z,
        },
        settings::Settings,
    },
    scene::{BoundingBox, Camera, Node, Texture, Triangle, World, to_cam_tr, to_raster},
    window::AppObserver,
};

#[derive(Debug, Clone)]
enum Msg {
    Compute {
        depth_color_buffer: Arc<[AtomicU64]>,
    },
    Quit,
}

// Pointers aren't supposed to be sent...
// I know what I'm doing, trust me...
unsafe impl Send for Msg {}

#[cfg(feature = "stats")]
#[derive(Default)]
struct ThreadStats {
    #[cfg(feature = "stats")]
    nb_triangles_sight: usize,
    #[cfg(feature = "stats")]
    nb_triangles_facing: usize,
    #[cfg(feature = "stats")]
    nb_triangles_drawn: usize,
    #[cfg(feature = "stats")]
    nb_pixels_tested: usize,
    #[cfg(feature = "stats")]
    nb_pixels_in: usize,
    #[cfg(feature = "stats")]
    nb_pixels_front: usize,
    #[cfg(feature = "stats")]
    nb_pixels_written: usize,
}

/// Data needed inside the thread
struct ThreadLocalData {
    thread_i: usize,

    order_rx: Receiver<Msg>,
    end_tx: SyncSender<()>,

    shared: Arc<RwLock<SharedData>>,

    #[cfg(feature = "stats")]
    stats: Arc<RwLock<ThreadStats>>,

    indices: Vec<usize>,
    triangles: Vec<(Vec3, Vec3, Vec3)>,
    textures: Vec<Texture>,

    t_raster: Vec<(Vec3, Vec3, Vec3)>,
    bounding_boxes: Vec<BoundingBox<u32>>,
    p01p20: Vec<(Vec3, Vec3)>,
}

impl ThreadLocalData {
    pub fn new(
        thread_i: usize,
        order_rx: Receiver<Msg>,
        end_tx: SyncSender<()>,
        shared: Arc<RwLock<SharedData>>,
        #[cfg(feature = "stats")] stats: Arc<RwLock<ThreadStats>>,
    ) -> Self {
        Self {
            thread_i,
            order_rx,
            end_tx,
            shared,

            #[cfg(feature = "stats")]
            stats,

            indices: Default::default(),
            triangles: Default::default(),
            textures: Default::default(),
            t_raster: Default::default(),
            bounding_boxes: Default::default(),
            p01p20: Default::default(),
        }
    }

    fn rasterize_loop(&mut self) {
        loop {
            let msg = self.order_rx.recv().unwrap();
            // println!("Thread {} recieved msg : {:?}", self.thread_i, msg);
            match msg {
                Msg::Compute { depth_color_buffer } => {
                    self.rasterize_world(&depth_color_buffer);
                    self.end_tx.send(()).unwrap();
                    self.indices.clear();
                }
                Msg::Quit => break,
            }
        }
        println!("Thread {} stopping...", self.thread_i);
    }

    fn rasterize_world(&mut self, depth_color_buffer: &[AtomicU64]) {
        // let t_start = Instant::now();

        let shared = self.shared.read().unwrap();
        #[cfg(feature = "stats")]
        let mut stats = self.stats.write().unwrap();

        // let t_lock = Instant::now();

        // We will work on every NB_THREADS of triangle collection : 0, 3, 6, ...
        self.indices
            .extend((self.thread_i..shared.triangles.len()).step_by(NB_THREADS));

        // self.triangles.clear();
        // self.textures.clear();

        // self.t_raster.clear();
        // self.t_raster.reserve(self.triangles.len());
        self.t_raster.extend(
            self.indices
                .iter()
                .map(|i| (&shared.triangles[*i], &shared.to_cam_trs[*i]))
                .map(|((p0, p1, p2), tr)| {
                    (
                        to_raster(*p0, &shared.camera, tr, shared.size, shared.ratio_w_h),
                        to_raster(*p1, &shared.camera, tr, shared.size, shared.ratio_w_h),
                        to_raster(*p2, &shared.camera, tr, shared.size, shared.ratio_w_h),
                    )
                }),
        );

        // self.bounding_boxes.clear();
        // self.bounding_boxes.reserve(self.triangles.len());
        while self.bounding_boxes.len() < self.indices.len() {
            let i = self.bounding_boxes.len();
            let bb = BoundingBox::new_2(self.t_raster[i], shared.size);
            if !shared.settings.culling_triangles || bb.is_visible(shared.camera.z_near) {
                self.bounding_boxes.push(bb);
            } else {
                self.indices.swap_remove(i);
                self.t_raster.swap_remove(i);
            }
        }

        #[cfg(feature = "stats")]
        {
            stats.nb_triangles_sight = self.indices.len();
        }

        ////////////////////////////////
        // Back face culling
        // If triangle normal and camera sight are in same direction (cross product > 0),
        // it's invisible.
        // self.p01p20.clear();
        // self.p01p20.reserve(self.triangles.len());
        while self.p01p20.len() < self.indices.len() {
            let i = self.p01p20.len();
            let (p0, p1, p2) = &self.t_raster[i];
            let (p01, p20) = (p1 - p0, p0 - p2);
            if vec_cross_z(p01, p20) >= 0. {
                self.p01p20.push((p01, p20));
            } else {
                self.indices.swap_remove(i);
                self.t_raster.swap_remove(i);
                self.bounding_boxes.swap_remove(i);
            }
        }

        #[cfg(feature = "stats")]
        {
            stats.nb_triangles_facing = self.indices.len();
        }

        self.triangles.extend(
            self.indices
                .iter()
                .map(|i| (&shared.triangles[*i], &shared.world_trs[*i]))
                .map(|((p0, p1, p2), tr)| {
                    (
                        (tr * p0.extend(1.)).xyz(),
                        (tr * p1.extend(1.)).xyz(),
                        (tr * p2.extend(1.)).xyz(),
                    )
                }),
        );
        // No need for self.world_trs anymore.

        ////////////////////////////////
        // Sunlight
        // Dot product gives negative if two vectors are opposed, so we compare light
        // vector to face normal vector to see if they are opposed (face is lit).
        //
        // Also simplifying colours.
        self.textures
            .extend(
                self.indices
                    .iter()
                    .zip(self.triangles.drain(..))
                    .map(|(i, (p0, p1, p2))| {
                        let mut texture = shared.textures[*i];

                        let triangle_normal = (p1 - p0).cross(p0 - p2).normalize();
                        let light = shared
                            .sun_direction
                            .dot(triangle_normal)
                            .clamp(MINIMAL_AMBIANT_LIGHT, 1.);

                        // TODO: remove this test, just load correctly ?
                        // If a `Texture::VertexColor` has the same color for all triangles, then we can
                        // consider it like a `Texture::Color`.
                        if let Texture::VertexColor(c0, c1, c2) = texture
                            && c0 == c1
                            && c1 == c2
                        {
                            texture = Texture::Color(c0);
                        }

                        match &mut texture {
                            Texture::Color(col) => {
                                texture = Texture::Color(
                                    (ColorF32::from_argb_u32(*col) * light).as_color_u32(),
                                );
                            }
                            Texture::VertexColor(c0, c1, c2) => {
                                *c0 = (ColorF32::from_argb_u32(*c0) * light).as_color_u32();
                                *c1 = (ColorF32::from_argb_u32(*c1) * light).as_color_u32();
                                *c2 = (ColorF32::from_argb_u32(*c2) * light).as_color_u32();
                            }
                        }

                        texture
                    }),
            );
        // No need for self.triangles anymore.

        // let nb_triangles_drawn = self.t_raster.len();
        self.t_raster
            .drain(..)
            .zip(self.textures.drain(..))
            .zip(self.bounding_boxes.drain(..))
            .zip(self.p01p20.drain(..))
            .for_each(|((((p0, p1, p2), material), bb), (p01, p20))| {
                rasterize_triangle(
                    &shared.settings,
                    &Triangle {
                        p0,
                        p1,
                        p2,
                        material,
                    },
                    &depth_color_buffer,
                    #[cfg(feature = "stats")]
                    &mut stats,
                    shared.camera.z_near,
                    shared.size,
                    &bb,
                    p01,
                    p20,
                )
            });
        // println!(
        //     "Thread {} processed {} triangles in : lock {}μs - tot {}μs",
        //     self.thread_i,
        //     nb_triangles_drawn,
        //     t_lock.duration_since(t_start).as_micros(),
        //     t_start.elapsed().as_micros()
        // );
    }
}

/// Thread data from the main thread to control it.
struct WorkerThread {
    handle: JoinHandle<()>,
    order_tx: SyncSender<Msg>,
    end_rx: Receiver<()>,
}

impl WorkerThread {
    pub fn spawn(thread_i: usize, shared: Arc<RwLock<SharedData>>) -> Self {
        let (order_tx, order_rx) = sync_channel(1);
        let (end_tx, end_rx) = sync_channel(1);

        let mut th = ThreadLocalData::new(
            thread_i,
            order_rx,
            end_tx,
            shared,
            #[cfg(feature = "stats")]
            stats,
        );
        let handle = spawn(move || th.rasterize_loop());

        return Self {
            handle,
            order_tx,
            end_rx,
        };
    }
}

#[derive(Default)]
struct SharedData {
    pub triangles: Vec<(Vec3, Vec3, Vec3)>,
    pub world_trs: Vec<Mat4>,
    pub to_cam_trs: Vec<Mat4>,
    pub textures: Vec<Texture>,

    pub settings: Settings,
    pub size: PhysicalSize<u32>,
    pub ratio_w_h: f32,
    pub camera: Camera,
    pub sun_direction: Vec3,
}

impl SharedData {
    pub fn clear(&mut self) {
        self.triangles.clear();
        self.world_trs.clear();
        self.to_cam_trs.clear();
        self.textures.clear();
    }
}

// From steps2
// TODO: rapporter dans steps2 ?
fn populate_nodes_split(
    settings: &Settings,
    camera: &Camera,
    size: PhysicalSize<u32>,
    ratio_w_h: f32,
    shared: &mut impl DerefMut<Target = SharedData>,
    node: &Node,
) {
    {
        let to_cam_tr = to_cam_tr(camera, &node.world_transform);
        if let Some(mesh) = node.mesh.as_ref()
            && (!settings.culling_meshes
                || mesh
                    .bounds
                    .is_visible_cpu(camera, &to_cam_tr, size, ratio_w_h))
        {
            // let vert_count = mesh.surfaces.iter().map(|s| s.count).sum::<usize>() / 3;
            // triangles.reserve(vert_count);
            // world_trs.reserve(vert_count);
            // to_cam_trs.reserve(vert_count);
            // textures.reserve(vert_count);
            mesh.surfaces
                .iter()
                .filter(|s| {
                    !settings.culling_surfaces
                        || s.bounds.is_visible_cpu(camera, &to_cam_tr, size, ratio_w_h)
                })
                .for_each(|s| {
                    mesh.indices[s.start_index..s.start_index + s.count]
                        .chunks_exact(3)
                        .for_each(|is| {
                            shared.triangles.push((
                                mesh.vertices[is[0]].position,
                                mesh.vertices[is[1]].position,
                                mesh.vertices[is[2]].position,
                            ));
                            shared.world_trs.push(node.world_transform);
                            shared.to_cam_trs.push(to_cam_tr);

                            let material = if settings.vertex_color {
                                let (c0, c1, c2) = if settings.vertex_color_normal {
                                    (
                                        mesh.vertices[is[0]].normal.extend(1.),
                                        mesh.vertices[is[1]].normal.extend(1.),
                                        mesh.vertices[is[2]].normal.extend(1.),
                                    )
                                } else {
                                    (
                                        mesh.vertices[is[0]].color,
                                        mesh.vertices[is[1]].color,
                                        mesh.vertices[is[2]].color,
                                    )
                                };
                                if c0 == c1 && c0 == c2 {
                                    Texture::Color(
                                        ColorF32::from_rgba(c0.to_array()).as_color_u32(),
                                    )
                                } else {
                                    Texture::VertexColor(
                                        ColorF32::from_rgba(c0.to_array()).as_color_u32(),
                                        ColorF32::from_rgba(c1.to_array()).as_color_u32(),
                                        ColorF32::from_rgba(c2.to_array()).as_color_u32(),
                                    )
                                }
                            } else {
                                s.material
                            };
                            shared.textures.push(material);
                        });
                });
        }
    }

    node.children.iter().for_each(|c| {
        populate_nodes_split(
            settings,
            camera,
            size,
            ratio_w_h,
            shared,
            &c.read().unwrap(),
        )
    });
}

// From single_threaded/mod.rs
fn rasterize_triangle(
    settings: &Settings,
    tri_raster: &Triangle,
    depth_color_buffer: &[AtomicU64],
    #[cfg(feature = "stats")] stats: &mut impl DerefMut<Target = ThreadStats>,
    z_near: f32,
    size: PhysicalSize<u32>,
    bb: &BoundingBox<u32>,
    p01: Vec3,
    p20: Vec3,
) {
    #[cfg(feature = "stats")]
    let mut was_drawn = false;

    let p12 = tri_raster.p2 - tri_raster.p1;

    let tri_area = edge_function(p20, p01);

    (bb.min_x..=bb.max_x)
        .flat_map(|x| (bb.min_y..=bb.max_y).map(move |y| vec3(x as f32, y as f32, 0.)))
        .for_each(|pixel| {
            // TODO: split to iterator ?

            #[cfg(feature = "stats")]
            {
                stats.nb_pixels_tested += 1;
            }

            let e01 = edge_function(p01, pixel - tri_raster.p0);
            let e12 = edge_function(p12, pixel - tri_raster.p1);
            let e20 = edge_function(p20, pixel - tri_raster.p2);

            // If negative for the 3 : we're outside the triangle.
            if e01 < 0. || e12 < 0. || e20 < 0. {
                return;
            }

            #[cfg(feature = "stats")]
            {
                stats.nb_pixels_in += 1;
            }

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
            {
                stats.nb_pixels_front += 1;
            }

            let index = (pixel.x as usize) + (pixel.y as usize) * size.width as usize;

            #[cfg(feature = "stats")]
            {
                was_drawn = true;
                stats.nb_pixels_written += 1;
            }

            let col = match tri_raster.material {
                Texture::Color(col) => col,
                Texture::VertexColor(c0, c1, c2) => {
                    // TODO: Optimize color calculus
                    let col_0 = ColorF32::from_argb_u32(c0) / tri_raster.p0.z;
                    let col_1 = ColorF32::from_argb_u32(c1) / tri_raster.p1.z;
                    let col_2 = ColorF32::from_argb_u32(c2) / tri_raster.p2.z;

                    ((col_2 + (col_0 - col_2) * a12 + (col_1 - col_2) * a20) * depth).as_color_u32()
                }
            };

            depth_color_buffer[index]
                .fetch_min(col as u64 | depth_to_u64(depth), Ordering::Relaxed);
        });

    #[cfg(feature = "stats")]
    if was_drawn {
        stats.nb_triangles_drawn += 1;
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

pub struct ThreadPoolEngine1 {
    /// List of spawned threads with :
    /// - Orders channel : Sender to send orders (start_index, count). Thread-side recv blocking.
    /// - Sync channel : Receiver to syncronize end of frame. Main-thread recv-blocking
    ///
    /// The thread should only block on the order channel.
    ///
    /// After an order, there should be an sync channel recv before the next order, or a quit command.
    thread_sync: Vec<WorkerThread>,
    #[cfg(feature = "stats")]
    all_stats: Vec<Arc<RwLock<ThreadStats>>>,
    shared: Arc<RwLock<SharedData>>,
    depth_color_buffer: Arc<[AtomicU64]>,
    last_cursor_color: Option<u32>,
}

impl Default for ThreadPoolEngine1 {
    fn default() -> Self {
        let shared: Arc<RwLock<SharedData>> = Default::default();

        #[cfg(feature = "stats")]
        let all_stats: Vec<_> = (0..NB_THREADS).map(|_| Default::default()).collect();

        // TODO: based on cpu count ?
        let thread_sync = (0..NB_THREADS)
            .map(|i| {
                WorkerThread::spawn(
                    i,
                    shared.clone(),
                    #[cfg(feature = "stats")]
                    all_stats[thread_i].clone(),
                )
            })
            .collect();

        Self {
            thread_sync,
            #[cfg(feature = "stats")]
            all_stats,
            shared,
            depth_color_buffer: Default::default(),
            last_cursor_color: Default::default(),
        }
    }
}

impl Drop for ThreadPoolEngine1 {
    fn drop(&mut self) {
        self.thread_sync.drain(..).for_each(|worker| {
            // Sending count=0 to tell it to quit.
            // Returns Err() if already disconnected : no need to report error.
            let _ = worker.order_tx.send(Msg::Quit);
            worker.handle.join().unwrap();
        });
    }
}

impl ThreadPoolEngine1 {
    pub fn rasterize<B: DerefMut<Target = [u32]>>(
        &mut self,
        settings: &Settings,
        text_writer: &TextWriter,
        world: &World,
        mut buffer: &mut B,
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

        let t = Instant::now();
        // Fill triangles to work on :
        {
            // let t = Instant::now();
            let mut shared = self.shared.write().unwrap();
            // Since we share, we can't drain, so we need to clean directly.
            shared.clear();
            world.scene.if_present(|s| {
                // let t = Instant::now();
                s.top_nodes().iter().for_each(|n| {
                    populate_nodes_split(
                        settings,
                        &world.camera,
                        size,
                        ratio_w_h,
                        &mut shared,
                        &n.read().unwrap(),
                    )
                });
                // println!("Populated nodes in : {}μs", t.elapsed().as_micros());
            });
            // if !shared.triangles.is_empty() {
            //     println!("  -> After node closure : {}μs", t.elapsed().as_micros());
            // }

            shared.settings = *settings;
            shared.size = size;
            shared.ratio_w_h = ratio_w_h;
            shared.camera = world.camera;
            shared.sun_direction = world.sun_direction;

            #[cfg(feature = "stats")]
            {
                stats.nb_triangles_tot = shared.triangles.len();
            }
        };

        self.thread_sync.iter().for_each(|worker| {
            worker
                .order_tx
                .send(Msg::Compute {
                    depth_color_buffer: self.depth_color_buffer.clone(),
                })
                .unwrap()
        });

        buffer.fill(0);
        if settings.parallel_text {
            let display = format_debug(
                settings,
                world,
                app,
                size,
                self.last_cursor_color,
                #[cfg(feature = "stats")]
                stats,
            );
            // text_writer.rasterize_atomic(&self.depth_color_buffer, size, font_size, &display[..]);
            text_writer.rasterize(&mut buffer, size, font_size, &display[..]);
        }

        self.thread_sync
            .iter()
            .for_each(|worker| worker.end_rx.recv().unwrap());

        #[cfg(feature = "stats")]
        self.all_stats.iter().for_each(|t| {
            let stats = t.read().unwrap();
            stats.nb_triangles_sight += stats.nb_triangles_sight;
            stats.nb_triangles_facing += stats.nb_triangles_facing;
            stats.nb_triangles_drawn += stats.nb_triangles_drawn;
            stats.nb_pixels_tested += stats.nb_pixels_tested;
            stats.nb_pixels_in += stats.nb_pixels_in;
            stats.nb_pixels_front += stats.nb_pixels_front;
            stats.nb_pixels_written += stats.nb_pixels_written;
        });
        app.last_rendering_micros = t.elapsed().as_micros();

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
                        buffer[j + i] |= color_avg.as_color_u32();
                    });
                });
        } else {
            (0..(size.width * size.height) as usize).for_each(|i| {
                buffer[i] |= u64_to_color(self.depth_color_buffer[i].load(Ordering::Relaxed));
            });
        }
        app.last_buffer_copy_micros = t.elapsed().as_micros();

        self.last_cursor_color = cursor_buffer_index(app.cursor(), size).map(|index| buffer[index]);
        if !settings.parallel_text {
            let display = format_debug(
                settings,
                world,
                app,
                size,
                self.last_cursor_color,
                #[cfg(feature = "stats")]
                stats,
            );
            text_writer.rasterize(buffer, size, font::PX, &display[..]);
        }
    }
}
