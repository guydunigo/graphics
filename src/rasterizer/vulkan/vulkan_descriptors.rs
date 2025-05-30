use ash::{Device, vk};
use std::rc::Rc;

use crate::rasterizer::vulkan::vulkan_shaders::ShaderName;

use super::vulkan_shaders::VulkanShaders;

pub struct VulkanDescriptors {
    device_copy: Rc<Device>,

    descriptor: DescriptorAllocator,
    pub draw_img_descs: vk::DescriptorSet,
    draw_img_desc_layout: vk::DescriptorSetLayout,

    pub gradient_pipeline: vk::Pipeline,
    pub gradient_pipeline_layout: vk::PipelineLayout,
}

impl VulkanDescriptors {
    pub fn new(device: Rc<Device>, shaders: &VulkanShaders, draw_img: vk::ImageView) -> Self {
        let descriptor = DescriptorAllocator::new_global(device.clone());
        let draw_img_desc_layout = DescriptorLayoutBuilder::default()
            .add_binding(0, vk::DescriptorType::STORAGE_IMAGE)
            .build(&device, vk::ShaderStageFlags::COMPUTE);
        let draw_img_descs = descriptor.allocate(draw_img_desc_layout);

        let img_infos = [vk::DescriptorImageInfo::default()
            .image_layout(vk::ImageLayout::GENERAL)
            .image_view(draw_img)];

        let draw_img_writes = [vk::WriteDescriptorSet::default()
            .dst_binding(0)
            .dst_set(draw_img_descs)
            .descriptor_count(1)
            .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
            .image_info(&img_infos[..])];

        unsafe { device.update_descriptor_sets(&draw_img_writes[..], &[]) };

        // TODO: extract ?
        let gradient_pipeline_layout = {
            let draw_img_desc_layouts = [draw_img_desc_layout];
            let create_info =
                vk::PipelineLayoutCreateInfo::default().set_layouts(&draw_img_desc_layouts[..]);
            unsafe { device.create_pipeline_layout(&create_info, None).unwrap() }
        };

        let gradient_pipeline = {
            let stage_info = vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::COMPUTE)
                .module(shaders.get(ShaderName::Gradient))
                .name(c"main");
            let compute_pipeline_create_infos = [vk::ComputePipelineCreateInfo::default()
                .layout(gradient_pipeline_layout)
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

        Self {
            device_copy: device,
            descriptor,
            draw_img_descs,
            draw_img_desc_layout,
            gradient_pipeline,
            gradient_pipeline_layout,
        }
    }
}

impl Drop for VulkanDescriptors {
    fn drop(&mut self) {
        println!("drop VulkanDescriptors");
        unsafe {
            self.device_copy
                .destroy_descriptor_set_layout(self.draw_img_desc_layout, None);
            self.device_copy
                .destroy_pipeline(self.gradient_pipeline, None);
            self.device_copy
                .destroy_pipeline_layout(self.gradient_pipeline_layout, None);
        }
    }
}

#[derive(Default)]
struct DescriptorLayoutBuilder<'a> {
    bindings: Vec<vk::DescriptorSetLayoutBinding<'a>>,
}

impl<'a> DescriptorLayoutBuilder<'a> {
    pub fn add_binding(mut self, binding: u32, desc_type: vk::DescriptorType) -> Self {
        let newbind = vk::DescriptorSetLayoutBinding::default()
            .binding(binding)
            .descriptor_type(desc_type)
            .descriptor_count(1);

        self.bindings.push(newbind);

        self
    }

    pub fn clear(&mut self) {
        self.bindings.clear();
    }

    pub fn build(
        mut self,
        device: &Device,
        shader_stages: vk::ShaderStageFlags,
    ) -> vk::DescriptorSetLayout {
        self.bindings
            .iter_mut()
            .for_each(|b| b.stage_flags |= shader_stages);

        let info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&self.bindings[..]);

        unsafe { device.create_descriptor_set_layout(&info, None).unwrap() }
    }

    fn build_2<T: vk::ExtendsDescriptorSetLayoutCreateInfo>(
        mut self,
        device: &Device,
        shader_stages: vk::ShaderStageFlags,
        p_next: &mut T,
        flags: vk::DescriptorSetLayoutCreateFlags,
    ) -> vk::DescriptorSetLayout {
        self.bindings
            .iter_mut()
            .for_each(|b| b.stage_flags |= shader_stages);

        let info = vk::DescriptorSetLayoutCreateInfo::default()
            .bindings(&self.bindings[..])
            .push_next(p_next)
            .flags(flags);

        unsafe { device.create_descriptor_set_layout(&info, None).unwrap() }
    }
}

struct PoolSizeRatio {
    desc_type: vk::DescriptorType,
    ratio: f32,
}

struct DescriptorAllocator {
    device_copy: Rc<Device>,
    pool: vk::DescriptorPool,
}

impl DescriptorAllocator {
    pub fn new_global(device: Rc<Device>) -> Self {
        let sizes = [PoolSizeRatio {
            desc_type: vk::DescriptorType::STORAGE_IMAGE,
            ratio: 1.,
        }];

        Self::new(device, 10, &sizes[..])
    }

    fn new(device: Rc<Device>, max_sets: u32, pool_ratios: &[PoolSizeRatio]) -> Self {
        let pool_sizes: Vec<vk::DescriptorPoolSize> = pool_ratios
            .iter()
            .map(|r| vk::DescriptorPoolSize {
                ty: r.desc_type,
                descriptor_count: (r.ratio * (max_sets as f32)) as u32,
            })
            .collect();

        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .max_sets(max_sets)
            .pool_sizes(&pool_sizes[..]);

        let pool = unsafe { device.create_descriptor_pool(&pool_info, None).unwrap() };

        Self {
            device_copy: device,
            pool,
        }
    }

    pub fn clear_descriptors(&self) {
        unsafe {
            self.device_copy
                .reset_descriptor_pool(self.pool, vk::DescriptorPoolResetFlags::empty())
                .unwrap()
        };
    }

    pub fn allocate(&self, layout: vk::DescriptorSetLayout) -> vk::DescriptorSet {
        let layouts = [layout];
        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(self.pool)
            .set_layouts(&layouts[..]);

        unsafe {
            self.device_copy
                .allocate_descriptor_sets(&alloc_info)
                // We allocate only one layout, so we keep only the first one :
                .unwrap()[0]
        }
    }
}

impl Drop for DescriptorAllocator {
    fn drop(&mut self) {
        println!("drop DescriptorAllocator");
        unsafe { self.device_copy.destroy_descriptor_pool(self.pool, None) };
    }
}
