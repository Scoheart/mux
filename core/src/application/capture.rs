//! Desktop-independent capture use cases.
pub use crate::capture::{CaptureEnvironment, CaptureSession, CaptureSnapshot, CapturedFlow, StartCapture};
pub fn environment() -> CaptureEnvironment { crate::capture::environment() }
pub fn sessions() -> Result<Vec<CaptureSession>, String> { crate::capture::sessions() }
pub fn snapshot(id: &str) -> Result<CaptureSnapshot, String> { crate::capture::snapshot(id) }
pub fn detail(session: &str, flow: &str) -> Result<CapturedFlow, String> { crate::capture::detail(session, flow) }
pub fn start(request: StartCapture) -> Result<CaptureSnapshot, String> { super::gate::write_independent(|| crate::capture::start(request)) }
pub fn stop(id: &str) -> Result<CaptureSnapshot, String> { crate::capture::stop(id) }
pub fn export(session: &str, flow: Option<&str>, path: &std::path::Path) -> Result<(), String> { crate::capture::export(session, flow, path) }
