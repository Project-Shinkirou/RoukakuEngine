use super::*;
use roukaku_media::{
    AudioFramePacket, LateVideoPolicy, MediaPlaybackConfig, MediaPlaybackSession,
    MediaPlaybackState, MediaTrackKind, PlaybackTickRequest, VideoFramePacket,
};
use std::sync::Arc;

pub(super) struct VideoAudioSync {
    pub(super) voice_id: String,
    pub(super) duration_us: u64,
    pub(super) scheduler: MediaPlaybackSession,
    packet_sequence: u64,
    pub(super) clock: roukaku_audio_kira::AudioMasterClock,
}

impl VideoAudioSync {
    pub(super) fn due(
        &mut self,
        frame: &roukaku_media::DecodedVideoFrame,
        audio_us: u64,
    ) -> Result<Option<bool>, PlatformError> {
        if self.scheduler.video_queue.is_empty() {
            self.packet_sequence += 1;
            self.scheduler
                .queue_video(VideoFramePacket {
                    generation: self.scheduler.generation,
                    sequence: self.packet_sequence,
                    resource_id: "movie.video".into(),
                    pts_us: frame.pts_us,
                    duration_us: frame.duration_us,
                    width: frame.width,
                    height: frame.height,
                    color: Default::default(),
                })
                .map_err(|error| media_error("player.video.sync.queue", error))?;
        }
        if self.scheduler.state == MediaPlaybackState::Seeking {
            self.scheduler
                .complete_seek()
                .map_err(|error| media_error("player.video.sync.seek", error))?;
        }
        if matches!(
            self.scheduler.state,
            MediaPlaybackState::Ready | MediaPlaybackState::Paused
        ) {
            self.scheduler
                .play()
                .map_err(|error| media_error("player.video.sync.play", error))?;
        }
        let output = self
            .scheduler
            .tick(PlaybackTickRequest {
                sequence: self.scheduler.last_tick_sequence + 1,
                delta_us: 1,
                audio_playhead_us: Some(audio_us.max(self.scheduler.position_us)),
            })
            .map_err(|error| media_error("player.video.sync.tick", error))?;
        Ok(if output.presented_video.is_some() {
            Some(true)
        } else if !output.dropped_video.is_empty() {
            Some(false)
        } else {
            None
        })
    }

    pub(super) fn seek(&mut self, position_us: u64) -> Result<(), PlatformError> {
        let generation = self
            .scheduler
            .seek(position_us)
            .map_err(|error| media_error("player.video.sync.seek", error))?;
        self.packet_sequence = 0;
        self.clock.seek(position_us);
        self.scheduler
            .queue_audio(AudioFramePacket {
                generation,
                sequence: 1,
                resource_id: "movie.audio".into(),
                pts_us: 0,
                duration_us: self.duration_us,
                sample_rate: CANONICAL_SAMPLE_RATE,
                channels: CANONICAL_CHANNELS,
                frame_count: u32::try_from(
                    self.duration_us
                        .saturating_mul(u64::from(CANONICAL_SAMPLE_RATE))
                        .div_ceil(1_000_000),
                )
                .map_err(|_| {
                    media_error("player.video.sync.seek", "audio frame count overflowed")
                })?,
            })
            .map_err(|error| media_error("player.video.sync.seek", error))?;
        self.scheduler
            .mark_eos(MediaTrackKind::Audio, self.duration_us)
            .map_err(|error| media_error("player.video.sync.seek", error))
    }
}

impl NativeVnProductMediaHost {
    pub(super) async fn seek_video_audio(
        &mut self,
        source: &mut NativeVnHostCommandSource,
        executor: &mut PlayerHostCommandExecutor<PlatformCommandSink>,
        sync: &mut VideoAudioSync,
        position_us: u64,
    ) -> Result<(), PlatformError> {
        if position_us < sync.duration_us {
            self.audio
                .seek_movie(source, executor, &sync.voice_id, position_us)
                .await?;
        } else {
            self.audio.stop_movie(&sync.voice_id)?;
            self.audio.recover_device_loss(source, executor).await?;
        }
        sync.seek(position_us)
    }

