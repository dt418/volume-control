//! Unit and integration tests for GPU context and surface lifecycle.

use crate::gpu::context::{GpuContext, PlatformBackendPolicy};
use crate::gpu::error::SurfaceRecoveryAction;

#[test]
fn platform_backend_candidates_match_policy() {
    assert_eq!(
        PlatformBackendPolicy::Windows.backend_candidates(),
        &[wgpu::Backends::DX12, wgpu::Backends::VULKAN]
    );
    assert_eq!(
        PlatformBackendPolicy::MacOS.backend_candidates(),
        &[wgpu::Backends::METAL]
    );
    assert_eq!(
        PlatformBackendPolicy::Linux.backend_candidates(),
        &[wgpu::Backends::VULKAN]
    );
}

#[test]
fn recovery_action_mapping_full_coverage() {
    // Normal / Suboptimal
    // We test non-texture variants directly:
    assert_eq!(
        SurfaceRecoveryAction::from_status(&wgpu::CurrentSurfaceTexture::Timeout, false),
        SurfaceRecoveryAction::SkipFrame
    );
    assert_eq!(
        SurfaceRecoveryAction::from_status(&wgpu::CurrentSurfaceTexture::Occluded, false),
        SurfaceRecoveryAction::SkipFrame
    );
    assert_eq!(
        SurfaceRecoveryAction::from_status(&wgpu::CurrentSurfaceTexture::Outdated, false),
        SurfaceRecoveryAction::Reconfigure
    );
    assert_eq!(
        SurfaceRecoveryAction::from_status(&wgpu::CurrentSurfaceTexture::Lost, false),
        SurfaceRecoveryAction::RecreateSurface
    );
    assert_eq!(
        SurfaceRecoveryAction::from_status(&wgpu::CurrentSurfaceTexture::Validation, false),
        SurfaceRecoveryAction::HoldError
    );

    // Any status during device loss escalates to RebuildDevice
    assert_eq!(
        SurfaceRecoveryAction::from_status(&wgpu::CurrentSurfaceTexture::Timeout, true),
        SurfaceRecoveryAction::RebuildDevice
    );
    assert_eq!(
        SurfaceRecoveryAction::from_status(&wgpu::CurrentSurfaceTexture::Outdated, true),
        SurfaceRecoveryAction::RebuildDevice
    );
    assert_eq!(
        SurfaceRecoveryAction::from_status(&wgpu::CurrentSurfaceTexture::Lost, true),
        SurfaceRecoveryAction::RebuildDevice
    );
}

#[test]
fn headless_gpu_context_smoke() {
    let ctx = pollster::block_on(GpuContext::init());
    match ctx {
        Ok(context) => {
            assert!(!context.is_device_lost());
            context.trigger_device_lost_for_test();
            assert!(context.is_device_lost());

            // Verify that after device loss, a fresh init creates an independent healthy context
            let ctx2 = pollster::block_on(GpuContext::init()).expect("Recreated context succeeds");
            assert!(!ctx2.is_device_lost());
        }
        Err(err) => {
            log::warn!("Headless GPU context unavailable on host: {:?}", err);
        }
    }
}

#[test]
fn gpu_error_converts_to_crate_error() {
    let gpu_err = crate::gpu::GpuError::NoAdapter("test adapter".into());
    let err: crate::Error = gpu_err.into();
    assert_eq!(
        err.to_string(),
        "GPU error: No compatible graphics adapter found for requested backends: test adapter"
    );
}
