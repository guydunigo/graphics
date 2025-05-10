mod par_iter;
mod rasterizer;
mod scene;
mod window;

/// Common base for engines requiring buffer usable in a parallel context.
trait ParallelEngine {
    fn depth_buffer_mut(&mut self) -> &mut Vec<f32>;

    fn rasterize_world<B: DerefMut<Target = [u32]>>(
        settings: &Settings,
        world: &World,
        buffer: &mut B,
        depth_buffer: &mut [f32],
        size: PhysicalSize<u32>,
        #[cfg(feature = "stats")] stats: &mut Stats,
    );
}

impl<T: ParallelEngine> Engine for T {
    fn rasterize<B: DerefMut<Target = [u32]>>(
        &mut self,
        settings: &Settings,
        text_writer: &TextWriter,
        world: &World,
        buffer: &mut B,
        size: PhysicalSize<u32>,
        app: AppObserver,
    ) {
        let buffer_fill_time = clean_buffers(buffer, self.depth_buffer_mut(), size);

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

        {
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
                        .map(|c| format!(
                            "\n({},{}) 0x{:x}",
                            cursor.x.floor(),
                            cursor.y.floor(),
                            c
                        )))
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
