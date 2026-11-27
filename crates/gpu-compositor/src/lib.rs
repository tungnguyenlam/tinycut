use bytemuck::{Pod, Zeroable};
use std::sync::Mutex;
use tinycut_core_domain::{BlurStyle, DomainError, DomainResult, NormalizedBox};

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

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShaderParams {
    width: u32,
    height: u32,
    style: u32,
    box_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PixelBox {
    x1: u32,
    y1: u32,
    x2: u32,
    y2: u32,
}

fn to_pixel_boxes(width: u32, height: u32, boxes: &[NormalizedBox]) -> Vec<PixelBox> {
    boxes
        .iter()
        .map(|b| {
            let x1 = ((b.x.clamp(0.0, 1.0) * width as f32).floor() as u32).min(width);
            let y1 = ((b.y.clamp(0.0, 1.0) * height as f32).floor() as u32).min(height);
            let x2 = (((b.x + b.w).clamp(0.0, 1.0) * width as f32).ceil() as u32).min(width);
            let y2 = (((b.y + b.h).clamp(0.0, 1.0) * height as f32).ceil() as u32).min(height);
            PixelBox { x1, y1, x2, y2 }
        })
        .collect()
}

struct CachedFrameBuffers {
    width: u32,
    height: u32,
    box_capacity: usize,
    params_buf: wgpu::Buffer,
    src_buf: wgpu::Buffer,
    dst_buf: wgpu::Buffer,
    staging_buf: wgpu::Buffer,
    boxes_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

/// Hardware-accelerated `wgpu` compute-shader compositor (Metal / Vulkan / DX12).
pub struct WgpuCompositor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    cache: Mutex<Option<CachedFrameBuffers>>,
}

impl WgpuCompositor {
    /// Initializes a `wgpu` device and compiles the `redact.wgsl` compute shader.
    pub fn try_new() -> DomainResult<Self> {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| DomainError::Media("no compatible wgpu adapter found".into()))?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("tinycut-compositor-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .map_err(|e| DomainError::Media(format!("wgpu device request failed: {e}")))?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tinycut-redact-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("redact.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tinycut-redact-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tinycut-redact-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("tinycut-redact-pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Ok(Self {
            device,
            queue,
            pipeline,
            bind_group_layout,
            cache: Mutex::new(None),
        })
    }

    fn allocate_frame_buffers(
        &self,
        width: u32,
        height: u32,
        box_capacity: usize,
    ) -> CachedFrameBuffers {
        let frame_bytes = (width as u64) * (height as u64) * 4;
        let boxes_bytes = (box_capacity * std::mem::size_of::<PixelBox>()) as u64;

        let params_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("params"),
            size: std::mem::size_of::<ShaderParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let src_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("src-pixels"),
            size: frame_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let dst_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dst-pixels"),
            size: frame_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let staging_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging-pixels"),
            size: frame_bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let boxes_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("redact-boxes"),
            size: boxes_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("redact-bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: src_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: dst_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: boxes_buf.as_entire_binding(),
                },
            ],
        });

        CachedFrameBuffers {
            width,
            height,
            box_capacity,
            params_buf,
            src_buf,
            dst_buf,
            staging_buf,
            boxes_buf,
            bind_group,
        }
    }
}

impl GpuCompositor for WgpuCompositor {
    fn render_frame_rgba8(
        &self,
        width: u32,
        height: u32,
        rgba_in: &[u8],
        task: &CompositorFrameTask,
    ) -> DomainResult<Vec<u8>> {
        let expected_bytes = (width as usize) * (height as usize) * 4;
        if rgba_in.len() != expected_bytes {
            return Err(DomainError::Media(format!(
                "rgba buffer length {} does not match {}x{}x4",
                rgba_in.len(),
                width,
                height
            )));
        }
        if task.boxes.is_empty() || expected_bytes == 0 {
            return Ok(rgba_in.to_vec());
        }

        let style_id = match task.style {
            BlurStyle::Gaussian => 0,
            BlurStyle::Pixelate => 1,
            BlurStyle::SolidMask => 2,
        };
        let pixel_boxes = to_pixel_boxes(width, height, &task.boxes);
        let params = ShaderParams {
            width,
            height,
            style: style_id,
            box_count: pixel_boxes.len() as u32,
        };

        let mut cache_guard = self
            .cache
            .lock()
            .map_err(|_| DomainError::Media("wgpu frame buffer cache mutex poisoned".into()))?;

        let needs_realloc = match cache_guard.as_ref() {
            Some(cached) => {
                cached.width != width
                    || cached.height != height
                    || pixel_boxes.len() > cached.box_capacity
            }
            None => true,
        };

        if needs_realloc {
            let box_capacity = pixel_boxes.len().next_power_of_two().max(16);
            *cache_guard = Some(self.allocate_frame_buffers(width, height, box_capacity));
        }

        let cached = cache_guard.as_ref().unwrap();
        self.queue
            .write_buffer(&cached.params_buf, 0, bytemuck::bytes_of(&params));
        self.queue.write_buffer(&cached.src_buf, 0, rgba_in);
        self.queue.write_buffer(
            &cached.boxes_buf,
            0,
            bytemuck::cast_slice(&pixel_boxes),
        );

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("redact-encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("redact-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &cached.bind_group, &[]);
            pass.dispatch_workgroups(width.div_ceil(16), height.div_ceil(16), 1);
        }
        encoder.copy_buffer_to_buffer(
            &cached.dst_buf,
            0,
            &cached.staging_buf,
            0,
            expected_bytes as u64,
        );
        self.queue.submit(Some(encoder.finish()));

