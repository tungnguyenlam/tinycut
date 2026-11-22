use tinycut_core_domain::{BlurStyle, DomainResult, NormalizedBox};

#[derive(Debug, Clone)]
pub struct CompositorFrameTask {
    pub frame_index: u64,
    pub style: BlurStyle,
    pub boxes: Vec<NormalizedBox>,
}

/// Cross-platform GPU redaction compositor interface (backed by `wgpu` Metal/Vulkan/DX12 shaders).
pub trait GpuCompositor: Send + Sync {
    fn render_frame_rgba8(
        &self,
        width: u32,
        height: u32,
        rgba_in: &[u8],
        task: &CompositorFrameTask,
    ) -> DomainResult<Vec<u8>>;
}
