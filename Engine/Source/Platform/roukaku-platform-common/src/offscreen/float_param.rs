use super::*;

/// Creates an instance with the corresponding native platform backend policy.
/// Retain it in the graphics owner and lend it to each session renderer so native
/// backend loaders remain loaded across session creation and destruction.
pub fn native_wgpu_instance() -> Result<wgpu::Instance, PlatformError> {
    Ok(wgpu::Instance::new(native_instance_descriptor()?))
}

fn native_instance_descriptor() -> Result<wgpu::InstanceDescriptor, PlatformError> {
    #[cfg(any(
        target_os = "windows",
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    ))]
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    #[cfg(target_os = "windows")]
    {
        descriptor.backends = wgpu::Backends::DX12;
        descriptor.backend_options.dx12.shader_compiler = wgpu::Dx12Compiler::StaticDxc;
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        descriptor.backends = wgpu::Backends::VULKAN;
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        descriptor.backends = wgpu::Backends::METAL;
    }
    #[cfg(not(any(
        target_os = "windows",
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    )))]
    {
        Err(unavailable(
            "offscreen.platform",
            "offscreen GPU is implemented only for Windows, Linux, Android, macOS, and iOS",
        ))
    }
    #[cfg(any(
        target_os = "windows",
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    ))]
    Ok(descriptor)
}

pub(super) fn native_backend_mask() -> wgpu::Backends {
    #[cfg(target_os = "windows")]
    return wgpu::Backends::DX12;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    return wgpu::Backends::VULKAN;
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    return wgpu::Backends::METAL;
    #[cfg(not(any(
        target_os = "windows",
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    )))]
    return wgpu::Backends::empty();
}

#[derive(Clone, Copy)]
struct ReadbackLayout {
    width: u32,
    height: u32,
    row: u32,
    padded: u32,
}

impl ReadbackLayout {
    fn new(width: u32, height: u32) -> Result<Self, PlatformError> {
        let row = width
            .checked_mul(4)
            .ok_or_else(|| invalid("offscreen.readback", "frame row overflows"))?;
        let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = row
            .div_ceil(alignment)
            .checked_mul(alignment)
            .ok_or_else(|| invalid("offscreen.readback", "padded frame row overflows"))?;
        Ok(Self {
            width,
            height,
            row,
            padded,
        })
    }

    fn padded_size(self) -> u64 {
        u64::from(self.padded) * u64::from(self.height)
    }

    fn buffer(self, device: &wgpu::Device, label: &'static str) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: self.padded_size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        })
    }

    fn encode(
        self,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        buffer: wgpu::Buffer,
    ) -> PendingReadback {
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        let (mapped, mapping) = mpsc::sync_channel(1);
        encoder.map_buffer_on_submit(&buffer, wgpu::MapMode::Read, .., move |result| {
            let _ = mapped.send(result);
        });
        PendingReadback {
            buffer,
            mapping,
            layout: self,
        }
    }
}

pub(super) struct PendingReadback {
    buffer: wgpu::Buffer,
    mapping: mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
    layout: ReadbackLayout,
}

pub(super) fn encode_readback(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
    ring: &mut ReadbackRing,
) -> Result<PendingReadback, PlatformError> {
    let layout = ReadbackLayout::new(width, height)?;
    if ring.width != width
        || ring.height != height
        || ring.padded_row_bytes != layout.padded
        || ring.slots.len() != READBACK_RING_SIZE
    {
        ring.width = width;
        ring.height = height;
        ring.padded_row_bytes = layout.padded;
        ring.next_slot = 0;
        ring.slots = (0..READBACK_RING_SIZE)
            .map(|_| layout.buffer(device, "roukaku-offscreen-readback-ring"))
            .collect();
    }
    let slot = ring.next_slot;
    ring.next_slot = (ring.next_slot + 1) % READBACK_RING_SIZE;
    let buffer = ring
        .slots
        .get(slot)
        .ok_or_else(|| invalid("offscreen.readback", "readback ring slot is unavailable"))?;
    Ok(layout.encode(encoder, texture, buffer.clone()))
}

impl PendingReadback {
    pub(super) fn finish(
        self,
        device: &wgpu::Device,
        submission: wgpu::SubmissionIndex,
    ) -> Result<CapturedFrame, PlatformError> {
        self.with_mapped(device, submission, |mapped, layout| {
            let row = layout.row as usize;
            let padded = layout.padded as usize;
            let height = layout.height as usize;
            let rgba8 = if row == padded {
                Arc::from(&mapped[..row * height])
            } else {
                let mut pixels = Vec::with_capacity(row * height);
                for source in mapped.chunks_exact(padded).take(height) {
                    pixels.extend_from_slice(&source[..row]);
                }
                pixels.into()
            };
            CapturedFrame {
                width: layout.width,
                height: layout.height,
                rgba8,
            }
        })
    }

