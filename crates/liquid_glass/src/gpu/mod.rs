//! WGPU instance, device, queue, and surface management.

pub mod context;
pub mod error;
pub mod surface;

#[cfg(test)]
pub mod tests;

pub use context::{GpuContext, PlatformBackendPolicy};
pub use error::{GpuError, SurfaceAcquisitionOutcome, SurfaceRecoveryAction};
pub use surface::{AcquiredFrame, SurfaceManager};
