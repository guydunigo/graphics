use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

use ash::vk;
use egui::{FontData, FontDefinitions, FontFamily, TextureId, TexturesDelta, epaint::ClippedShape};
use egui_ash_renderer::{DynamicRendering, Options, Renderer};
use winit::{event::WindowEvent, window::Window};

use crate::font::{FONT, FONT_NAME};

use super::{vulkan_base::VulkanBase, vulkan_commands::FRAME_OVERLAP};

pub struct GeneratedUi(Vec<ClippedShape>, TexturesDelta, f32);

pub struct VulkanGui {
    inner: RefCell<VulkanGuiMutable>,
}

struct VulkanGuiMutable {
    window: Rc<Window>,
    state: egui_winit::State,
    info: egui::ViewportInfo,
    renderer: Renderer,
    textures_to_free: Option<Vec<TextureId>>,
}

impl VulkanGui {
    pub fn new(
        base: &VulkanBase,
        allocator: Arc<Mutex<vk_mem::Allocator>>,
        format: vk::Format,
    ) -> Self {
        Self {
            inner: RefCell::new(VulkanGuiMutable::new(base, allocator, format)),
        }
    }

    pub fn generate(&mut self, ui: impl FnMut(&egui::Context)) -> GeneratedUi {
        let mut inner = self.inner.borrow_mut();
        inner.generate(ui)
    }

    pub fn draw(
        &self,
        queue: vk::Queue,
        extent: vk::Extent2D,
        cmd_pool: vk::CommandPool,
        cmd_buf: vk::CommandBuffer,
        generated_ui: GeneratedUi,
    ) {
        let mut inner = self.inner.borrow_mut();
        inner.draw(queue, extent, cmd_pool, cmd_buf, generated_ui);
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) {
        self.inner.get_mut().on_window_event(event);
    }

    pub fn on_mouse_motion(&mut self, delta: (f64, f64)) {
        let inner = self.inner.get_mut();
        inner.on_mouse_motion(delta);
    }
}

impl VulkanGuiMutable {
    pub fn new(
        base: &VulkanBase,
        allocator: Arc<std::sync::Mutex<vk_mem::Allocator>>,
        format: vk::Format,
    ) -> Self {
        let dyn_render = DynamicRendering {
            color_attachment_format: format,
            depth_attachment_format: None,
        };

        let renderer = Renderer::with_vk_mem_allocator(
            allocator,
            base.device.as_ref().clone(),
            dyn_render,
            Options {
                srgb_framebuffer: true,
                in_flight_frames: FRAME_OVERLAP,
                ..Default::default()
            },
        )
        .unwrap();

        // 2: initialize egui library
        // this initializes the core structures of egui

        let ctx = egui::Context::default();
        load_fonts(&ctx);
        let info = egui::ViewportInfo::default();
        let viewport_id = ctx.viewport_id();
        let state = egui_winit::State::new(
            ctx,
            viewport_id,
            &base.window,
            Some(base.window.scale_factor() as f32),
            None,
            None,
        );

        Self {
            window: base.window.clone(),
            state,
            info,
            renderer,
            textures_to_free: None,
        }
    }

    pub fn generate(&mut self, ui: impl FnMut(&egui::Context)) -> GeneratedUi {
        egui_winit::update_viewport_info(
            &mut self.info,
            self.state.egui_ctx(),
            &self.window,
            false,
        );

        // Free last frames textures after the previous frame is done rendering
        if let Some(textures) = self.textures_to_free.take() {
            self.renderer
                .free_textures(&textures)
                .expect("Failed to free textures");
        }

        let raw_input = self.state.take_egui_input(&self.window);
        // Already filled, but in the docs it says it doesn't...
        // self.state
        //     .egui_ctx()
        //     .input(|i| raw_input.viewports = i.raw.viewports.clone());

        let egui::FullOutput {
            shapes,
            textures_delta,
            platform_output,
            pixels_per_point,
            ..
        } = self.state.egui_ctx().run(raw_input, ui);

        self.state
            .handle_platform_output(&self.window, platform_output);

        GeneratedUi(shapes, textures_delta, pixels_per_point)
    }

    pub fn draw(
        &mut self,
        queue: vk::Queue,
        extent: vk::Extent2D,
        cmd_pool: vk::CommandPool,
        cmd_buf: vk::CommandBuffer,
        GeneratedUi(shapes, textures_delta, pixels_per_point): GeneratedUi,
    ) {
        if !textures_delta.free.is_empty() {
            self.textures_to_free = Some(textures_delta.free.clone());
        }

        if !textures_delta.set.is_empty() {
            self.renderer
                .set_textures(queue, cmd_pool, textures_delta.set.as_slice())
                .expect("Failed to update texture");
        }

        let clipped_primitives = self.state.egui_ctx().tessellate(shapes, pixels_per_point);

        self.renderer
            .cmd_draw(cmd_buf, extent, pixels_per_point, &clipped_primitives[..])
            .unwrap();
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) {
        // TODO: result ?
        let _ = self.state.on_window_event(&self.window, event);
    }

    pub fn on_mouse_motion(&mut self, delta: (f64, f64)) {
        self.state.on_mouse_motion(delta);
    }
}

impl Drop for VulkanGuiMutable {
    fn drop(&mut self) {
        println!("drop VulkanGui");
        // unsafe {
        // }
    }
}

fn load_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert(FONT_NAME.into(), Arc::new(FontData::from_static(FONT)));
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, FONT_NAME.into());
    fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .unwrap()
        .insert(0, FONT_NAME.into());
    ctx.set_fonts(fonts);
}