    pub(super) async fn start_video_audio(
        &mut self,
        source: &mut NativeVnHostCommandSource,
        executor: &mut PlayerHostCommandExecutor<PlatformCommandSink>,
        request: &NativeVnVideoRequest,
    ) -> Result<Option<VideoAudioSync>, PlatformError> {
        let Some(audio_start_us) =
            roukaku_media::container_audio_start_us(&request.codec, request.encoded_bytes.clone())
                .map_err(|error| media_error("player.video.tracks", error))?
        else {
            return Ok(None);
        };
        let voice_id = format!("movie.{}", request.layer);
        let cached = self
            .decode_audio(
                source,
                executor,
                &NativeVnAudioPreloadRequest {
                    asset_id: request.asset_id.clone(),
                    codec: request.codec.clone(),
                    encoded_bytes: request.encoded_bytes.clone(),
                    encoded_length: request.encoded_length,
                },
            )
            .await?;
        if request.is_cancelled() {
            return Err(media_error(
                "player.video.audio",
                "movie request was cancelled",
            ));
        }
        let offset_frames =
            audio_start_us.saturating_mul(u64::from(CANONICAL_SAMPLE_RATE)) / 1_000_000;
        let frames = cached.asset.samples.len() / usize::from(CANONICAL_CHANNELS);
        let target_frames = (frames as u64).saturating_add(offset_frames);
        let target_samples = usize::try_from(target_frames)
            .ok()
            .and_then(|frames| frames.checked_mul(usize::from(CANONICAL_CHANNELS)))
            .filter(|samples| *samples <= NativeVnProductAudioHost::MAX_CONVERTED_SAMPLES)
            .ok_or_else(|| media_error("player.video.audio", "movie audio exceeds PCM budget"))?;
        let duration_us = target_frames * 1_000_000 / u64::from(CANONICAL_SAMPLE_RATE);
        let mut asset = cached.asset;
        if offset_frames > 0 {
            let mut samples = Vec::with_capacity(target_samples);
            samples.resize(
                offset_frames as usize * usize::from(CANONICAL_CHANNELS),
                0.0,
            );
            samples.extend_from_slice(&asset.samples);
            asset.samples = Arc::new(samples);
        }
        self.audio
            .start_canonical(
                source,
                executor,
                &crate::NativeVnAudioRequest {
                    command_id: voice_id.clone(),
                    target_id: voice_id.clone(),
                    command: "movie".into(),
                    attributes: BTreeMap::from([("loop".into(), "false".into())]),
                    asset_id: request.asset_id.clone(),
                    codec: request.codec.clone(),
                    encoded_bytes: request.encoded_bytes.clone(),
                    encoded_length: request.encoded_length,
                },
                asset,
                &mut self.completed_signals,
            )
            .await?;
        if request.is_cancelled() {
            self.audio.stop_movie(&voice_id)?;
            return Err(media_error(
                "player.video.audio",
                "movie request was cancelled",
            ));
        }
        let mut scheduler = MediaPlaybackSession::new(MediaPlaybackConfig {
            // EOS comes from the two tracks. Demux estimates never bound a streaming tail.
            duration_us: i64::MAX as u64,
            max_audio_clock_jump_us: i64::MAX as u64,
            max_video_frames: 1,
            late_video_policy: LateVideoPolicy::Drop,
            ..Default::default()
        })
        .map_err(|error| media_error("player.video.sync.create", error))?;
        scheduler
            .queue_audio(AudioFramePacket {
                generation: 1,
                sequence: 1,
                resource_id: "movie.audio".into(),
                pts_us: 0,
                duration_us,
                sample_rate: CANONICAL_SAMPLE_RATE,
                channels: CANONICAL_CHANNELS,
                frame_count: u32::try_from(target_frames).map_err(|_| {
                    media_error("player.video.audio", "movie frame count overflowed")
                })?,
            })
            .map_err(|error| media_error("player.video.sync.audio", error))?;
        scheduler
            .mark_eos(MediaTrackKind::Audio, duration_us)
            .map_err(|error| media_error("player.video.sync.eos", error))?;
        Ok(Some(VideoAudioSync {
            voice_id,
            duration_us,
            scheduler,
            packet_sequence: 0,
            clock: roukaku_audio_kira::AudioMasterClock::new(duration_us),
        }))
    }
}
