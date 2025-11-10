//! Like thread_pool
use glam::{Mat4, Vec3, Vec4Swizzles, vec3};
use std::{
    ops::{DerefMut, Range},
    slice,
    sync::{
        Arc, RwLock,
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
            single_threaded::draw_vertice_basic, vec_cross_z,
        },
        settings::Settings,
    },
    scene::{
        BoundingBox, Camera, DEFAULT_BACKGROUND_COLOR, Node, Texture, Triangle, World, to_cam_tr,
        to_raster,
    },
    window::AppObserver,
};

const NB_THREADS: usize = 4;

#[derive(Debug, Clone)]
enum Msg {
    Resize { new_buf_len: usize },
    Compute,
    Merge { dst_ptr_range: Range<*mut u32> },
    Clear,
    Quit,
}

// Pointers aren't supposed to be sent...
// I know what I'm doing, trust me...
unsafe impl Send for Msg {}

#[derive(Default)]
struct ThreadLocalSharedData {
    buffer: Vec<u32>,
    depth: Vec<f32>,
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

    thread_shared: Arc<RwLock<ThreadLocalSharedData>>,

    all_thread_shared: Vec<Arc<RwLock<ThreadLocalSharedData>>>,

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
        all_thread_shared: Vec<Arc<RwLock<ThreadLocalSharedData>>>,
    ) -> Self {
        Self {
            thread_i,
            order_rx,
            end_tx,
            shared,

            thread_shared: all_thread_shared[thread_i].clone(),
            all_thread_shared,

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
                Msg::Resize { new_buf_len } => self.resize(new_buf_len),
                Msg::Compute => {
                    self.rasterize_world();
                    self.end_tx.send(()).unwrap();
                }
                Msg::Merge { dst_ptr_range } => {
                    self.merge(dst_ptr_range);
                    self.end_tx.send(()).unwrap();
                }
                Msg::Clear => self.clear(),
                Msg::Quit => break,
            }
        }
        println!("Thread {} stopping...", self.thread_i);
    }

    fn merge(&mut self, dst_ptr_range: Range<*mut u32>) {
        let src_buffers: Vec<_> = self
            .all_thread_shared
            .iter()
            .map(|b| b.read().unwrap())
            .collect();
        let dst_buffer = unsafe { slice::from_mut_ptr_range(dst_ptr_range) };
        dst_buffer
            .iter_mut()
            .enumerate()
            .skip(self.thread_i)
            .step_by(NB_THREADS)
            .for_each(|(pix_i, b)| {
                *b = src_buffers
                    .iter()
                    .map(|buffers| (buffers.buffer[pix_i], buffers.depth[pix_i]))
                    .min_by(|(_, depth1), (_, depth2)| f32::total_cmp(depth1, depth2))
                    .map(|(pix, _)| pix)
                    .unwrap();
            });
    }

    fn clear(&mut self) {
        // let t_start = Instant::now();
        let mut thread_shared = self.thread_shared.write().unwrap();
        // let t_lock = Instant::now();
        thread_shared.buffer.fill(DEFAULT_BACKGROUND_COLOR);
        thread_shared.depth.fill(f32::INFINITY);
        // println!(
        //     "Thread {} clear buffers : lock {}μs - tot {}μs",
        //     self.thread_i,
        //     t_lock.duration_since(t_start).as_micros(),
        //     t_start.elapsed().as_micros()
        // );
    }

    fn resize(&mut self, new_buf_len: usize) {
        let mut thread_shared = self.thread_shared.write().unwrap();
        thread_shared
            .buffer
            .resize(new_buf_len, DEFAULT_BACKGROUND_COLOR);
        thread_shared.depth.resize(new_buf_len, f32::INFINITY);

        self.indices.clear();
    }

    fn rasterize_world(&mut self) {
        // let t_start = Instant::now();

        let shared = self.shared.read().unwrap();
        let mut thread_shared = self.thread_shared.write().unwrap();

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
            thread_shared.nb_triangles_sight = self.indices.len();
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
            thread_shared.nb_triangles_facing = self.indices.len();
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
                    &mut thread_shared,
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
    pub fn spawn(
        thread_i: usize,
        all_thread_shared: Vec<Arc<RwLock<ThreadLocalSharedData>>>,
        shared: Arc<RwLock<SharedData>>,
    ) -> Self {
        let (order_tx, order_rx) = sync_channel(1);
        let (end_tx, end_rx) = sync_channel(1);

        let mut th = ThreadLocalData::new(thread_i, order_rx, end_tx, shared, all_thread_shared);
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

pub struct ThreadPoolEngine2 {
    /// List of spawned threads with :
    /// - Orders channel : Sender to send orders (start_index, count). Thread-side recv blocking.
    /// - Sync channel : Receiver to syncronize end of frame. Main-thread recv-blocking
    ///
    /// The thread should only block on the order channel.
    ///
    /// After an order, there should be an sync channel recv before the next order, or a quit command.
    thread_sync: Vec<WorkerThread>,
    #[cfg(feature = "stats")]
    all_thread_shared: Vec<Arc<RwLock<ThreadLocalSharedData>>>,
    shared: Arc<RwLock<SharedData>>,
}

impl Default for ThreadPoolEngine2 {
    fn default() -> Self {
        let shared: Arc<RwLock<SharedData>> = Default::default();

        let all_thread_shared: Vec<_> = (0..NB_THREADS).map(|_| Default::default()).collect();

        // TODO: based on cpu count ?
        let thread_sync = (0..NB_THREADS)
            .map(|i| WorkerThread::spawn(i, all_thread_shared.clone(), shared.clone()))
            .collect();

        Self {
            thread_sync,
            #[cfg(feature = "stats")]
            all_thread_shared,
            shared,
        }
    }
}

impl Drop for ThreadPoolEngine2 {
    fn drop(&mut self) {
        self.thread_sync.drain(..).for_each(|worker| {
            // Sending count=0 to tell it to quit.
            // Returns Err() if already disconnected : no need to report error.
            let _ = worker.order_tx.send(Msg::Quit);
            worker.handle.join().unwrap();
        });
    }
}

impl ThreadPoolEngine2 {
    fn rasterize_world<B: DerefMut<Target = [u32]>>(
        &mut self,
        settings: &Settings,
        world: &World,
        buffer: &mut B,
        size: PhysicalSize<u32>,
        ratio_w_h: f32,
        #[cfg(feature = "stats")] stats: &mut Stats,
    ) {
        self.thread_sync.iter().for_each(|worker| {
            worker
                .order_tx
                .send(Msg::Resize {
                    new_buf_len: buffer.len(),
                })
                .unwrap()
        });

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

        self.thread_sync
            .iter()
            .for_each(|worker| worker.order_tx.send(Msg::Compute).unwrap());
        self.thread_sync
            .iter()
            .for_each(|worker| worker.end_rx.recv().unwrap());

        let t_start_merge = Instant::now();
        /*
        let buffers: Vec<_> = self
            .thread_sync
            .iter()
            .map(|t| t.thread_shared.read().unwrap())
            .collect();
        // buffer.fill(DEFAULT_BACKGROUND_COLOR);
        buffer.iter_mut().enumerate().for_each(|(pix_i, b)| {
            *b = buffers
                .iter()
                .map(|buffers| (buffers.0[pix_i], buffers.1[pix_i]))
                .min_by(|(_, depth1), (_, depth2)| f32::total_cmp(depth1, depth2))
                .map(|(pix, _)| pix)
                .unwrap();
        });
        */

        let dst_ptr_range = buffer.as_mut_ptr_range();
        self.thread_sync.iter().for_each(|worker| {
            worker
                .order_tx
                .send(Msg::Merge {
                    dst_ptr_range: dst_ptr_range.clone(),
                })
                .unwrap()
        });
        self.thread_sync
            .iter()
            .for_each(|worker| worker.end_rx.recv().unwrap());
        println!("Merged : {}μs", t_start_merge.elapsed().as_micros());

        self.thread_sync
            .iter()
            .for_each(|worker| worker.order_tx.send(Msg::Clear).unwrap());

        #[cfg(feature = "stats")]
        self.all_thread_shared.iter().for_each(|t| {
            let thread_shared = t.read().unwrap();
            stats.nb_triangles_sight += thread_shared.nb_triangles_sight;
            stats.nb_triangles_facing += thread_shared.nb_triangles_facing;
            stats.nb_triangles_drawn += thread_shared.nb_triangles_drawn;
            stats.nb_pixels_tested += thread_shared.nb_pixels_tested;
            stats.nb_pixels_in += thread_shared.nb_pixels_in;
            stats.nb_pixels_front += thread_shared.nb_pixels_front;
            stats.nb_pixels_written += thread_shared.nb_pixels_written;
        });
    }

    pub fn rasterize<B: DerefMut<Target = [u32]>>(
        &mut self,
        settings: &Settings,
        text_writer: &TextWriter,
        world: &World,
        buffer: &mut B,
        size: PhysicalSize<u32>,
        app: &mut AppObserver,
        #[cfg(feature = "stats")] stats: &mut Stats,
    ) {
        let t = Instant::now();
        app.last_buffer_fill_micros = t.elapsed().as_micros();

        let ratio_w_h = size.width as f32 / size.height as f32;

        let t = Instant::now();
        self.rasterize_world(
            settings,
            world,
            buffer,
            size,
            ratio_w_h,
            #[cfg(feature = "stats")]
            stats,
        );
        app.last_rendering_micros = t.elapsed().as_micros();

        {
            let cursor_color = cursor_buffer_index(app.cursor(), size).map(|index| buffer[index]);
            let display = format_debug(
                settings,
                world,
                app,
                size,
                cursor_color,
                #[cfg(feature = "stats")]
                stats,
            );
            text_writer.rasterize(buffer, size, font::PX, &display[..]);
        }
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
fn rasterize_triangle<B: DerefMut<Target = ThreadLocalSharedData>>(
    settings: &Settings,
    tri_raster: &Triangle,
    thread_shared: &mut B,
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
                thread_shared.nb_pixels_tested += 1;
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
                thread_shared.nb_pixels_in += 1;
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
                thread_shared.nb_pixels_front += 1;
            }

            let index = (pixel.x as usize) + (pixel.y as usize) * size.width as usize;

            if depth >= thread_shared.depth[index] {
                return;
            }

            #[cfg(feature = "stats")]
            {
                was_drawn = true;
                thread_shared.nb_pixels_written += 1;
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

            thread_shared.buffer[index] = col;
            thread_shared.depth[index] = depth;
        });

    #[cfg(feature = "stats")]
    if was_drawn {
        thread_shared.nb_triangles_drawn += 1;
    }

    if settings.show_vertices {
        draw_vertice_basic(
            &mut thread_shared.buffer,
            size,
            tri_raster.p0,
            &tri_raster.material,
        );
        draw_vertice_basic(
            &mut thread_shared.buffer,
            size,
            tri_raster.p1,
            &tri_raster.material,
        );
        draw_vertice_basic(
            &mut thread_shared.buffer,
            size,
            tri_raster.p2,
            &tri_raster.material,
        );
    }
}
