use ash::{Device, vk};
use std::{
    ffi::c_void,
    rc::Rc,
    slice,
    sync::{Arc, Mutex},
};
use vk_mem::Alloc;
use winit::dpi::PhysicalSize;

use super::commands::{VulkanCommands, transition_image};

pub struct AllocatedBuffer {
    allocator_copy: Arc<Mutex<vk_mem::Allocator>>,
    pub buffer: vk::Buffer,
    allocation: vk_mem::Allocation,
    info: vk_mem::AllocationInfo,
}

pub enum MyMemoryUsage {
    GpuOnly,
    StagingUpload,
    CpuToGpu,
}

impl AllocatedBuffer {
    pub fn new(
        allocator: Arc<Mutex<vk_mem::Allocator>>,
        alloc_size: u64,
        usage: vk::BufferUsageFlags,
        memory_usage: MyMemoryUsage,
    ) -> Self {
        let buffer_info = vk::BufferCreateInfo::default()
            .size(alloc_size)
            .usage(usage);

        let mut alloc_info = vk_mem::AllocationCreateInfo {
            usage: vk_mem::MemoryUsage::Auto,
            ..Default::default()
        };

        match memory_usage {
            MyMemoryUsage::GpuOnly => {
                alloc_info.required_flags = vk::MemoryPropertyFlags::DEVICE_LOCAL;
                // TODO: or usage : AutoPreferDevice ?
                // TODO: Consider using vk_mem::AllocationCreateFlags::DEDICATED_MEMORY,
                // especially if large
            }
            MyMemoryUsage::StagingUpload | MyMemoryUsage::CpuToGpu => {
                // When using MemoryUsage::Auto + MAPPED, needs one of :
                // #VMA_ALLOCATION_CREATE_HOST_ACCESS_SEQUENTIAL_WRITE_BIT
                // or #VMA_ALLOCATION_CREATE_HOST_ACCESS_RANDOM_BIT
                // requires memcpy and no random access (no mapped_data[i] = ...) !
                alloc_info.flags = vk_mem::AllocationCreateFlags::MAPPED
                    | vk_mem::AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE;
            }
        }

        let (buffer, allocation, info) = {
            let allocator = allocator.lock().unwrap();
            unsafe {
                let (buffer, allocation) =
                    allocator.create_buffer(&buffer_info, &alloc_info).unwrap();
                let info = allocator.get_allocation_info(&allocation);
                #[cfg(feature = "dbg_mem")]
                println!("{info:?}");
                (buffer, allocation, info)
            }
        };

        Self {
            allocator_copy: allocator,
            buffer,
            allocation,
            info,
        }
    }

    pub fn mapped_data(&self) -> *mut c_void {
        self.info.mapped_data
    }
}

impl Drop for AllocatedBuffer {
    fn drop(&mut self) {
        #[cfg(feature = "dbg_mem")]
        println!("drop AllocatedBuffer");
        unsafe {
            self.allocator_copy
                .lock()
                .unwrap()
                .destroy_buffer(self.buffer, &mut self.allocation);
        }
    }
}

pub struct AllocatedImage {
    device_copy: Rc<Device>,
    allocator_copy: Arc<Mutex<vk_mem::Allocator>>,

    pub img: vk::Image,
    pub img_view: vk::ImageView,
    allocation: vk_mem::Allocation,
    pub extent: vk::Extent3D,
    pub format: vk::Format,
}

impl AllocatedImage {
    pub fn new_draw_img(
        device: Rc<Device>,
        allocator: Arc<Mutex<vk_mem::Allocator>>,
        window_size: PhysicalSize<u32>,
    ) -> Self {
        let usages = vk::ImageUsageFlags::TRANSFER_SRC
            | vk::ImageUsageFlags::TRANSFER_DST
            | vk::ImageUsageFlags::STORAGE
            | vk::ImageUsageFlags::COLOR_ATTACHMENT;

        Self::new_img_with_window_size(
            device,
            allocator,
            window_size,
            vk::Format::R16G16B16A16_SFLOAT,
            usages,
        )
    }

