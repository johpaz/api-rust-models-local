use anyhow::Result;
use std::sync::Arc;
use crate::vulkan_context::VulkanContext;
use crate::gpu_tensor::{GpuTensor, GpuContext};
use crate::tensor::Tensor;
use crate::gpu_pipeline::{GpuPipelineManager, GpuPipeline};

/// GPU compute operations using Vulkan compute shaders
pub struct GpuOps {
    ctx: Arc<VulkanContext>,
    gpu_ctx: GpuContext,
    pipelines: GpuPipelineManager,
}

impl GpuOps {
    pub fn new(ctx: Arc<VulkanContext>) -> Self {
        let pipelines = GpuPipelineManager::new(&ctx)
            .expect("Failed to create GPU pipeline manager");
        let gpu_ctx = GpuContext::new(&ctx);

        Self { ctx, gpu_ctx, pipelines }
    }

    /// Matrix multiply: A[M,K] @ B[K,N] -> C[M,N]
    pub fn matmul_gpu(&self, a: &GpuTensor, b: &GpuTensor) -> Result<GpuTensor> {
        let m = a.rows();
        let k = a.cols();
        let n = b.cols();

        assert_eq!(k, b.rows(), "Matmul shape mismatch: {}x{} @ {}x{}", m, k, b.rows(), n);

        // Create output tensor
        let out_shape = vec![m, n];
        let zeros = vec![0.0f32; m * n];
        let use_flags = ash::vk::BufferUsageFlags::STORAGE_BUFFER
            | ash::vk::BufferUsageFlags::TRANSFER_SRC
            | ash::vk::BufferUsageFlags::TRANSFER_DST;
        let (buffer, memory, _) = self.gpu_ctx.ctx.create_buffer_with_data(&zeros, use_flags)?;
        let output = GpuTensor {
            buffer,
            memory,
            element_count: m * n,
            shape: out_shape,
            gpu_ctx: self.gpu_ctx.clone(),
        };

        // Use GPU pipeline if available
        if let Some(ref pipeline) = self.pipelines.matmul_pipeline {
            // Update descriptor set
            pipeline.update_descriptor_set(&self.gpu_ctx.ctx, 0, &[a.buffer, b.buffer, output.buffer]);

            // Push constants: M, N, K
            let push_data: [u32; 3] = [m as u32, n as u32, k as u32];
            let push_bytes: &[u8] = bytemuck::cast_slice(&push_data);

            // Dispatch: 256 threads per workgroup
            let group_x = ((n as u32 + 255) / 256).max(1);
            let group_y = (m as u32).max(1);

            pipeline.dispatch(&self.gpu_ctx.ctx, 0, group_x, group_y, 1, push_bytes)?;
        } else {
            // Fallback to CPU
            let a_cpu = a.to_cpu()?;
            let b_cpu = b.to_cpu()?;
            let result = a_cpu.matmul(&b_cpu);
            let new_output = GpuTensor::from_tensor(&self.gpu_ctx, &result)?;

            // Copy result to output buffer
            self.copy_buffer_to_buffer(&new_output.buffer, &output.buffer, (result.nelems() * 4) as u64)?;
        }

        Ok(output)
    }

    /// RMSNorm: x / sqrt(mean(x^2) + eps) * w
    pub fn rms_norm_gpu(&self, x: &GpuTensor, w: &GpuTensor, eps: f32) -> Result<GpuTensor> {
        let n = x.nelems();
        let zeros = vec![0.0f32; n];
        let use_flags = ash::vk::BufferUsageFlags::STORAGE_BUFFER
            | ash::vk::BufferUsageFlags::TRANSFER_SRC
            | ash::vk::BufferUsageFlags::TRANSFER_DST;
        let (buffer, memory, _) = self.gpu_ctx.ctx.create_buffer_with_data(&zeros, use_flags)?;
        let output = GpuTensor {
            buffer,
            memory,
            element_count: n,
            shape: x.shape.clone(),
            gpu_ctx: self.gpu_ctx.clone(),
        };

        if let Some(ref pipeline) = self.pipelines.rms_norm_pipeline {
            pipeline.update_descriptor_set(&self.gpu_ctx.ctx, 0, &[x.buffer, w.buffer, output.buffer]);

            // Push constants: N (u32), eps (f32)
            let push_data: [u32; 2] = [n as u32, eps.to_bits()];
            let push_bytes: &[u8] = bytemuck::cast_slice(&push_data);

            let group_x = ((n as u32 + 255) / 256).max(1);
            pipeline.dispatch(&self.gpu_ctx.ctx, 0, group_x, 1, 1, push_bytes)?;
        } else {
            let x_cpu = x.to_cpu()?;
            let w_cpu = w.to_cpu()?;
            let result = x_cpu.rms_norm(&w_cpu, eps);
            let new_output = GpuTensor::from_tensor(&self.gpu_ctx, &result)?;
            self.copy_buffer_to_buffer(&new_output.buffer, &output.buffer, (n * 4) as u64)?;
        }

        Ok(output)
    }

    /// Element-wise SiLU
    pub fn silu_gpu(&self, x: &GpuTensor) -> Result<GpuTensor> {
        self.activation_gpu(x, None, 0)
    }

    /// Element-wise multiply
    pub fn mul_gpu(&self, a: &GpuTensor, b: &GpuTensor) -> Result<GpuTensor> {
        self.activation_gpu(a, Some(b), 2)
    }

    /// Element-wise add
    pub fn add_gpu(&self, a: &GpuTensor, b: &GpuTensor) -> Result<GpuTensor> {
        self.activation_gpu(a, Some(b), 3)
    }

