use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, ErrorKind},
};

use roukaku_core::{Diagnostic, is_safe_symbol as safe_identity};

use schemars::JsonSchema;

use serde::{Deserialize, Serialize};

use symphonia::core::{
    codecs::audio::{AudioDecoder, AudioDecoderOptions, CODEC_ID_NULL_AUDIO},
    errors::Error as SymphoniaError,
    formats::{FormatOptions, FormatReader, TrackType, probe::Hint},
    io::MediaSourceStream,
    meta::MetadataOptions,
};

use crate::pcm_contract::{
    contains_non_finite_sample, is_aligned_pcm_sample_count, is_supported_pcm_channel_count,
    is_supported_pcm_rate,
};

use crate::MediaError;

#[cfg(feature = "ffmpeg-vcpkg")]
mod ffmpeg;

#[cfg(feature = "ffmpeg-vcpkg")]
mod ffmpeg_stream;

#[cfg(feature = "ffmpeg-vcpkg")]
pub use ffmpeg_stream::*;

#[cfg(windows)]
mod wmf_decode;

mod decode_contract;
pub use decode_contract::DecodeBindingContext;
pub use decode_contract::DecodeCapability;
pub use decode_contract::DecodeKind;
pub use decode_contract::DecodeOutput;
pub use decode_contract::DecodeProvider;
pub use decode_contract::DecodeProviderRegistry;
pub use decode_contract::DecodeRequest;
pub use decode_contract::DecodeResult;
use decode_contract::MAX_DECODED_AUDIO_BYTES;
#[cfg(any(windows, feature = "ffmpeg-vcpkg"))]
use decode_contract::MAX_DECODED_VIDEO_FRAME_BYTES;
pub use decode_contract::MediaSurfaceToken;
pub use decode_contract::ProviderPriority;
pub use decode_contract::SymphoniaAudioStreamDecoder;
pub use decode_contract::SymphoniaDecodedAudioChunk;
pub use decode_contract::WindowsAudioStreamDecoder;
pub use decode_contract::WindowsDecodedAudioChunk;
pub use decode_contract::WindowsVideoStreamDecoder;
#[cfg(windows)]
pub use decode_contract::decode_windows_video_stream;
#[cfg(not(windows))]
pub use decode_contract::decode_windows_video_stream;
pub use decode_contract::ffmpeg_compiled;
pub use decode_contract::open_symphonia_audio_stream;
pub use decode_contract::open_windows_audio_stream;
pub use decode_contract::open_windows_video_stream;
pub use decode_contract::probe_ffmpeg_provider;
mod decode_provider_registry;
pub use decode_provider_registry::ImageDecodeProvider;
pub use decode_provider_registry::SymphoniaAudioDecodeProvider;
pub use decode_provider_registry::SyntheticPlatformDecodeProvider;
#[cfg(windows)]
pub use decode_provider_registry::WindowsMediaFoundationDecodeProvider;
use decode_provider_registry::decode_error;
mod windows_media_foundation_decode_provider;
#[cfg(feature = "ffmpeg-vcpkg")]
pub use windows_media_foundation_decode_provider::FfmpegDecodeProvider;
#[cfg(target_arch = "wasm32")]
pub use windows_media_foundation_decode_provider::WebCodecsDecodeProvider;
#[cfg(target_arch = "wasm32")]
use windows_media_foundation_decode_provider::next_webcodecs_token;
#[cfg(target_arch = "wasm32")]
use windows_media_foundation_decode_provider::webcodecs_available;
