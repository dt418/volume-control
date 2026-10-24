//! WGPU context initialization and deterministic platform backend negotiation.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::gpu::error::GpuError;

/// Target platform backend preference policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformBackendPolicy {
    /// Windows: Attempt DX12 first, fallback to Vulkan.
    Windows,
    /// macOS: Metal exclusively.
    MacOS,
    /// Linux: Vulkan exclusively.
    Linux,
    /// Custom explicit backend list.
    Custom(&'static [wgpu::Backends]),
}

impl PlatformBackendPolicy {
    /// Return the active policy for the current compilation target.
    pub const fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOS
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Custom(&[wgpu::Backends::PRIMARY])
        }
    }

    /// List backend candidates in strict descending preference order.
    pub fn backend_candidates(&self) -> &'static [wgpu::Backends] {
        match self {
            Self::Windows => &[wgpu::Backends::DX12, wgpu::Backends::VULKAN],
            Self::MacOS => &[wgpu::Backends::METAL],
            Self::Linux => &[wgpu::Backends::VULKAN],
            Self::Custom(list) => list,
        }
    }
}

/// Shared graphics device, command queue, adapter, and instance.
pub struct GpuContext {
    instance: Arc<wgpu::Instance>,
    adapter: Arc<wgpu::Adapter>,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    backend: wgpu::Backend,
    device_lost: Arc<AtomicBool>,
}

impl GpuContext {
    /// Initialize a GPU context using the platform backend policy.
    pub async fn init() -> Result<Self, GpuError> {
        Self::init_with_policy(PlatformBackendPolicy::current()).await
    }

    /// Initialize a GPU context with an explicit backend policy.
    pub async fn init_with_policy(policy: PlatformBackendPolicy) -> Result<Self, GpuError> {
        let candidates = policy.backend_candidates();
        let mut last_error = None;

        for &backends in candidates {
            log::info!(
                "Attempting GPU initialization with backends: {:?}",
                backends
            );
            let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
            desc.backends = backends;
            let instance = Arc::new(wgpu::Instance::new(desc));

            let adapter_options = wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            };
            let adapter = match instance.request_adapter(&adapter_options).await {
                Ok(adapter) => Arc::new(adapter),
                Err(err) => {
                    log::warn!("Adapter request failed for {:?}: {:?}", backends, err);
                    last_error = Some(GpuError::NoAdapter(format!("{:?}: {:?}", backends, err)));
                    continue;
                }
            };

            let info = adapter.get_info();
            log::info!("Acquired adapter: {} ({:?})", info.name, info.backend);

            let (device, queue) = match adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("liquid_glass_device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                    ..Default::default()
                })
                .await
            {
                Ok((d, q)) => (Arc::new(d), Arc::new(q)),
                Err(err) => {
                    log::warn!("Device request failed for {:?}: {:?}", backends, err);
                    last_error = Some(GpuError::RequestDevice(format!("{:?}", err)));
                    continue;
                }
            };

            let device_lost = Arc::new(AtomicBool::new(false));
            let device_lost_flag = Arc::clone(&device_lost);

            device.set_device_lost_callback(move |reason, msg| {
                log::error!("WGPU Device Lost! Reason: {:?}, Message: {}", reason, msg);
                device_lost_flag.store(true, Ordering::SeqCst);
            });

            return Ok(Self {
                instance,
                adapter,
                device,
                queue,
                backend: info.backend,
                device_lost,
            });
        }

        Err(last_error.unwrap_or_else(|| {
            GpuError::NoAdapter("No supported GPU backend candidates succeeded".into())
        }))
    }

    /// True if the underlying device was lost and must be recreated.
    pub fn is_device_lost(&self) -> bool {
        self.device_lost.load(Ordering::SeqCst)
    }

    /// Manually trigger device loss state (used by test harnesses).
    pub fn trigger_device_lost_for_test(&self) {
        self.device_lost.store(true, Ordering::SeqCst);
    }

    pub fn instance(&self) -> &Arc<wgpu::Instance> {
        &self.instance
    }

    pub fn adapter(&self) -> &Arc<wgpu::Adapter> {
        &self.adapter
    }

    pub fn device(&self) -> &Arc<wgpu::Device> {
        &self.device
    }

    pub fn queue(&self) -> &Arc<wgpu::Queue> {
        &self.queue
    }

    pub fn backend(&self) -> wgpu::Backend {
        self.backend
    }

    /// Create a new surface bound to this context's WGPU instance.
    pub fn create_surface<'a>(
        &self,
        target: impl Into<wgpu::SurfaceTarget<'a>>,
    ) -> Result<wgpu::Surface<'a>, GpuError> {
        self.instance
            .create_surface(target)
            .map_err(|err| GpuError::SurfaceCreation(err.to_string()))
    }
}
