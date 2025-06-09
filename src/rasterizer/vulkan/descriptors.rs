use ash::{Device, vk};
use std::rc::Rc;

const MAX_SETS_PER_POOL: u32 = 4092;

// #[derive(Default, Debug, Clone, Copy)]
// struct PoolSizeRatio {
//     desc_type: vk::DescriptorType,
//     ratio: f32,
// }

type PoolSizeRatio = (vk::DescriptorType, f32);

pub struct DescriptorAllocator {
    device_copy: Rc<Device>,
    pool: vk::DescriptorPool,
}

impl DescriptorAllocator {
    pub fn new_global(device: Rc<Device>) -> Self {
        let sizes = [(vk::DescriptorType::STORAGE_IMAGE, 1.)];

        Self::new(device, 10, &sizes[..])
    }

    fn new(device: Rc<Device>, max_sets: u32, pool_ratios: &[PoolSizeRatio]) -> Self {
        let pool_sizes: Vec<vk::DescriptorPoolSize> = pool_ratios
            .iter()
            .map(|(ty, ratio)| vk::DescriptorPoolSize {
                ty: *ty,
                descriptor_count: (ratio * (max_sets as f32)) as u32,
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

    // pub fn clear_descriptors(&self) {
    //     unsafe {
    //         self.device_copy
    //             .reset_descriptor_pool(self.pool, vk::DescriptorPoolResetFlags::empty())
    //             .unwrap()
    //     };
    // }

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

// TODO: not already done by VMA ?
pub struct DescriptorAllocatorGrowable {
    device_copy: Rc<Device>,

    ratios: Vec<PoolSizeRatio>,

    full: Vec<vk::DescriptorPool>,
    ready: Vec<vk::DescriptorPool>,

    sets_per_pool: u32,
}

impl DescriptorAllocatorGrowable {
    pub fn new_global(device: Rc<Device>) -> Self {
        let sizes = [
            (vk::DescriptorType::STORAGE_IMAGE, 3.),
            (vk::DescriptorType::STORAGE_BUFFER, 3.),
            (vk::DescriptorType::UNIFORM_BUFFER, 3.),
            (vk::DescriptorType::COMBINED_IMAGE_SAMPLER, 4.),
        ];

        Self::new(device, 10, &sizes[..])
    }

    fn new(device: Rc<Device>, max_sets: u32, pool_ratios: &[PoolSizeRatio]) -> Self {
        let mut res = Self {
            device_copy: device,
            // TODO: clone
            ratios: pool_ratios.into(),
            full: Default::default(),
            ready: Default::default(),
            sets_per_pool: max_sets,
        };
        let pool = res.get_pool();
        res.ready.push(pool);

        res
    }

    pub fn clear_pools(&mut self) {
        self.ready.append(&mut self.full);
        self.ready.iter().for_each(|d| unsafe {
            self.device_copy
                .reset_descriptor_pool(*d, vk::DescriptorPoolResetFlags::empty())
                .unwrap()
        });
    }

    // TODO: look online, seems weird...
    // why continue creating bigger pools, aren't we creating empty ones anyway ?
    // We are not re-allocating existing.
    pub fn allocate<T: vk::ExtendsDescriptorSetAllocateInfo>(
        &mut self,
        layout: vk::DescriptorSetLayout,
        p_next: &mut T,
    ) -> vk::DescriptorSet {
        let mut pool = self.get_pool();

        let layouts = [layout];
        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .push_next(p_next)
            .descriptor_pool(pool)
            .set_layouts(&layouts);

        let res = unsafe { self.device_copy.allocate_descriptor_sets(&alloc_info) };
        let ds = match res {
            Ok(ds) => ds,
            Err(vk::Result::ERROR_OUT_OF_POOL_MEMORY) | Err(vk::Result::ERROR_FRAGMENTED_POOL) => {
                self.full.push(pool);
                pool = self.get_pool();
                unsafe {
                    self.device_copy
                        .allocate_descriptor_sets(&alloc_info.descriptor_pool(pool))
                }
                .unwrap()
            }
            Err(e) => panic!("Error allocating descriptor set : {e}"),
        };

        self.ready.push(pool);

        ds[0]
    }

    fn get_pool(&mut self) -> vk::DescriptorPool {
        self.ready.pop().unwrap_or_else(|| {
            let new = self.create_pool(self.sets_per_pool);

            self.sets_per_pool = (self.sets_per_pool * 3) / 2;
            if self.sets_per_pool > MAX_SETS_PER_POOL {
                self.sets_per_pool = MAX_SETS_PER_POOL;
            }

            new
        })
    }

    fn create_pool(&self, set_count: u32) -> vk::DescriptorPool {
        let pool_sizes: Vec<vk::DescriptorPoolSize> = self
            .ratios
            .iter()
            .map(|(ty, ratio)| vk::DescriptorPoolSize {
                ty: *ty,
                descriptor_count: (ratio * (set_count as f32)) as u32,
            })
            .collect();

        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .max_sets(set_count)
            .pool_sizes(&pool_sizes[..]);

        unsafe {
            self.device_copy
                .create_descriptor_pool(&pool_info, None)
                .unwrap()
        }
    }
}

impl Drop for DescriptorAllocatorGrowable {
    fn drop(&mut self) {
        println!(
            "drop DescriptorAllocatorGrowable : {} full, {} ready descriptor pools",
            self.full.len(),
            self.ready.len()
        );
        unsafe {
            self.full
                .drain(..)
                .chain(self.ready.drain(..))
                .for_each(|d| self.device_copy.destroy_descriptor_pool(d, None));
        }
    }
}

#[derive(Default, Debug, Clone)]
struct AppendOnlyVec<T>(Vec<T>);

impl<T> AppendOnlyVec<T> {
    pub fn push_and_ref(&mut self, value: T) -> &T {
        self.0.push(value);
        self.0.last().unwrap()
    }
}

#[derive(Default, Debug, Clone)]
pub struct DescriptorWriter<'a> {
    image_infos: AppendOnlyVec<vk::DescriptorImageInfo>,
    buffer_infos: AppendOnlyVec<vk::DescriptorBufferInfo>,
    /// **Warning** : This references [`image_infos`] and [`buffer_infos`], please don't try to
    /// clear their items before clearing writes !
    writes: Vec<vk::WriteDescriptorSet<'a>>,
}

impl<'a> DescriptorWriter<'a> {
    // This should be fine as long as we don't remove the item from {image,buffer}_infos.
    //
    // This is needed because [`writes`] refers to [`image_infos`] and [`buffer_infos`],
    // but it locks the whole object as it cannot ensure you won't remove the referenced item
    // from the vecs.
    fn unsafe_ref_to_slice_cut_lifetime<'b, 'c, T>(ref_value: &'b T) -> &'c [T] {
        unsafe { std::slice::from_raw_parts(ref_value, 1) }
    }

    // TODO: not all parameters needed, enum with values instead ?
    pub fn write_image(
        &mut self,
        binding: u32,
        image_view: vk::ImageView,
        sampler: vk::Sampler,
        image_layout: vk::ImageLayout,
        desc_type: vk::DescriptorType,
    ) {
        let info = vk::DescriptorImageInfo::default()
            .sampler(sampler)
            .image_view(image_view)
            .image_layout(image_layout);

        let info_ref = self.image_infos.push_and_ref(info);
        let info_slice = Self::unsafe_ref_to_slice_cut_lifetime(info_ref);

        let write = vk::WriteDescriptorSet::default()
            .dst_binding(binding)
            .dst_set(vk::DescriptorSet::null())
            .descriptor_count(1)
            .descriptor_type(desc_type)
            .image_info(info_slice);

        self.writes.push(write);
    }

    fn write_buffer(
        &mut self,
        binding: u32,
        buffer: vk::Buffer,
        size: u64,
        offset: u64,
        desc_type: vk::DescriptorType,
    ) {
        let info = vk::DescriptorBufferInfo::default()
            .buffer(buffer)
            .offset(offset)
            .range(size);

        let info_ref = self.buffer_infos.push_and_ref(info);
        let info_slice = Self::unsafe_ref_to_slice_cut_lifetime(info_ref);

        let write = vk::WriteDescriptorSet::default()
            .dst_binding(binding)
            .dst_set(vk::DescriptorSet::null())
            .descriptor_count(1)
            .descriptor_type(desc_type)
            .buffer_info(info_slice);

        self.writes.push(write);
    }

    pub fn update_set(&mut self, device: &Device, set: vk::DescriptorSet) {
        self.writes.iter_mut().for_each(|w| w.dst_set = set);

        unsafe {
            device.update_descriptor_sets(&self.writes[..], &[]);
        }
    }
}
