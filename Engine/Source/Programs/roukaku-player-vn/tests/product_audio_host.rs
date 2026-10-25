use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use roukaku_platform::{
    AudioDeviceFormat, AudioOutputHandle, AudioOutputLane, DecodeKind, DecodeOutput,
    DecodeSessionHandle, HostCommand, OpenedAudioOutput, PlatformError, PlatformHostProfile,
    SurfaceHandle, host_channel,
};

use roukaku_player_core::{
    PlatformCommandSink, PlayerDecodedAudio, PlayerHostCommandExecutor, PlayerHostResourceId,
};

use roukaku_player_vn::{
    NativeVnAudioControlRequest, NativeVnAudioRequest, NativeVnHostCommandSource,
    NativeVnProductAudioHost, NativeVnProductMediaHost,
};

mod support;

#[path = "product_audio_host/fixtures.rs"]
mod fixtures;
use fixtures::ProductAudioTestLane;
use fixtures::assert_endpoint_consumed_is_monotonic;
use fixtures::source;
#[path = "product_audio_host/video_restore.rs"]
mod video_restore;
