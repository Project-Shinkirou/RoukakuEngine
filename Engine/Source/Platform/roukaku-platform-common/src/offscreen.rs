use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use roukaku_media_core::SceneCommand;

use roukaku_platform::{
    CapturedFrame, GpuAdapterPolicy, GpuBackendPolicy, GpuDeviceTypePolicy, PlatformError,
    PlatformErrorCode, SceneFrame,
};

use crate::{
    glyph_atlas::{GlyphProfileQueries, WgpuGlyphAtlasRenderer},
    presentation::install_device_lost_callback,
    scene_filters::SceneFilters,
};

#[cfg(test)]
mod adapter_identity_tests {
    use super::renderer_execution_identity;
    use super::submit_frame_internal::adapter_policy_identity;

    fn adapter_info(driver_info: &str) -> wgpu::AdapterInfo {
        wgpu::AdapterInfo {
            name: "bounded-test-adapter".into(),
            vendor: 0x1002,
            device: 0x150e,
            device_type: wgpu::DeviceType::IntegratedGpu,
            device_pci_bus_id: String::new(),
            driver: "test-driver".into(),
            driver_info: driver_info.into(),
            backend: wgpu::Backend::Dx12,
            subgroup_min_size: 32,
            subgroup_max_size: 64,
            transient_saves_memory: Some(false),
            limit_bucket: None,
        }
    }

    #[test]
    fn policy_identity_is_the_reported_renderer_identity_hash() {
        let info = adapter_info("1.2.3");
        assert_eq!(
            adapter_policy_identity(&info),
            renderer_execution_identity(&info).hash()
        );
        assert_ne!(
            adapter_policy_identity(&info),
            adapter_policy_identity(&adapter_info("1.2.4"))
        );
    }
}

mod wgpu_offscreen_renderer;
use wgpu_offscreen_renderer::GPU_MAP_FAILED;
use wgpu_offscreen_renderer::GPU_MAP_PENDING;
use wgpu_offscreen_renderer::GPU_MAP_SUCCEEDED;
use wgpu_offscreen_renderer::GpuTimer;
use wgpu_offscreen_renderer::GpuTimerSlot;
use wgpu_offscreen_renderer::MAX_GPU_TIMESTAMP_QUERIES;
use wgpu_offscreen_renderer::PERFORMANCE_ATLAS_BYTES;
use wgpu_offscreen_renderer::PERFORMANCE_ATLAS_SIDE;
use wgpu_offscreen_renderer::PendingOutput;
use wgpu_offscreen_renderer::PendingTimestampRead;
use wgpu_offscreen_renderer::READBACK_RING_SIZE;
use wgpu_offscreen_renderer::READBACK_TIMEOUT;
use wgpu_offscreen_renderer::ReadbackRing;
pub use wgpu_offscreen_renderer::RendererExecutionIdentity;
pub use wgpu_offscreen_renderer::WGPU_TIMESTAMP_RING_SIZE;
pub use wgpu_offscreen_renderer::WgpuFramePerformanceCounters;
pub use wgpu_offscreen_renderer::WgpuOffscreenRenderer;
pub use wgpu_offscreen_renderer::WgpuPendingProfile;
pub use wgpu_offscreen_renderer::WgpuProfiledSubmission;
mod with_default_compositing;

mod submit_frame_internal;
use submit_frame_internal::device_descriptor;
use submit_frame_internal::renderer_execution_identity;
use submit_frame_internal::select_adapter;
mod apply_filter;
use apply_filter::elapsed_ns;
use apply_filter::mark_gpu_timestamp_recorded;
use apply_filter::resolve_gpu_timestamps;
mod float_param;
use float_param::backend_name;
use float_param::device_type_name;
use float_param::encode_readback;
use float_param::hash;
use float_param::invalid;
use float_param::native_backend_mask;
pub use float_param::native_wgpu_instance;
use float_param::unavailable;
use float_param::validate_native_backend;

#[cfg(test)]
mod shared_scene_tests {
    use super::*;
    use crate::{WgpuSceneTarget, WgpuSharedSceneRenderer};
    use roukaku_media_core::{GlyphBitmap, GlyphBitmapFormat, GlyphInstance, SceneCompositing2D};

    #[tokio::test]
    #[ignore = "requires a native hardware GPU runner"]
    async fn shared_scene_matches_offscreen_and_keeps_leased_targets_intact() {
        let instance = native_wgpu_instance().unwrap();
        let mut offscreen = WgpuOffscreenRenderer::new(&instance)
            .await
            .unwrap()
            .with_default_compositing(SceneCompositing2D::EncodedSrgb);
        let mut shared = WgpuSharedSceneRenderer::new(&offscreen.device);
        let mut first = WgpuSceneTarget::new(&offscreen.device, 3, 2).unwrap();
        let mut second = WgpuSceneTarget::new(&offscreen.device, 3, 2).unwrap();
        let mut frame = SceneFrame {
            sequence: 1,
            width: 3,
            height: 2,
            clear_rgba: [13, 33, 99, 208],
            commands: vec![
                SceneCommand::UploadGlyph {
                    resource_id: "edge".into(),
                    glyph: GlyphBitmap {
                        width: 1,
                        height: 1,
                        format: GlyphBitmapFormat::Alpha8,
                        pixels: vec![128].into(),
                    },
                },
                SceneCommand::LinearGlyphRun {
                    id: "text".into(),
                    glyphs: [GlyphInstance {
                        resource_id: "edge".into(),
                        x: 1,
                        y: 0,
                        rotation_quadrants: 0,
                    }]
                    .into(),
                    rgba: [255; 4],
                    opacity: 1.0,
                },
                SceneCommand::rect("foreground", 1, 0, 2, 1, [7, 74, 206, 97]),
            ],
            semantics: None,
        };
        let expected_first = offscreen.render(&frame).unwrap();
        shared
            .render(&offscreen.device, &offscreen.queue, &frame, &mut first)
            .unwrap();

        frame.sequence += 1;
        frame.clear_rgba = [53, 16, 23, 255];
        frame.commands.remove(0);
        let expected_second = offscreen.render(&frame).unwrap();
        shared
            .render(&offscreen.device, &offscreen.queue, &frame, &mut second)
            .unwrap();

        // Both rows are narrower than GPU copy alignment, and the second pass
        // must still use the glyph uploaded in the first pass's atlas.
        let capture = |target: &WgpuSceneTarget| {
            let mut pixels = Arc::<[u8]>::new_uninit_slice(3 * 2 * 4);
            target
                .capture_into(
                    &offscreen.device,
                    &offscreen.queue,
                    Arc::get_mut(&mut pixels).unwrap(),
                )
                .unwrap();
            // SAFETY: Successful capture initialized every pixel.
            unsafe { pixels.assume_init() }
        };
        assert_eq!(capture(&first), expected_first.rgba8);
        assert_eq!(capture(&second), expected_second.rgba8);
        shared
            .render(&offscreen.device, &offscreen.queue, &frame, &mut first)
            .unwrap();
        assert_eq!(capture(&first), expected_second.rgba8);

        frame.sequence += 1;
        frame.commands.clear();
        frame.clear_rgba = [21, 83, 153, 79];
        shared
            .render(&offscreen.device, &offscreen.queue, &frame, &mut first)
            .unwrap();
        assert_eq!(&*capture(&first), frame.clear_rgba.repeat(3 * 2));
    }
}
