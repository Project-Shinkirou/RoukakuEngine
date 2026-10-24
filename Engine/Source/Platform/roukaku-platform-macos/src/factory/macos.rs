#[cfg(target_os = "macos")]
use std::time::Duration;
use std::{
    collections::BTreeMap,
    future::Future,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc as std_mpsc, Arc,
    },
    thread,
};

use crate::accessibility::MacosAccessibilityBridge;

use roukaku_media::{DecodeOutput as MediaDecodeOutput, DecodeProvider};

use roukaku_platform::{
    host_channel_with_command_wake, AudioDeviceFormat, AudioOutputHandle, AudioOutputRequest,
    AudioWakeRegistration, CapturedFrame, DecodeKind, DecodeOutput, DecodeSessionHandle,
    HostCommand, HostLaunchProfile, InputState, OpenedAudioOutput, PackageSourceHandle,
    PackageSourceRequest, PlatformBackendChannels, PlatformCommandWakeRegistration,
    PlatformDecodeRequest, PlatformError, PlatformErrorCode, PlatformEvent, PlatformEventKind,
    PlatformHostProfile, PlatformHostSession, PointerButton, SaveTransactionHandle, SurfaceHandle,
    TouchPhase, WindowHandle,
};

use roukaku_platform_common::{
    AtomicSaveStore, CachedPackageSource, FilePackageSource, ResourceTable, SaveTransaction,
    VerifiedPackageCache,
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use tokio::sync::oneshot;

#[cfg(target_os = "macos")]
use winit::platform::pump_events::EventLoopExtPumpEvents;
use winit::{
    application::ApplicationHandler,
    event::{
        ElementState, Ime, MouseButton, MouseScrollDelta, TouchPhase as WinitTouchPhase,
        WindowEvent,
    },
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::{Window, WindowAttributes, WindowId},
};

mod backend_setup;
#[cfg(target_os = "ios")]
pub mod ios;
pub(super) use backend_setup::start;
pub(super) use backend_setup::BackendSetup;
use backend_setup::MacosHostApp;
#[cfg(target_os = "macos")]
pub use backend_setup::MacosHostRunner;
use backend_setup::PackageHostConfig;
mod new;

mod process_commands;

mod macos_host_app;
use macos_host_app::capture_surface;
use macos_host_app::create_surface;
#[cfg(target_os = "macos")]
use macos_host_app::raw_gamepad_event;
use macos_host_app::AudioResource;
use macos_host_app::PackageCompletion;
use macos_host_app::PackageSourceResource;
use macos_host_app::SurfaceResource;
mod audio_resource;
use audio_resource::DecodeResource;
mod decode_avfoundation;
use decode_avfoundation::decode_avfoundation;
use decode_avfoundation::default_roots;
use decode_avfoundation::host_error;
use decode_avfoundation::media_decode_error;
