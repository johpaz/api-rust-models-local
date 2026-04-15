use ash::vk;
use anyhow::Result;
use std::sync::Arc;
use crate::vulkan_context::VulkanContext;
use crate::tensor::Tensor;

/// Shared GPU resources for buffer management
#[derive(Clone)]
pub struct GpuContext {
    pub ctx: Arc<VulkanContext>,
}

impl GpuContext {
    pub fn new(ctx: &Arc<VulkanContext>) -> Self {
        Self { ctx: ctx.clone() }
    }

    /// Free a buffer and its memory
    pub fn free_buffer(&self, buffer: vk::Buffer, memory: vk::DeviceMemory) {
        unsafe {
            self.ctx.device().free_memory(memory, None);
            self.ctx.device().destroy_buffer(buffer, None);
        }
    }
}

/// A tensor stored in GPU device memory
pub struct GpuTensor {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
    pub element_count: usize,
    pub shape: Vec<usize>,
    pub gpu_ctx: GpuContext,
}

impl GpuTensor {
    /// Create GPU tensor from CPU data
    pub fn from_tensor(gpu_ctx: &GpuContext, tensor: &Tensor) -> Result<Self> {
        let usage = vk::BufferUsageFlags::STORAGE_BUFFER
            | vk::BufferUsageFlags::TRANSFER_SRC
            | vk::BufferUsageFlags::TRANSFER_DST;

        let (buffer, memory, element_count) = gpu_ctx.ctx.create_buffer_with_data(&tensor.data, usage)?;

        Ok(Self {
            buffer,
            memory,
            element_count,
            shape: tensor.shape.clone(),
            gpu_ctx: gpu_ctx.clone(),
        })
    }

    /// Create empty GPU tensor
    pub fn zeros(gpu_ctx: &GpuContext, shape: &[usize]) -> Result<Self> {
        let element_count: usize = shape.iter().product();
        let zeros = vec![0.0f32; element_count];
        let usage = vk::BufferUsageFlags::STORAGE_BUFFER
            | vk::BufferUsageFlags::TRANSFER_SRC
            | vk::BufferUsageFlags::TRANSFER_DST;

        let (buffer, memory, _) = gpu_ctx.ctx.create_buffer_with_data(&zeros, usage)?;

        Ok(Self {
            buffer,
            memory,
            element_count,
            shape: shape.to_vec(),
            gpu_ctx: gpu_ctx.clone(),
        })
    }

    /// Read back to CPU
    pub fn to_cpu(&self) -> Result<Tensor> {
        let data = self.gpu_ctx.ctx.read_buffer(self.buffer, self.element_count)?;
        Ok(Tensor::new(data, self.shape.clone()))
    }

    pub fn rows(&self) -> usize {
        if self.shape.len() >= 2 { self.shape[0] } else { 1 }
    }

    pub fn cols(&self) -> usize {
        if self.shape.len() >= 2 { self.shape[1] } else { self.shape[0] }
    }

    pub fn nelems(&self) -> usize {
        self.element_count
    }
}

impl Drop for GpuTensor {
    fn drop(&mut self) {
        // Free GPU buffer and memory
        self.gpu_ctx.free_buffer(self.buffer, self.memory);
    }
}