    fn with_mapped<T>(
        self,
        device: &wgpu::Device,
        submission: wgpu::SubmissionIndex,
        read: impl FnOnce(&[u8], ReadbackLayout) -> T,
    ) -> Result<T, PlatformError> {
        let Self {
            buffer,
            mapping,
            layout,
        } = self;
        let deadline = Instant::now() + READBACK_TIMEOUT;
        if device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(READBACK_TIMEOUT),
            })
            .is_err()
        {
            buffer.unmap();
            return Err(unavailable(
                "offscreen.readback",
                "GPU readback exceeded its bounded timeout",
            ));
        }
        let result = match mapping.recv_timeout(deadline.saturating_duration_since(Instant::now()))
        {
            Ok(result) => result,
            Err(_) => {
                buffer.unmap();
                return Err(unavailable(
                    "offscreen.readback",
                    "GPU readback callback was lost",
                ));
            }
        };
        result.map_err(|_| unavailable("offscreen.readback", "GPU readback mapping failed"))?;
        let mapped = match buffer.slice(..).get_mapped_range() {
            Ok(mapped) => mapped,
            Err(_) => {
                buffer.unmap();
                return Err(unavailable(
                    "offscreen.readback",
                    "GPU readback mapped range is unavailable",
                ));
            }
        };
        let value = read(&mapped, layout);
        drop(mapped);
        buffer.unmap();
        Ok(value)
    }
}

impl crate::WgpuSceneTarget {
    /// Read the target into tightly packed, top-down encoded RGBA8, retaining alpha.
    /// This blocks for an explicit capture; ordinary rendering never maps the target.
    /// `dst` must contain exactly `width * height * 4` bytes. On success every
    /// byte is initialized. Use the same device and queue as for rendering.
    pub fn capture_into(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        dst: &mut [std::mem::MaybeUninit<u8>],
    ) -> Result<(), PlatformError> {
        let texture = self.texture();
        let layout = ReadbackLayout::new(texture.width(), texture.height())?;
        if u64::from(layout.row) * u64::from(layout.height) != dst.len() as u64 {
            return Err(invalid(
                "offscreen.readback",
                "capture buffer does not match the target extent",
            ));
        }
        if layout.padded_size() > device.limits().max_buffer_size {
            return Err(unavailable(
                "offscreen.readback",
                "padded capture exceeds the device buffer size limit",
            ));
        }
        let mut encoder = device.create_command_encoder(&Default::default());
        let buffer = layout.buffer(device, "roukaku-shared-scene-capture");
        let readback = layout.encode(&mut encoder, texture, buffer);
        let submission = queue.submit([encoder.finish()]);
        readback.with_mapped(device, submission, |mapped, layout| {
            let row = layout.row as usize;
            let padded = layout.padded as usize;
            for (index, target) in dst.chunks_exact_mut(row).enumerate() {
                let start = index * padded;
                let source = &mapped[start..start + row];
                // SAFETY: Each row is in bounds and the caller's allocation
                // does not overlap the mapped staging buffer.
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        source.as_ptr(),
                        target.as_mut_ptr().cast::<u8>(),
                        row,
                    );
                }
            }
        })
    }
}

pub(super) fn validate_native_backend(backend: wgpu::Backend) -> Result<(), PlatformError> {
    let valid = cfg!(target_os = "windows") && backend == wgpu::Backend::Dx12
        || cfg!(any(target_os = "linux", target_os = "android"))
            && backend == wgpu::Backend::Vulkan
        || cfg!(any(target_os = "macos", target_os = "ios")) && backend == wgpu::Backend::Metal;
    if valid {
        Ok(())
    } else {
        Err(unavailable(
            "offscreen.backend",
            "adapter backend does not match the native platform policy",
        ))
    }
}

#[cfg(all(test, target_os = "android"))]
mod android_tests {
    use super::*;

    #[test]
    fn android_instance_and_adapter_selection_use_vulkan() {
        let descriptor = native_instance_descriptor().unwrap();
        let mask = native_backend_mask();
        assert!(!mask.is_empty());
        assert_eq!(descriptor.backends, wgpu::Backends::VULKAN);
        assert_eq!(descriptor.backends, mask);
        assert!(validate_native_backend(wgpu::Backend::Vulkan).is_ok());
        for backend in [
            wgpu::Backend::Noop,
            wgpu::Backend::Dx12,
            wgpu::Backend::Metal,
            wgpu::Backend::Gl,
            wgpu::Backend::BrowserWebGpu,
        ] {
            assert!(validate_native_backend(backend).is_err());
        }
    }
}

pub(super) fn backend_name(backend: wgpu::Backend) -> &'static str {
    match backend {
        wgpu::Backend::Dx12 => "dx12",
        wgpu::Backend::Vulkan => "vulkan",
        wgpu::Backend::Metal => "metal",
        _ => "unsupported",
    }
}

pub(super) fn device_type_name(device: wgpu::DeviceType) -> &'static str {
    match device {
        wgpu::DeviceType::DiscreteGpu => "discrete_gpu",
        wgpu::DeviceType::IntegratedGpu => "integrated_gpu",
        wgpu::DeviceType::VirtualGpu => "virtual_gpu",
        wgpu::DeviceType::Cpu => "cpu",
        _ => "other",
    }
}

pub(super) fn hash(bytes: &[u8]) -> String {
    roukaku_core::Hash256::from_sha256(bytes).to_string()
}

pub(super) fn invalid(operation: &'static str, message: &'static str) -> PlatformError {
    PlatformError::new(PlatformErrorCode::InvalidState, operation, message)
}

pub(super) fn unavailable(operation: &'static str, message: &'static str) -> PlatformError {
    PlatformError::new(PlatformErrorCode::ProviderUnavailable, operation, message)
}