    pub fn new_draw_depth(
        device: Rc<Device>,
        allocator: Arc<Mutex<vk_mem::Allocator>>,
        window_size: PhysicalSize<u32>,
    ) -> Self {
        Self::new_img_with_window_size(
            device,
            allocator,
            window_size,
            vk::Format::D32_SFLOAT,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
        )
    }

    fn new_img_with_window_size(
        device: Rc<Device>,
        allocator: Arc<Mutex<vk_mem::Allocator>>,
        window_size: PhysicalSize<u32>,
        format: vk::Format,
        usages: vk::ImageUsageFlags,
    ) -> Self {
        let extent = vk::Extent3D {
            width: window_size.width,
            height: window_size.height,
            depth: 1,
        };
        Self::new(device, allocator, extent, format, usages, false)
    }

    fn new(
        device: Rc<Device>,
        allocator: Arc<Mutex<vk_mem::Allocator>>,
        extent: vk::Extent3D,
        format: vk::Format,
        usages: vk::ImageUsageFlags,
        mipmapped: bool,
    ) -> Self {
        let aspect = if format == vk::Format::D32_SFLOAT {
            vk::ImageAspectFlags::DEPTH
        } else {
            vk::ImageAspectFlags::COLOR
        };

        Self::new_with_aspect(
            device.clone(),
            allocator,
            extent,
            format,
            usages,
            aspect,
            mipmapped,
        )
    }

    fn new_with_aspect(
        device: Rc<Device>,
        allocator: Arc<Mutex<vk_mem::Allocator>>,
        extent: vk::Extent3D,
        format: vk::Format,
        usages: vk::ImageUsageFlags,
        aspect: vk::ImageAspectFlags,
        mipmapped: bool,
    ) -> Self {
        let rimg_info = image_create_info(format, usages, extent, mipmapped);
        let mut rimg_allocinfo = vk_mem::AllocationCreateInfo::default();
        {
            // Example : https://gpuopen-librariesandsdks.github.io/VulkanMemoryAllocator/html/usage_patterns.html
            // Prefered to GpuOnly (deprecated)
            rimg_allocinfo.usage = vk_mem::MemoryUsage::Auto;
            rimg_allocinfo.flags = vk_mem::AllocationCreateFlags::DEDICATED_MEMORY;
            rimg_allocinfo.priority = 1.;
            rimg_allocinfo.required_flags = vk::MemoryPropertyFlags::DEVICE_LOCAL;
        }

        let (img, allocation) = unsafe {
            allocator
                .lock()
                .unwrap()
                .create_image(&rimg_info, &rimg_allocinfo)
                .unwrap()
        };

        let view_create_info =
            image_view_create_info(format, img, aspect, Some(rimg_info.mip_levels));
        let img_view = unsafe { device.create_image_view(&view_create_info, None).unwrap() };

        Self {
            device_copy: device,
            allocator_copy: allocator,

            img,
            img_view,
            allocation,
            extent,
            format,
        }
    }

