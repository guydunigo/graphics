use ash::{Device, vk};

use super::vulkan_base::VulkanBase;

const FRAME_OVERLAP: usize = 2;

#[derive(Debug)]
pub struct FrameData {
    cmd_pool: vk::CommandPool,
    pub cmd_buf: vk::CommandBuffer,

    pub fence_render: vk::Fence,
    pub sem_swapchain: vk::Semaphore,
    sem_render: vk::Semaphore,
}

pub struct VulkanCommands {
    device_copy: Device,

    queue: vk::Queue,
    frames: [FrameData; FRAME_OVERLAP],
    frame_number: usize,
}

impl VulkanCommands {
    pub fn new(base: &VulkanBase) -> Self {
        let queue = unsafe { base.device.get_device_queue(base.queue_family_index, 0) };

        let pool_create_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(base.queue_family_index);
        let fence_create_info =
            vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
        let sem_create_info = vk::SemaphoreCreateInfo::default();

        let frames: Vec<FrameData> = (0..FRAME_OVERLAP)
            .map(|_| {
                let cmd_pool = unsafe {
                    base.device
                        .create_command_pool(&pool_create_info, None)
                        .unwrap()
                };

                let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
                    .command_buffer_count(1)
                    .command_pool(cmd_pool)
                    .level(vk::CommandBufferLevel::PRIMARY);

                // TODO: always take index 0 ?
                let cmd_buf = unsafe {
                    base.device
                        .allocate_command_buffers(&command_buffer_allocate_info)
                        .unwrap()
                }[0];

                let fence_render =
                    unsafe { base.device.create_fence(&fence_create_info, None).unwrap() };
                let sem_swapchain = unsafe {
                    base.device
                        .create_semaphore(&sem_create_info, None)
                        .unwrap()
                };
                let sem_render = unsafe {
                    base.device
                        .create_semaphore(&sem_create_info, None)
                        .unwrap()
                };

                FrameData {
                    cmd_pool,
                    cmd_buf,
                    fence_render,
                    sem_swapchain,
                    sem_render,
                }
            })
            .collect();
        let frames: [FrameData; FRAME_OVERLAP] = frames.try_into().unwrap();

        Self {
            // I hope it's okay to clone the device...
            // It's needed for Drop, but I'd like to keep this object separated from `VulkanBase`.
            device_copy: base.device.clone(),

            queue,
            frames,
            frame_number: 0,
        }
    }

    // TODO: to infinite iter ? .cycle() + self.frames.next().unwrap()
    pub fn current_frame(&self) -> &FrameData {
        &self.frames[self.frame_number % FRAME_OVERLAP]
    }
}

impl Drop for VulkanCommands {
    fn drop(&mut self) {
        println!("drop VulkanCommands");
        unsafe {
            self.device_copy.device_wait_idle().unwrap();
            self.frames.iter().for_each(|f| {
                self.device_copy.destroy_command_pool(f.cmd_pool, None);
                self.device_copy.destroy_fence(f.fence_render, None);
                self.device_copy.destroy_semaphore(f.sem_swapchain, None);
                self.device_copy.destroy_semaphore(f.sem_render, None);
            });
        }
    }
}
