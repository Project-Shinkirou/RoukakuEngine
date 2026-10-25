mod allocation;
mod config;
mod crash;
mod perfetto;
mod pipeline;
mod process_memory;
mod record;
mod ring;
mod windows_crash;

pub use allocation::{
    AllocationSnapshot, ThreadAllocationSnapshot, TrackingAllocator, allocation_snapshot,
    thread_allocation_snapshot,
};
pub use config::{
    ConsoleFormat, CrashReportingMode, DEFAULT_MAX_ARCHIVES, DEFAULT_MAX_CRASH_BUNDLES,
    DEFAULT_MAX_FILE_BYTES, DEFAULT_RING_MAX_BYTES, DEFAULT_RING_MAX_RECORDS,
    HostObservabilityConfig, HostRole,
};
pub use crash::{CRASH_BUNDLE_SCHEMA, CrashArtifactRef, CrashBundleManifestV1};
pub use perfetto::{
    DEFAULT_PERFETTO_MAX_BYTES, DEFAULT_PERFETTO_MAX_EVENTS, PerfettoFlowPhase,
    PerfettoTraceConfig, PerfettoTraceError, PerfettoTraceSummary, PerfettoTraceWriter,
};
pub use pipeline::{ObservabilityError, ObservabilityGuard, init_host};
pub use process_memory::{
    ProcessMemoryError, ProcessMemorySample, sample_process_cpu_time_us_by_pid,
    sample_process_memory, sample_process_memory_by_pid,
};
pub use record::{LOG_EVENT_SCHEMA, LogEventV1, SpanContextV1};
pub use windows_crash::{
    WindowsCrashReporterConfig, WindowsCrashReporterGuard, install_windows_crash_reporter,
};
