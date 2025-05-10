mod iterator;
mod original;

pub use iterator::IteratorEngine;
pub use original::OriginalEngine;

use std::{ops::DerefMut, time::Instant};
use winit::dpi::PhysicalSize;

use crate::{
    font::TextWriter,
    rasterizer::{Engine, Settings},
    scene::{DEFAULT_BACKGROUND_COLOR, World},
    window::AppObserver,
};

/// Common base for engines not requiring buffer synchronization.
trait SingleThreadedEngine {
    fn depth_buffer_mut(&mut self) -> &mut Vec<f32>;

    fn rasterize_world<B: DerefMut<Target = [u32]>>(
        settings: &Settings,
        world: &World,
        buffer: &mut B,
        depth_buffer: &mut [f32],
        size: PhysicalSize<u32>,
        #[cfg(feature = "stats")] stats: &mut Stats,
    );

    /// - Resize `depth_buffer` and fill it with inifite depth
    /// - Fill `buffer` with `DEFAULT_BACKGROUND_COLOR`
    /// `buffer` should be already resized.
    fn clean_resize_buffers<B: DerefMut<Target = [u32]>>(
        &mut self,
        buffer: &mut B,
        size: PhysicalSize<u32>,
    ) -> u128 {
        let t = Instant::now();
        buffer.fill(DEFAULT_BACKGROUND_COLOR);

        self.depth_buffer_mut()
            .resize(size.width as usize * size.height as usize, f32::INFINITY);
        self.depth_buffer_mut().fill(f32::INFINITY);

        Instant::now().duration_since(t).as_millis()
    }
}

impl<T: SingleThreadedEngine> Engine for T {
    fn rasterize<B: DerefMut<Target = [u32]>>(
        &mut self,
        settings: &Settings,
        text_writer: &TextWriter,
        world: &World,
        buffer: &mut B,
        size: PhysicalSize<u32>,
        app: AppObserver,
    ) {
        let buffer_fill_time = self.clean_resize_buffers(buffer, size);

        let t = Instant::now();
        Self::rasterize_world(
            settings,
            world,
            buffer,
            &mut self.depth_buffer_mut()[..],
            size,
            #[cfg(feature = "stats")]
            stats,
        );
        let rendering_time = Instant::now().duration_since(t).as_millis();

        display_debug(
            settings,
            text_writer,
            world,
            buffer,
            size,
            app,
            buffer_fill_time,
            rendering_time,
        );
    }
}

fn display_debug<B: DerefMut<Target = [u32]>>(
    settings: &Settings,
    text_writer: &TextWriter,
    world: &World,
    buffer: &mut B,
    size: PhysicalSize<u32>,
    app: AppObserver,
    buffer_fill_time: u128,
    rendering_time: u128,
    #[cfg(feature = "stats")] stats: &Stats,
) {
    let cam_rot = world.camera.rot();
    #[cfg(feature = "stats")]
    let stats = format!("{:#?}", stats);
    #[cfg(not(feature = "stats"))]
    let stats = "Stats disabled";
    let display = format!(
        "fps : {} {} | {}ms - {}ms / {}ms / {}ms{}\n{} {} {} {}\n{:?}\n{}",
        (1000. / app.last_frame_duration as f32).round(),
        (1000. / app.last_rendering_duration as f32).round(),
        buffer_fill_time,
        rendering_time,
        app.last_rendering_duration,
        app.last_frame_duration,
        app.cursor
            .and_then(|cursor| buffer
                .get(cursor.x as usize + cursor.y as usize * size.width as usize)
                .map(|c| format!("\n({},{}) 0x{:x}", cursor.x.floor(), cursor.y.floor(), c)))
            .unwrap_or(String::from("\nNo cursor position")),
        world.camera.pos,
        cam_rot.u(),
        cam_rot.v(),
        cam_rot.w(),
        settings,
        stats
    );
    text_writer.rasterize(buffer, size, &display[..]);
}
