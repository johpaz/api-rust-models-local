use ash::vk;
use anyhow::Result;
use std::collections::HashMap;
use tracing::debug;
use crate::vulkan_context::VulkanContext;

/// Compiled GPU pipeline for a specific compute operation
pub struct GpuPipeline {
    pub shader_module: vk::ShaderModule,
    pub pipeline_layout: vk::PipelineLayout,
    pub pipeline: vk::Pipeline,
    pub descriptor_set_layout: vk::DescriptorSetLayout,
    pub descriptor_pool: vk::DescriptorPool,
    pub descriptor_sets: Vec<vk::DescriptorSet>,
    pub command_pool: vk::CommandPool,
    pub command_buffers: Vec<vk::CommandBuffer>,
}

impl GpuPipeline {
    pub fn new(
        ctx: &VulkanContext,
        shader_name: &str,
        num_descriptor_sets: usize,
        buffer_bindings: &[vk::DescriptorSetLayoutBinding],
    ) -> Result<Self> {
        debug!("Creating GPU pipeline for shader: {}", shader_name);

        // Load SPIR-V shader from embedded bytes
        let spirv_code = Self::load_spirv(shader_name);
        let shader_module = Self::create_shader_module(ctx, &spirv_code)?;

        // Create descriptor set layout
        let set_layout_info = vk::DescriptorSetLayoutCreateInfo::default()
            .bindings(buffer_bindings);
        let descriptor_set_layout = unsafe {
            ctx.device()
                .create_descriptor_set_layout(&set_layout_info, None)
                .map_err(|e| anyhow::anyhow!("Failed to create descriptor set layout: {:?}", e))?
        };

        // Create pipeline layout with push constants
        let push_constant_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::COMPUTE)
            .offset(0)
            .size(16); // 4 x u32/f32 = 16 bytes

