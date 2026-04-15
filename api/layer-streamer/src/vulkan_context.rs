use anyhow::{Context, Result};
use ash::vk;
use std::ffi::CString;
use tracing::{debug, info};

/// Vulkan context for GPU compute operations
pub struct VulkanContext {
    pub entry: ash::Entry,
    pub instance: ash::Instance,
    pub physical_device: vk::PhysicalDevice,
    pub device: ash::Device,
    pub queue_family_index: u32,
    pub compute_queue: vk::Queue,
    pub device_properties: vk::PhysicalDeviceProperties,
}

impl VulkanContext {
    pub fn new() -> Result<Self> {
        info!("🔧 Initializing Vulkan context...");

        let entry = unsafe { ash::Entry::load() }
            .context("Failed to load Vulkan entry")?;

        // Create instance
        let instance = Self::create_instance(&entry)?;

        // Select physical device (prefer discrete GPU)
        let physical_devices = unsafe { instance.enumerate_physical_devices() }?;
        let (physical_device, device_properties) = Self::select_device(&instance, &physical_devices)?;

        info!(
            "📺 GPU: {} (Vulkan {}.{})",
            unsafe {
                std::ffi::CStr::from_ptr(device_properties.device_name.as_ptr())
                    .to_string_lossy()
            },
            vk::api_version_major(device_properties.api_version),
            vk::api_version_minor(device_properties.api_version)
        );
        debug!("   Max compute workgroup: {:?}", device_properties.limits.max_compute_work_group_count);

        // Create logical device with compute queue
        let (device, queue_family_index, compute_queue) =
            Self::create_logical_device(&instance, physical_device)?;

        Ok(Self {
            entry,
            instance,
            physical_device,
            device,
            queue_family_index,
            compute_queue,
            device_properties,
        })
    }

    fn create_instance(entry: &ash::Entry) -> Result<ash::Instance> {
        let app_name = CString::new("layer-streamer").unwrap();

        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .application_version(vk::make_api_version(0, 1, 0, 0))
            .engine_name(&app_name)
            .engine_version(vk::make_api_version(0, 1, 0, 0))
            .api_version(vk::API_VERSION_1_2);

        let instance_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info);

