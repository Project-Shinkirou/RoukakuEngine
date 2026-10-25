use super::*;

impl NativeVnProductMediaHost {
    /// Seek both tracks as one transport operation. The platform decoder is reopened and
    /// drained with bounded frame ownership; old device buffers are discarded before resume.
    pub async fn seek_video(
        &mut self,
        source: &mut NativeVnHostCommandSource,
        executor: &mut PlayerHostCommandExecutor<PlatformCommandSink>,
        layer: &str,
        position_us: u64,
    ) -> Result<(), PlatformError> {
        let index = self
            .active_videos
            .iter()
            .position(|video| video.request.layer == layer)
            .ok_or_else(|| media_error("player.video.seek", "video layer is not active"))?;
        if self.active_videos[index]
            .loop_duration_us
            .is_some_and(|duration| position_us >= duration)
        {
            return Err(media_error(
                "player.video.seek",
                "seek target is outside the video timeline",
            ));
        }
        let was_paused = self.clock.is_paused();
        if !was_paused {
            self.set_output_paused(true, executor).await?;
        }
        let old = self.active_videos.remove(index);
        if let Some(sync) = &old.audio_sync {
            self.audio.stop_movie(&sync.voice_id)?;
        }
        self.pending_video_closes.push(old.session);
        self.close_pending_video_streams(source, executor).await?;
        let result = async {
            let mut video = self
                .open_video_stream(source, executor, old.request, self.clock.time_ms())
                .await?;
            self.pending_video_closes.push(video.session);
            loop {
                let frame = Self::fetch_video_frame(
                    self.max_decode_output_bytes,
                    self.max_video_frames,
                    source,
                    executor,
                    &mut video,
                )
                .await?
                .ok_or_else(|| {
                    media_error(
                        "player.video.seek",
                        "seek target is outside the video timeline",
                    )
                })?;
                if frame.pts_us.saturating_add(frame.duration_us) > position_us {
                    video.pending_frame = Some(frame);
                    break;
                }
                video.next_frame = frame.sequence;
                video.presented_end_us = video.decoded_end_us;
            }
            if let Some(sync) = &mut video.audio_sync {
                self.seek_video_audio(source, executor, sync, position_us)
                    .await?;
            }
            video.seek_offset_us = position_us;
            self.pending_video_closes.retain(|id| *id != video.session);
            self.active_videos.push(video);
            Ok(())
        }
        .await;
        if result.is_err() {
            self.audio.stop_movie(&format!("movie.{layer}"))?;
            self.close_pending_video_streams(source, executor).await?;
        }
        if !was_paused {
            self.set_output_paused(false, executor).await?;
        }
        result
    }
}
