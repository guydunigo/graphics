use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use winit::dpi::PhysicalSize;

use crate::rasterizer::Engine;

#[derive(Default, Debug, Clone)]
pub struct ParIterEngine {
    depth_color_buffer: Arc<[AtomicU64]>,
}

impl ParIterEngine {
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

    fn clean_buffer(&mut self, size: PhysicalSize<u32>) -> u128 {
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

impl Engine for ParIterEngine {
    fn rasterize<B: std::ops::DerefMut<Target = [u32]>>(
        &mut self,
        settings: &crate::rasterizer::settings::Settings,
        text_writer: &crate::font::TextWriter,
        world: &crate::scene::World,
        buffer: &mut B,
        size: winit::dpi::PhysicalSize<u32>,
        app: crate::window::AppObserver,
        #[cfg(feature = "stats")] stats: &mut Stats,
    ) {
        let buffer_fill_time = self.clean_buffer(size);

        let t = Instant::now();
        rasterize(
            &self.world,
            &gfx.depth_color_buffer,
            &size,
            &self.settings,
            #[cfg(feature = "stats")]
            &stats,
        );
        let rendering_time = Instant::now().duration_since(t).as_millis();

        // TODO: copy from single thread
        {
            let cam_rot = self.world.camera.rot();
            #[cfg(feature = "stats")]
            let stats = format!("{:#?}", stats);
            #[cfg(not(feature = "stats"))]
            let stats = "Stats disabled";
            let display = format!(
                "fps : {} | {}ms - {}ms - {}ms / {}ms{}\n{} {} {} {}\n{:?}\n{}",
                (1000. / self.last_rendering_duration as f32).round(),
                buffers_fill,
                rendering_time,
                self.last_copy_buffer,
                self.last_rendering_duration,
                self.cursor
                    .and_then(|cursor| gfx
                        .depth_color_buffer
                        .get(cursor.x as usize + cursor.y as usize * size.width as usize)
                        .map(|c| format!(
                            "\n({},{}) 0x{:x}",
                            cursor.x.floor(),
                            cursor.y.floor(),
                            u64_to_color(c.load(Ordering::Relaxed))
                        )))
                    .unwrap_or(String::from("\nNo cursor position")),
                self.world.camera.pos,
                cam_rot.u(),
                cam_rot.v(),
                cam_rot.w(),
                self.settings,
                stats,
            );
            self.text_writer
                .rasterize_par(&gfx.depth_color_buffer, size, &display[..]);
        }

        let copy_buffer = Instant::now();
        let buffer = {
            let (Some(width), Some(height)) =
                (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
            else {
                return;
            };

            gfx.surface
                .resize(width, height)
                .expect("Failed to resize the softbuffer surface");

            let mut buffer = gfx
                .surface
                .buffer_mut()
                .expect("Failed to get the softbuffer buffer");

            (0..(size.width * size.height) as usize).for_each(|i| {
                buffer[i] = u64_to_color(gfx.depth_color_buffer[i].load(Ordering::Relaxed));
            });

            buffer
        };

        self.last_copy_buffer = Instant::now().duration_since(copy_buffer).as_millis();
    }
}