    /// Allocates an image and uploads data directly to it.
    ///
    /// The image ends in vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
    ///
    /// Example usecase : textures.
    /// `data` must be at least `extent.depth * extent.width * extent.height * size_of::<u32>()`
    pub fn new_and_upload(
        commands: &VulkanCommands,
        device: Rc<Device>,
        allocator: Arc<Mutex<vk_mem::Allocator>>,
        extent: vk::Extent3D,
        format: vk::Format,
        usages: vk::ImageUsageFlags,
        mipmapped: bool,
        data: &[u8],
    ) -> Self {
        let pixel = match format {
            vk::Format::R8G8B8A8_UNORM => size_of::<u8>() * 4,
            // vk::Format::R8_UNORM => size_of::<u8>() * 1,
            // vk::Format::R8G8_UNORM => size_of::<u8>() * 2,
            // vk::Format::R8G8B8_UNORM => size_of::<u8>() * 3,
            // vk::Format::R8G8B8A8_UNORM => size_of::<u8>() * 4,
            // vk::Format::R16_UNORM => size_of::<u16>() * 1,
            // vk::Format::R16G16_UNORM => size_of::<u16>() * 2,
            // vk::Format::R16G16B16_UNORM => size_of::<u16>() * 3,
            // vk::Format::R16G16B16A16_UNORM => size_of::<u16>() * 4,
            // vk::Format::R32G32B32_SFLOAT => size_of::<f32>() * 3,
            // vk::Format::R32G32B32A32_SFLOAT => size_of::<f32>() * 4,
            _ => unimplemented!("Unsupported image format : {format:?} !"),
        };

        let data_size = (extent.depth * extent.width * extent.height * pixel as u32) as usize;
        assert!(data_size <= data.len());

        let buffer = AllocatedBuffer::new(
            allocator.clone(),
            data_size as u64,
            vk::BufferUsageFlags::TRANSFER_SRC,
            MyMemoryUsage::CpuToGpu,
        );

        let data_buffer =
            unsafe { slice::from_raw_parts_mut(buffer.mapped_data() as *mut u8, data_size) };

        data_buffer.copy_from_slice(&data[0..data_size]);

        let new_image = Self::new(
            device,
            allocator,
            extent,
            format,
            usages | vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::TRANSFER_SRC,
            mipmapped,
        );

        commands.immediate_submit(|device, cmd_buf| {
            transition_image(
                device,
                cmd_buf,
                new_image.img,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            );

            let copy_region = vk::BufferImageCopy::default()
                .buffer_offset(0)
                .buffer_row_length(0)
                .buffer_image_height(0)
                .image_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .image_extent(extent);
            let copy_regions = [copy_region];

            unsafe {
                device.cmd_copy_buffer_to_image(
                    cmd_buf,
                    buffer.buffer,
                    new_image.img,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &copy_regions[..],
                );
            }

            transition_image(
                device,
                cmd_buf,
                new_image.img,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );
        });

        new_image
    }
}

impl Drop for AllocatedImage {
    fn drop(&mut self) {
        #[cfg(feature = "dbg_mem")]
        println!("drop AllocatedImage");
        unsafe {
            self.device_copy.device_wait_idle().unwrap();
            self.device_copy.destroy_image_view(self.img_view, None);
            self.allocator_copy
                .lock()
                .unwrap()
                .destroy_image(self.img, &mut self.allocation);
        }
    }
}

fn image_create_info<'a>(
    format: vk::Format,
    usage_flags: vk::ImageUsageFlags,
    extent: vk::Extent3D,
    mipmapped: bool,
) -> vk::ImageCreateInfo<'a> {
    let mip_levels = if mipmapped {
        f32::floor(f32::log2(u32::max(extent.width, extent.height) as f32)) as u32 + 1
    } else {
        1
    };
    vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(format)
        .extent(extent)
        .mip_levels(mip_levels)
        .array_layers(1)
        // for MSAA. we will not be using it by default, so default it to 1 sample per pixel.
        .samples(vk::SampleCountFlags::TYPE_1)
        // optimal tiling, which means the image is stored on the best gpu format
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(usage_flags)
}

pub fn image_view_create_info<'a>(
    format: vk::Format,
    image: vk::Image,
    aspect_flags: vk::ImageAspectFlags,
    level_count: Option<u32>,
) -> vk::ImageViewCreateInfo<'a> {
    vk::ImageViewCreateInfo::default()
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(format)
        // .components(vk::ComponentMapping {
        //     r: vk::ComponentSwizzle::R,
        //     g: vk::ComponentSwizzle::G,
        //     b: vk::ComponentSwizzle::B,
        //     a: vk::ComponentSwizzle::A,
        // })
        .subresource_range(
            vk::ImageSubresourceRange::default()
                .level_count(level_count.unwrap_or(1))
                .layer_count(1)
                .base_mip_level(0)
                .base_array_layer(0)
                .aspect_mask(aspect_flags),
        )
        .image(image)
}