        let slice = cached.staging_buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .map_err(|_| DomainError::Media("wgpu readback channel closed".into()))?
            .map_err(|e| DomainError::Media(format!("wgpu buffer map failed: {e}")))?;

        let mapped = slice.get_mapped_range();
        let out = mapped.to_vec();
        drop(mapped);
        cached.staging_buf.unmap();
        Ok(out)
    }
}

/// Returns a hardware-accelerated `WgpuCompositor` if a compatible GPU adapter is available,
/// otherwise falls back to `CpuCompositor`.
pub fn create_best_compositor() -> Box<dyn GpuCompositor> {
    match WgpuCompositor::try_new() {
        Ok(gpu) => Box::new(gpu),
        Err(_) => Box::new(CpuCompositor),
    }
}

/// Deterministic CPU fallback compositor (used on headless servers or machines without a GPU adapter).
#[derive(Debug, Default, Clone)]
pub struct CpuCompositor;

impl GpuCompositor for CpuCompositor {
    fn render_frame_rgba8(
        &self,
        width: u32,
        height: u32,
        rgba_in: &[u8],
        task: &CompositorFrameTask,
    ) -> DomainResult<Vec<u8>> {
        let expected_bytes = (width as usize) * (height as usize) * 4;
        if rgba_in.len() != expected_bytes {
            return Err(DomainError::Media(format!(
                "rgba buffer length {} does not match {}x{}x4",
                rgba_in.len(),
                width,
                height
            )));
        }
        let mut out = rgba_in.to_vec();
        if task.boxes.is_empty() || expected_bytes == 0 {
            return Ok(out);
        }

        let boxes = to_pixel_boxes(width, height, &task.boxes);
        for y in 0..height {
            for x in 0..width {
                let inside = boxes
                    .iter()
                    .any(|b| x >= b.x1 && x < b.x2 && y >= b.y1 && y < b.y2);
                if !inside {
                    continue;
                }
                let dst_offset = ((y * width + x) as usize) * 4;
                match task.style {
                    BlurStyle::SolidMask => {
                        out[dst_offset..dst_offset + 4].copy_from_slice(&[18, 18, 22, 255]);
                    }
                    BlurStyle::Pixelate => {
                        let block = 12;
                        let bx = ((x / block) * block + block / 2).min(width - 1);
                        let by = ((y / block) * block + block / 2).min(height - 1);
                        let src_offset = ((by * width + bx) as usize) * 4;
                        out[dst_offset..dst_offset + 4]
                            .copy_from_slice(&rgba_in[src_offset..src_offset + 4]);
                    }
                    BlurStyle::Gaussian => {
                        let mut acc = [0.0_f32; 4];
                        let mut count = 0.0_f32;
                        let radius = 3_i32;
                        for dy in -radius..=radius {
                            let sy = (y as i32 + dy).clamp(0, height as i32 - 1) as u32;
                            for dx in -radius..=radius {
                                let sx = (x as i32 + dx).clamp(0, width as i32 - 1) as u32;
                                let src_offset = ((sy * width + sx) as usize) * 4;
                                for c in 0..4 {
                                    acc[c] += rgba_in[src_offset + c] as f32;
                                }
                                count += 1.0;
                            }
                        }
                        for c in 0..4 {
                            out[dst_offset + c] = (acc[c] / count).round().clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_compositor_applies_mask_only_inside_box() {
        let w = 16;
        let h = 16;
        let mut frame = vec![255u8; (w * h * 4) as usize];
        // Give top-left pixel a distinct color outside the redaction box
        frame[0..4].copy_from_slice(&[10, 20, 30, 255]);

        let task = CompositorFrameTask {
            frame_index: 0,
            style: BlurStyle::SolidMask,
            boxes: vec![NormalizedBox {
                x: 0.5,
                y: 0.5,
                w: 0.5,
                h: 0.5,
            }],
        };

        let out = CpuCompositor
            .render_frame_rgba8(w, h, &frame, &task)
            .unwrap();
        // Outside box remains unchanged
        assert_eq!(&out[0..4], &[10, 20, 30, 255]);
        // Inside box (pixel 12,12) is masked
        let inside = ((12 * w + 12) as usize) * 4;
        assert_eq!(&out[inside..inside + 4], &[18, 18, 22, 255]);
    }

    #[test]
    fn cpu_compositor_gaussian_blurs_high_contrast_region() {
        let w = 16;
        let h = 16;
        let mut frame = vec![0u8; (w * h * 4) as usize];
        // Checkerboard pattern
        for y in 0..h {
            for x in 0..w {
                let off = ((y * w + x) as usize) * 4;
                let v = if (x + y) % 2 == 0 { 240 } else { 0 };
                frame[off..off + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }

        let task = CompositorFrameTask {
            frame_index: 1,
            style: BlurStyle::Gaussian,
            boxes: vec![NormalizedBox {
                x: 0.25,
                y: 0.25,
                w: 0.5,
                h: 0.5,
            }],
        };

        let out = CpuCompositor
            .render_frame_rgba8(w, h, &frame, &task)
            .unwrap();
        let center = ((8 * w + 8) as usize) * 4;
        // Center pixel should be averaged rather than pure 240 or 0
        assert!(out[center] > 40 && out[center] < 220);
    }

    #[test]
    fn best_compositor_renders_multiple_frames_and_reuses_buffers() {
        let compositor = create_best_compositor();
        let w = 32_u32;
        let h = 24_u32;

        // Render several frames with identical dimensions (exercises buffer reuse in WgpuCompositor)
        for frame_idx in 0..5_u64 {
            let mut frame = vec![200u8; (w * h * 4) as usize];
            frame[0..4].copy_from_slice(&[15, 25, 35, 255]);

            let style = match frame_idx % 3 {
                0 => BlurStyle::SolidMask,
                1 => BlurStyle::Pixelate,
                _ => BlurStyle::Gaussian,
            };
            let task = CompositorFrameTask {
                frame_index: frame_idx,
                style,
                boxes: vec![NormalizedBox {
                    x: 0.5,
                    y: 0.5,
                    w: 0.4,
                    h: 0.4,
                }],
            };

            let out = compositor
                .render_frame_rgba8(w, h, &frame, &task)
                .expect("render should succeed");
            assert_eq!(out.len(), frame.len());
            assert_eq!(&out[0..4], &[15, 25, 35, 255]);
            if style == BlurStyle::SolidMask {
                let inside = ((16 * w + 20) as usize) * 4;
                assert_eq!(&out[inside..inside + 4], &[18, 18, 22, 255]);
            }
        }

        // Grow box count past initial capacity (16) to exercise boxes_buf reallocation
        let many_boxes: Vec<NormalizedBox> = (0..20)
            .map(|i| NormalizedBox {
                x: 0.5 + (i as f32) * 0.01,
                y: 0.5,
                w: 0.1,
                h: 0.1,
            })
            .collect();
        let frame = vec![220u8; (w * h * 4) as usize];
        let out = compositor
            .render_frame_rgba8(
                w,
                h,
                &frame,
                &CompositorFrameTask {
                    frame_index: 5,
                    style: BlurStyle::SolidMask,
                    boxes: many_boxes,
                },
            )
            .expect("reallocation for >16 boxes should succeed");
        assert_eq!(out.len(), frame.len());

        // Change frame dimensions to exercise full buffer reallocation
        let w2 = 16_u32;
        let h2 = 16_u32;
        let frame2 = vec![180u8; (w2 * h2 * 4) as usize];
        let out2 = compositor
            .render_frame_rgba8(
                w2,
                h2,
                &frame2,
                &CompositorFrameTask {
                    frame_index: 6,
                    style: BlurStyle::SolidMask,
                    boxes: vec![NormalizedBox {
                        x: 0.0,
                        y: 0.0,
                        w: 0.5,
                        h: 0.5,
                    }],
                },
            )
            .expect("reallocation for new dimensions should succeed");
        assert_eq!(out2.len(), frame2.len());
        assert_eq!(&out2[0..4], &[18, 18, 22, 255]);
    }
}
