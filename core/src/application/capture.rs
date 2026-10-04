//! Desktop-independent capture use cases.
pub use crate::capture::{CaptureDelta, CaptureEnvironment, CaptureSession, CaptureSnapshot, CapturedFlow, FlowSummary, StartCapture};
pub fn environment() -> CaptureEnvironment { crate::capture::environment() }
pub fn sessions() -> Result<Vec<CaptureSession>, String> { crate::capture::sessions() }
pub fn snapshot(id: &str) -> Result<CaptureSnapshot, String> { crate::capture::snapshot(id) }
pub fn detail(session: &str, flow: &str) -> Result<CapturedFlow, String> { crate::capture::detail(session, flow) }
pub fn start(request: StartCapture) -> Result<CaptureSnapshot, String> { super::gate::write_independent(|| crate::capture::start(request)) }
pub fn stop(id: &str) -> Result<CaptureSnapshot, String> { crate::capture::stop(id) }
pub fn export(session: &str, flow: Option<&str>, path: &std::path::Path) -> Result<(), String> { crate::capture::export(session, flow, path) }

pub fn summaries(id: &str) -> Result<Vec<FlowSummary>, String> { crate::capture::summaries(id) }
pub fn delta(id: &str, revision: Option<&str>) -> Result<CaptureDelta, String> { crate::capture::delta(id, revision) }
