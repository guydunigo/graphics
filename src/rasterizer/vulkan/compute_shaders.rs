use ash::{Device, vk};
use std::rc::Rc;

use crate::rasterizer::vulkan::descriptors::DescriptorLayoutBuilder;

use super::{
    descriptors::{DescriptorAllocator, DescriptorWriter},
    shaders_loader::{ShaderName, ShadersLoader},
};

/// This struct manages the background effects based on compute shaders.
pub struct Effects {
    device_copy: Rc<Device>,

    _descriptor: DescriptorAllocator,
    pub draw_img_descs: vk::DescriptorSet,
    draw_img_desc_layout: vk::DescriptorSetLayout,

    pub pipeline_layout: vk::PipelineLayout,

    pub bg_effects: Vec<ComputeEffect>,
}

impl Effects {
    pub fn new(device: Rc<Device>, shaders: &ShadersLoader, draw_img: vk::ImageView) -> Self {
        let sizes = [(vk::DescriptorType::STORAGE_IMAGE, 1.)];
        let descriptor = DescriptorAllocator::new(device.clone(), 10, &sizes[..]);
        let draw_img_desc_layout = DescriptorLayoutBuilder::default()
            .add_binding(0, vk::DescriptorType::STORAGE_IMAGE)
            .build(&device, vk::ShaderStageFlags::COMPUTE);
        let draw_img_descs = descriptor.allocate(draw_img_desc_layout);

        let mut writer = DescriptorWriter::default();
        writer.write_image(
            0,
            draw_img,
            vk::Sampler::null(),
            vk::ImageLayout::GENERAL,
            vk::DescriptorType::STORAGE_IMAGE,
        );
        writer.update_set(&device, draw_img_descs);

        let push_constants = [vk::PushConstantRange::default()
            .size(size_of::<ComputePushConstants>() as u32)
            .stage_flags(vk::ShaderStageFlags::COMPUTE)];
        let draw_img_desc_layouts = [draw_img_desc_layout];

        let pipeline_layout = {
            let create_info = vk::PipelineLayoutCreateInfo::default()
                .set_layouts(&draw_img_desc_layouts[..])
                .push_constant_ranges(&push_constants[..]);
            unsafe { device.create_pipeline_layout(&create_info, None).unwrap() }
        };

        let gradient = ComputeEffect::gradient(device.clone(), shaders, pipeline_layout);
        let sky = ComputeEffect::sky(device.clone(), shaders, pipeline_layout);

        Self {
            device_copy: device,
            _descriptor: descriptor,
            draw_img_descs,
            draw_img_desc_layout,
            pipeline_layout,
            bg_effects: vec![gradient, sky],
        }
    }
}

impl Drop for Effects {
    fn drop(&mut self) {
        #[cfg(feature = "dbg_mem")]
        println!("drop Effects");
        unsafe {
            self.device_copy
                .destroy_descriptor_set_layout(self.draw_img_desc_layout, None);
            self.device_copy
                .destroy_pipeline_layout(self.pipeline_layout, None);
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
#[repr(C)]
pub struct ComputePushConstants {
    pub data0: [f32; 4],
    pub data1: [f32; 4],
    pub data2: [f32; 4],
    pub data3: [f32; 4],
}

/// Loaded compute shader pipeline.
pub struct ComputeEffect {
    device_copy: Rc<Device>,

    pub name: ShaderName,
    pub pipeline: vk::Pipeline,

    default_data: ComputePushConstants,
}

impl ComputeEffect {
    fn new(
        device: Rc<Device>,
        shaders: &ShadersLoader,
        pipeline_layout: vk::PipelineLayout,
        name: ShaderName,
        default_data: ComputePushConstants,
    ) -> Self {
        let shader = shaders.get(name);
        let pipeline = {
            let stage_info = vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::COMPUTE)
                .module(shader.module_copy())
                .name(c"main");
            let compute_pipeline_create_infos = [vk::ComputePipelineCreateInfo::default()
                .layout(pipeline_layout)
                .stage(stage_info)];
            unsafe {
                device
                    .create_compute_pipelines(
                        vk::PipelineCache::null(),
                        &compute_pipeline_create_infos[..],
                        None,
                    )
                    .unwrap()[0]
            }
        };

        ComputeEffect {
            device_copy: device,
            name,
            pipeline,
            default_data,
        }
    }

    pub fn gradient(
        device: Rc<Device>,
        shaders: &ShadersLoader,
        pipeline_layout: vk::PipelineLayout,
    ) -> Self {
        Self::new(
            device,
            shaders,
            pipeline_layout,
            ShaderName::ParametrableGradient,
            ComputePushConstants {
                data0: [1., 0., 0., 1.],
                data1: [0., 0., 1., 1.],
                ..Default::default()
            },
        )
    }

    pub fn sky(
        device: Rc<Device>,
        shaders: &ShadersLoader,
        pipeline_layout: vk::PipelineLayout,
    ) -> Self {
        Self::new(
            device,
            shaders,
            pipeline_layout,
            ShaderName::Sky,
            ComputePushConstants {
                data0: [0.1, 0.2, 0.4, 0.97],
                ..Default::default()
            },
        )
    }

    pub fn default_data(&self) -> &ComputePushConstants {
        &self.default_data
    }
}

impl Drop for ComputeEffect {
    fn drop(&mut self) {
        #[cfg(feature = "dbg_mem")]
        println!("drop ComputeEffect");
        unsafe {
            self.device_copy.destroy_pipeline(self.pipeline, None);
        }
    }
}