    /// Generic activation: op 0=silu, 1=gelu, 2=mul, 3=add, 4=scale
    fn activation_gpu(&self, x: &GpuTensor, y: Option<&GpuTensor>, op: u32) -> Result<GpuTensor> {
        let n = x.nelems();
        let zeros = vec![0.0f32; n];
        let use_flags = ash::vk::BufferUsageFlags::STORAGE_BUFFER
            | ash::vk::BufferUsageFlags::TRANSFER_SRC
            | ash::vk::BufferUsageFlags::TRANSFER_DST;
        let (buffer, memory, _) = self.gpu_ctx.ctx.create_buffer_with_data(&zeros, use_flags)?;
        let output = GpuTensor {
            buffer,
            memory,
            element_count: n,
            shape: x.shape.clone(),
            gpu_ctx: self.gpu_ctx.clone(),
        };

        let y_buffer = y.map(|t| t.buffer).unwrap_or(x.buffer);

        if let Some(ref pipeline) = self.pipelines.activation_pipeline {
            pipeline.update_descriptor_set(&self.gpu_ctx.ctx, 0, &[x.buffer, y_buffer, output.buffer]);

            let push_data: [u32; 3] = [n as u32, op, 0];
            let push_bytes: &[u8] = bytemuck::cast_slice(&push_data);

            let group_x = ((n as u32 + 255) / 256).max(1);
            pipeline.dispatch(&self.gpu_ctx.ctx, 0, group_x, 1, 1, push_bytes)?;
        } else {
            let x_cpu = x.to_cpu()?;
            let result = match op {
                0 => x_cpu.silu(),
                2 => {
                    let y_cpu = y.unwrap().to_cpu()?;
                    x_cpu.mul(&y_cpu)
                }
                3 => {
                    let y_cpu = y.unwrap().to_cpu()?;
                    x_cpu.add(&y_cpu)
                }
                _ => x_cpu.clone(),
            };
            let new_output = GpuTensor::from_tensor(&self.gpu_ctx, &result)?;
            self.copy_buffer_to_buffer(&new_output.buffer, &output.buffer, (n * 4) as u64)?;
        }

        Ok(output)
    }

    /// Softmax (CPU fallback)
    pub fn softmax_gpu(&self, x: &GpuTensor) -> Result<GpuTensor> {
        let x_cpu = x.to_cpu()?;
        let result = x_cpu.softmax();
        GpuTensor::from_tensor(&self.gpu_ctx, &result)
    }

    /// Copy data between GPU buffers
    fn copy_buffer_to_buffer(&self, src: &ash::vk::Buffer, dst: &ash::vk::Buffer, size: u64) -> Result<()> {
        let cmd_pool_info = ash::vk::CommandPoolCreateInfo::default()
            .queue_family_index(self.gpu_ctx.ctx.queue_family_index())
            .flags(ash::vk::CommandPoolCreateFlags::TRANSIENT);

        let cmd_pool = unsafe { self.gpu_ctx.ctx.device().create_command_pool(&cmd_pool_info, None) }?;

        let cmd_alloc_info = ash::vk::CommandBufferAllocateInfo::default()
            .command_pool(cmd_pool)
            .level(ash::vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let cmd_buffers = unsafe { self.gpu_ctx.ctx.device().allocate_command_buffers(&cmd_alloc_info) }?;
        let cmd = cmd_buffers[0];

        let begin_info = ash::vk::CommandBufferBeginInfo::default()
            .flags(ash::vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        unsafe {
            self.gpu_ctx.ctx.device().begin_command_buffer(cmd, &begin_info)?;
            let copy_region = ash::vk::BufferCopy::default().size(size);
            self.gpu_ctx.ctx.device().cmd_copy_buffer(cmd, *src, *dst, &[copy_region]);
            self.gpu_ctx.ctx.device().end_command_buffer(cmd)?;
        }

        let submit_info = ash::vk::SubmitInfo::default();
        let cmd_buffers = [cmd];
        let submit_info = submit_info.command_buffers(&cmd_buffers);
        let fence_info = ash::vk::FenceCreateInfo::default();
        let fence = unsafe { self.gpu_ctx.ctx.device().create_fence(&fence_info, None)? };

        unsafe {
            self.gpu_ctx.ctx.device().queue_submit(self.gpu_ctx.ctx.compute_queue(), &[submit_info], fence)?;
            self.gpu_ctx.ctx.device().wait_for_fences(&[fence], true, u64::MAX)?;
            self.gpu_ctx.ctx.device().destroy_fence(fence, None);
            self.gpu_ctx.ctx.device().free_command_buffers(cmd_pool, &[cmd]);
            self.gpu_ctx.ctx.device().destroy_command_pool(cmd_pool, None);
        }

        Ok(())
    }

    /// Benchmark: run a GPU matmul and return time
    pub fn benchmark(&self, size: usize) -> Result<f64> {
        let data = vec![1.0f32; size * size];
        let t1 = GpuTensor::from_tensor(
            &self.gpu_ctx,
            &Tensor::new(data.clone(), vec![size, size]),
        )?;
        let t2 = GpuTensor::from_tensor(
            &self.gpu_ctx,
            &Tensor::new(data, vec![size, size]),
        )?;

        let start = std::time::Instant::now();
        let _result = self.matmul_gpu(&t1, &t2)?;
        let elapsed = start.elapsed();

        Ok(elapsed.as_secs_f64())
    }

    pub fn has_gpu_pipelines(&self) -> bool {
        self.pipelines.matmul_pipeline.is_some()
            && self.pipelines.rms_norm_pipeline.is_some()
            && self.pipelines.activation_pipeline.is_some()
    }
}
