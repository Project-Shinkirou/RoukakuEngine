use std::{
    collections::BTreeMap,
    sync::{Arc, mpsc as std_mpsc},
    thread,
};

use android_activity::AndroidApp;

use roukaku_core::Hash256;

use roukaku_platform::{
    AudioOutputHandle, CapturedFrame, DecodeSessionHandle, HostCommand, HostLaunchProfile,
    InputState, MemoryPressureLevel, OpenedAudioOutput, PackageCachePolicy, PackageSourceHandle,
    PackageSourcePolicy, PackageSourceRequest, PlatformBackendChannels,
    PlatformCommandWakeRegistration, PlatformError, PlatformErrorCode, PlatformEvent,
    PlatformEventKind, PlatformHostSession, PointerButton, SaveTransactionHandle, SurfaceHandle,
    TouchPhase, WindowHandle, host_channel_with_command_wake,
};

use roukaku_platform_common::{
    AtomicSaveStore, CachedPackageSource, ResourceTable, SaveTransaction, VerifiedPackageCache,
};

use winit::{
    application::ApplicationHandler,
    event::{
        ElementState, Ime, MouseButton, MouseScrollDelta, TouchPhase as WinitTouchPhase,
        WindowEvent,
    },
    event_loop::{ActiveEventLoop, ControlFlow},
    window::{Window, WindowAttributes, WindowId},
};

use crate::accessibility::AndroidAccessibilityBridge;

use crate::audio::AndroidAudioResource;

use crate::decode::AndroidDecodeWorker;

#[path = "native_host/surface_core.rs"]
mod surface_core;
pub(super) use surface_core::AndroidHostApp;
use surface_core::MemoryPackageSource;
use surface_core::PackageCompletion;
use surface_core::PackageSourceResource;
use surface_core::SurfaceCore;
use surface_core::SurfaceSlot;
pub(super) use surface_core::create_host;
#[path = "native_host/process_command.rs"]
mod process_command;

#[path = "native_host/android_host_app.rs"]
mod android_host_app;
use android_host_app::capture_surface;
use android_host_app::host_error;