        unsafe {
            entry.create_instance(&instance_info, None)
                .context("Failed to create Vulkan instance")
        }
    }

    fn select_device(
        instance: &ash::Instance,
        devices: &[vk::PhysicalDevice],
    ) -> Result<(vk::PhysicalDevice, vk::PhysicalDeviceProperties)> {
        for &device in devices {
            let props = unsafe { instance.get_physical_device_properties(device) };

            // Prefer discrete GPU, then integrated, then anything with compute
            let has_compute = unsafe {
                instance
                    .get_physical_device_queue_family_properties(device)
                    .iter()
                    .any(|qf| qf.queue_flags.contains(vk::QueueFlags::COMPUTE))
            };

            if !has_compute {
                continue;
            }

            // Return first device with compute queue (good enough for now)
            return Ok((device, props));
        }

        anyhow::bail!("No Vulkan physical devices with compute support found")
    }

    fn create_logical_device(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
    ) -> Result<(ash::Device, u32, vk::Queue)> {
        // Find compute queue family
        let queue_family_properties =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

        let queue_family_index = queue_family_properties
            .iter()
            .position(|qf| qf.queue_flags.contains(vk::QueueFlags::COMPUTE))
            .ok_or_else(|| anyhow::anyhow!("No compute queue family found"))? as u32;

        let queue_priority = 1.0f32;
        let queue_priorities = [queue_priority];
        let queue_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index)
            .queue_priorities(&queue_priorities);

        // Enable features we need
        let features = vk::PhysicalDeviceFeatures::default()
            .shader_int64(true)
            .fill_mode_non_solid(true);

        let queue_create_infos = [queue_info];
        let device_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_features(&features);

        let device = unsafe {
            instance
                .create_device(physical_device, &device_info, None)
                .context("Failed to create logical device")?
        };

        let compute_queue = unsafe { device.get_device_queue(queue_family_index, 0) };

        Ok((device, queue_family_index, compute_queue))
    }

    /// Get memory type index with required properties
    pub fn find_memory_type(
        &self,
        type_bits: u32,
        properties: vk::MemoryPropertyFlags,
    ) -> Result<u32> {
        let mem_properties = unsafe {
            self.instance
                .get_physical_device_memory_properties(self.physical_device)
        };

        for i in 0..mem_properties.memory_type_count {
            let type_bits_match = 1 << i;
            if (type_bits & type_bits_match) != 0
                && mem_properties.memory_types[i as usize]
                    .property_flags
                    .contains(properties)
            {
                return Ok(i);
            }
        }

        anyhow::bail!(
            "No memory type with properties {:?} found",
            properties
        )
    }

    /// Create a buffer with data
    pub fn create_buffer_with_data(
        &self,
        data: &[f32],
        usage: vk::BufferUsageFlags,
    ) -> Result<(vk::Buffer, vk::DeviceMemory, usize)> {
        let size = data.len() * std::mem::size_of::<f32>();

        // Create staging buffer (host visible)
        let staging_info = vk::BufferCreateInfo::default()
            .size(size as u64)
            .usage(vk::BufferUsageFlags::TRANSFER_SRC)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let staging_buffer = unsafe { self.device.create_buffer(&staging_info, None) }?;
        let mem_reqs = unsafe { self.device.get_buffer_memory_requirements(staging_buffer) };

        let mem_type = self.find_memory_type(
            mem_reqs.memory_type_bits,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;

        let mem_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_reqs.size)
            .memory_type_index(mem_type);

        let staging_memory = unsafe { self.device.allocate_memory(&mem_info, None) }?;
        unsafe {
            self.device
                .bind_buffer_memory(staging_buffer, staging_memory, 0)
                .map_err(|e| anyhow::anyhow!("Failed to bind staging memory: {:?}", e))?;
        }

        // Map and copy data
        let mapped = unsafe {
            self.device
                .map_memory(staging_memory, 0, mem_reqs.size, vk::MemoryMapFlags::empty())?
        };

        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), mapped as *mut f32, data.len());
            self.device.unmap_memory(staging_memory);
        }

        // Create device-local buffer
        let device_info = vk::BufferCreateInfo::default()
            .size(size as u64)
            .usage(usage | vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::STORAGE_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let device_buffer = unsafe { self.device.create_buffer(&device_info, None) }?;
        let device_mem_reqs = unsafe { self.device.get_buffer_memory_requirements(device_buffer) };

        let device_mem_type = self.find_memory_type(
            device_mem_reqs.memory_type_bits,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;

        let device_mem_info = vk::MemoryAllocateInfo::default()
            .allocation_size(device_mem_reqs.size)
            .memory_type_index(device_mem_type);

        let device_memory = unsafe { self.device.allocate_memory(&device_mem_info, None) }?;
        unsafe {
            self.device
                .bind_buffer_memory(device_buffer, device_memory, 0)
                .map_err(|e| anyhow::anyhow!("Failed to bind buffer memory: {:?}", e))?;
        }

        // Copy staging to device
        self.copy_buffer(staging_buffer, device_buffer, size as u64)?;

        // Cleanup staging
        unsafe {
            self.device.free_memory(staging_memory, None);
            self.device.destroy_buffer(staging_buffer, None);
        }

        Ok((device_buffer, device_memory, data.len()))
    }

    /// Copy data between buffers
    fn copy_buffer(&self, src: vk::Buffer, dst: vk::Buffer, size: u64) -> Result<()> {
        let command_pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(self.queue_family_index)
            .flags(vk::CommandPoolCreateFlags::TRANSIENT);

        let command_pool = unsafe { self.device.create_command_pool(&command_pool_info, None) }?;

        let command_buffer_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let command_buffers = unsafe { self.device.allocate_command_buffers(&command_buffer_info) }?;
        let cmd = command_buffers[0];

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        unsafe {
            self.device.begin_command_buffer(cmd, &begin_info)?;

            let copy_region = vk::BufferCopy::default().size(size);
            self.device.cmd_copy_buffer(cmd, src, dst, &[copy_region]);

            self.device.end_command_buffer(cmd)?;
        }

        let submit_info = vk::SubmitInfo::default();
        let cmd_buffers = [cmd];
        let submit_info = submit_info.command_buffers(&cmd_buffers);
        let fence = unsafe {
            self.device
                .create_fence(&vk::FenceCreateInfo::default(), None)?
        };

        unsafe {
            self.device.queue_submit(self.compute_queue, &[submit_info], fence)?;
            self.device.wait_for_fences(&[fence], true, u64::MAX)?;
            self.device.destroy_fence(fence, None);
            self.device.free_command_buffers(command_pool, &[cmd]);
            self.device.destroy_command_pool(command_pool, None);
        }

        Ok(())
    }

    /// Read data back from a buffer
    pub fn read_buffer(&self, src: vk::Buffer, element_count: usize) -> Result<Vec<f32>> {
        let size = element_count * std::mem::size_of::<f32>();

        // Create staging buffer
        let staging_info = vk::BufferCreateInfo::default()
            .size(size as u64)
            .usage(vk::BufferUsageFlags::TRANSFER_DST)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let staging_buffer = unsafe { self.device.create_buffer(&staging_info, None) }?;
        let mem_reqs = unsafe { self.device.get_buffer_memory_requirements(staging_buffer) };

        let mem_type = self.find_memory_type(
            mem_reqs.memory_type_bits,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;

        let mem_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_reqs.size)
            .memory_type_index(mem_type);

        let staging_memory = unsafe { self.device.allocate_memory(&mem_info, None) }?;
        unsafe {
            self.device
                .bind_buffer_memory(staging_buffer, staging_memory, 0)
                .map_err(|e| anyhow::anyhow!("Failed to bind readback staging memory: {:?}", e))?;
        }

        // Copy device -> staging
        self.copy_buffer(src, staging_buffer, size as u64)?;

        // Map and read
        let mapped = unsafe {
            self.device
                .map_memory(staging_memory, 0, mem_reqs.size, vk::MemoryMapFlags::empty())?
        };

        let mut result = vec![0.0f32; element_count];
        unsafe {
            std::ptr::copy_nonoverlapping(mapped as *const f32, result.as_mut_ptr(), element_count);
            self.device.unmap_memory(staging_memory);
            self.device.free_memory(staging_memory, None);
            self.device.destroy_buffer(staging_buffer, None);
        }

        Ok(result)
    }

    pub fn device(&self) -> &ash::Device {
        &self.device
    }

    pub fn compute_queue(&self) -> vk::Queue {
        self.compute_queue
    }

    pub fn queue_family_index(&self) -> u32 {
        self.queue_family_index
    }
}

impl Drop for VulkanContext {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}