        let set_layouts = [descriptor_set_layout];
        let push_constant_ranges = [push_constant_range];
        let layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&set_layouts)
            .push_constant_ranges(&push_constant_ranges);

        let pipeline_layout = unsafe {
            ctx.device().create_pipeline_layout(&layout_info, None)
                .map_err(|e| anyhow::anyhow!("Failed to create pipeline layout: {:?}", e))?
        };

        // Create compute pipeline
        let stage_info = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(shader_module)
            .name(std::ffi::CStr::from_bytes_with_nul(b"main\0").unwrap());

        let pipeline_info = vk::ComputePipelineCreateInfo::default()
            .stage(stage_info)
            .layout(pipeline_layout);

        let pipelines = unsafe {
            ctx.device()
                .create_compute_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
                .map_err(|e| anyhow::anyhow!("Failed to create compute pipeline: {:?}", e))?
        };
        let pipeline = pipelines[0];

        // Create descriptor pool
        let pool_sizes: Vec<vk::DescriptorPoolSize> = buffer_bindings
            .iter()
            .map(|b| vk::DescriptorPoolSize::default()
                .ty(b.descriptor_type)
                .descriptor_count(num_descriptor_sets as u32))
            .collect();

        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&pool_sizes)
            .max_sets(num_descriptor_sets as u32);

        let descriptor_pool = unsafe {
            ctx.device().create_descriptor_pool(&pool_info, None)
                .map_err(|e| anyhow::anyhow!("Failed to create descriptor pool: {:?}", e))?
        };

        // Allocate descriptor sets
        let set_layouts: Vec<vk::DescriptorSetLayout> =
            vec![descriptor_set_layout; num_descriptor_sets];
        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&set_layouts);

        let descriptor_sets = unsafe {
            ctx.device().allocate_descriptor_sets(&alloc_info)
                .map_err(|e| anyhow::anyhow!("Failed to allocate descriptor sets: {:?}", e))?
        };

        // Create command pool
        let cmd_pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(ctx.queue_family_index())
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

        let command_pool = unsafe {
            ctx.device().create_command_pool(&cmd_pool_info, None)
                .map_err(|e| anyhow::anyhow!("Failed to create command pool: {:?}", e))?
        };

        // Allocate command buffers
        let cmd_alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(num_descriptor_sets as u32);

        let command_buffers = unsafe {
            ctx.device().allocate_command_buffers(&vk::CommandBufferAllocateInfo::default()
                .command_pool(command_pool)
                .command_buffer_count(num_descriptor_sets as u32)
                .level(vk::CommandBufferLevel::PRIMARY))
                .map_err(|e| anyhow::anyhow!("Failed to allocate command buffers: {:?}", e))?
        };

        Ok(Self {
            shader_module,
            pipeline_layout,
            pipeline,
            descriptor_set_layout,
            descriptor_pool,
            descriptor_sets,
            command_pool,
            command_buffers,
        })
    }

    /// Load embedded SPIR-V shader
    fn load_spirv(name: &str) -> Vec<u32> {
        match name {
            "matmul" => include_bytes!("shaders/spv/matmul.spv").chunks_exact(4)
                .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect(),
            "rms_norm" => include_bytes!("shaders/spv/rms_norm.spv").chunks_exact(4)
                .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect(),
            "activation" => include_bytes!("shaders/spv/activation.spv").chunks_exact(4)
                .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect(),
            _ => panic!("Unknown shader: {}", name),
        }
    }

    fn create_shader_module(ctx: &VulkanContext, code: &[u32]) -> Result<vk::ShaderModule> {
        let info = vk::ShaderModuleCreateInfo::default().code(code);
        unsafe { Ok(ctx.device().create_shader_module(&info, None)?) }
    }

    /// Update descriptor set with buffers
    pub fn update_descriptor_set(
        &self,
        ctx: &VulkanContext,
        set_index: usize,
        buffers: &[vk::Buffer],
    ) {
        let set = self.descriptor_sets[set_index];
        let device = ctx.device();

        for (binding_idx, &buffer) in buffers.iter().enumerate() {
            let buffer_info = vk::DescriptorBufferInfo::default()
                .buffer(buffer)
                .offset(0)
                .range(vk::WHOLE_SIZE);
            let buffer_info_arr = [buffer_info];

            let write = vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(binding_idx as u32)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(&buffer_info_arr);

            unsafe {
                device.update_descriptor_sets(&[write], &[]);
            }
        }
    }

    /// Dispatch compute shader
    pub fn dispatch(
        &self,
        ctx: &VulkanContext,
        set_index: usize,
        group_count_x: u32,
        group_count_y: u32,
        group_count_z: u32,
        push_constants: &[u8],
    ) -> Result<()> {
        let cmd = self.command_buffers[set_index];

        // Reset and begin command buffer
        unsafe {
            ctx.device().reset_command_buffer(
                cmd,
                vk::CommandBufferResetFlags::RELEASE_RESOURCES,
            )?;
        }

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe { ctx.device().begin_command_buffer(cmd, &begin_info)?; }

        // Bind pipeline
        unsafe {
            ctx.device().cmd_bind_pipeline(
                cmd,
                vk::PipelineBindPoint::COMPUTE,
                self.pipeline,
            );
        }

        // Bind descriptor set
        unsafe {
            ctx.device().cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::COMPUTE,
                self.pipeline_layout,
                0,
                &[self.descriptor_sets[set_index]],
                &[],
            );
        }

        // Push constants
        if !push_constants.is_empty() {
            unsafe {
                ctx.device().cmd_push_constants(
                    cmd,
                    self.pipeline_layout,
                    vk::ShaderStageFlags::COMPUTE,
                    0,
                    push_constants,
                );
            }
        }

        // Dispatch
        unsafe {
            ctx.device().cmd_dispatch(cmd, group_count_x, group_count_y, group_count_z);
        }

        unsafe { ctx.device().end_command_buffer(cmd)?; }

        // Submit
        let submit_info = vk::SubmitInfo::default();
        let cmd_buffers = [cmd];
        let submit_info = submit_info.command_buffers(&cmd_buffers);
        let fence_info = vk::FenceCreateInfo::default();
        let fence = unsafe { ctx.device().create_fence(&fence_info, None)? };

        unsafe {
            ctx.device().queue_submit(ctx.compute_queue(), &[submit_info], fence)?;
            ctx.device().wait_for_fences(&[fence], true, u64::MAX)?;
            ctx.device().destroy_fence(fence, None);
        }

        Ok(())
    }
}

impl Drop for GpuPipeline {
    fn drop(&mut self) {
        // Cleanup is handled by VulkanContext's drop
    }
}

/// GPU Pipeline manager holding all shader pipelines
pub struct GpuPipelineManager {
    pub matmul_pipeline: Option<GpuPipeline>,
    pub rms_norm_pipeline: Option<GpuPipeline>,
    pub activation_pipeline: Option<GpuPipeline>,
}

impl GpuPipelineManager {
    pub fn new(ctx: &VulkanContext) -> Result<Self> {
        // Matmul: 3 buffers (A, B, Result)
        let matmul_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
            vk::DescriptorSetLayoutBinding::default()
                .binding(2)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
        ];

        let matmul_pipeline = GpuPipeline::new(ctx, "matmul", 1, &matmul_bindings).ok();

        // RMSNorm: 3 buffers (input, weight, result)
        let rms_norm_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
            vk::DescriptorSetLayoutBinding::default()
                .binding(2)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
        ];

        let rms_norm_pipeline = GpuPipeline::new(ctx, "rms_norm", 1, &rms_norm_bindings).ok();

        // Activation: 3 buffers (x, y optional, result)
        let activation_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
            vk::DescriptorSetLayoutBinding::default()
                .binding(2)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
        ];

        let activation_pipeline = GpuPipeline::new(ctx, "activation", 1, &activation_bindings).ok();

        debug!(
            "GPU pipelines created: matmul={}, rms_norm={}, activation={}",
            matmul_pipeline.is_some(),
            rms_norm_pipeline.is_some(),
            activation_pipeline.is_some()
        );

        Ok(Self {
            matmul_pipeline,
            rms_norm_pipeline,
            activation_pipeline,
        })
    }
}
