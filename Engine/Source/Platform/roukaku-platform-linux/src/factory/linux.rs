use super::package_completion::{PackageCompletion, PackageSourceResource, deliver_package_result};

use super::package_worker::{PackageWorker, cancellable_fetch};

use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc as std_mpsc,
    },
    thread,
    time::Duration,
};

use crate::decode::DecodeSession;

use roukaku_platform::{
    AudioDeviceFormat, AudioOutputHandle, AudioOutputRequest, AudioWakeRegistration, CapturedFrame,
    DecodeSessionHandle, HostCommand, HostLaunchProfile, InputState, OpenedAudioOutput,
    PackageSourceHandle, PackageSourceRequest, PlatformBackendChannels,
    PlatformCommandWakeRegistration, PlatformError, PlatformErrorCode, PlatformEvent,
    PlatformEventKind, PlatformHostProfile, PlatformHostSession, PointerButton,
    SaveTransactionHandle, SurfaceHandle, TouchPhase, WindowHandle, host_channel_with_command_wake,
};

use roukaku_platform_common::{
    AtomicSaveStore, CachedPackageSource, FilePackageSource, ResourceTable, SaveTransaction,
    VerifiedPackageCache,
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use tokio::sync::oneshot;

use winit::{
    application::ApplicationHandler,
    event::{
        ElementState, Ime, MouseButton, MouseScrollDelta, TouchPhase as WinitTouchPhase,
        WindowEvent,
    },
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    platform::wayland::EventLoopBuilderExtWayland,
    window::{Window, WindowAttributes, WindowId},
};

mod start;
use start::LinuxHostApp;
use start::PackageHostConfig;
pub use start::start;
mod new;

mod open_user_authorized_package;
use open_user_authorized_package::AudioResource;
use open_user_authorized_package::SurfaceResource;
use open_user_authorized_package::capture_surface;
use open_user_authorized_package::create_surface;
use open_user_authorized_package::linux_gamepad_worker;
mod audio_resource;
use audio_resource::default_roots;
use audio_resource::host_error;
