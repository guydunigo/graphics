mod iterator;
mod original;

pub use iterator::IteratorEngine;
pub use original::OriginalEngine;

use std::{ops::DerefMut, time::Instant};
use winit::dpi::PhysicalSize;

use crate::{
    rasterizer::{Engine, Rasterizer},
    scene::{DEFAULT_BACKGROUND_COLOR, World},
    window::App,
};

/// Common base for engines not requiring buffer synchronization.
trait SingleThreadedEngine {
    fn depth_buffer_mut(&mut self) -> &mut Vec<f32>;

    fn rasterize_world<B: DerefMut<Target = [u32]>>(
        rasterizer: &Rasterizer,
        world: &World,
        buffer: &mut B,
        depth_buffer: &mut [f32],
        size: PhysicalSize<u32>,
        #[cfg(feature = "stats")] stats: &mut Stats,
    );
}

impl<T: SingleThreadedEngine> Engine for T {
    fn rasterize<B: DerefMut<Target = [u32]>>(
        &mut self,
        app: &App,
        buffer: &mut B,
        size: PhysicalSize<u32>,
    ) {
        let buffer_fill_time = clean_buffers(buffer, self.depth_buffer_mut(), size);

        let rendering_time = Instant::now();

        Self::rasterize_world(
            &app.rasterizer,
            &app.world,
            buffer,
            &mut self.depth_buffer_mut()[..],
            size,
            #[cfg(feature = "stats")]
            stats,
        );

        {
            let cam_rot = app.world.camera.rot();
            #[cfg(feature = "stats")]
            let stats = format!("{:#?}", stats);
            #[cfg(not(feature = "stats"))]
            let stats = "Stats disabled";
            let display = format!(
                "fps : {} | {}ms - {}ms / {}ms{}\n{} {} {} {}\n{:?}\n{}",
                (1000. / app.last_rendering_duration as f32).round(),
                buffer_fill_time,
                Instant::now().duration_since(rendering_time).as_millis(),
                app.last_rendering_duration,
                app.cursor
                    .and_then(|cursor| buffer
                        .get(cursor.x as usize + cursor.y as usize * size.width as usize)
                        .map(|c| format!(
                            "\n({},{}) 0x{:x}",
                            cursor.x.floor(),
                            cursor.y.floor(),
                            c
                        )))
                    .unwrap_or(String::from("\nNo cursor position")),
                app.world.camera.pos,
                cam_rot.u(),
                cam_rot.v(),
                cam_rot.w(),
                app.rasterizer,
                stats
            );
            app.rasterizer
                .text_writer
                .rasterize(buffer, size, &display[..]);
        }
    }
}

fn clean_buffers<B: DerefMut<Target = [u32]>>(
    buffer: &mut B,
    depth_buffer: &mut Vec<f32>,
    size: PhysicalSize<u32>,
) -> u128 {
    let t = Instant::now();
    buffer.fill(DEFAULT_BACKGROUND_COLOR);

    depth_buffer.resize(size.width as usize * size.height as usize, f32::INFINITY);
    depth_buffer.fill(f32::INFINITY);

    Instant::now().duration_since(t).as_millis()
}
