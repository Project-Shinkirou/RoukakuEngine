#![cfg(windows)]

use roukaku_audio_kira::{
    AudioAssetRevision, AudioMasterClock, AudioServiceCommand, AudioServiceConfig,
    AudioServiceSession, RoukakuChunkBackendSettings,
};
use roukaku_media::{
    AudioFramePacket, MediaPlaybackConfig, MediaPlaybackSession, PlaybackTickRequest,
    VideoFramePacket,
};
use roukaku_platform::AudioWakeRegistration;
use roukaku_platform_common::{NativeAudioConsumer, NativeAudioQueue};
use std::{sync::Arc, time::Duration};

fn mixer(pcm: Arc<Vec<f32>>, start_frame: u64) -> (AudioServiceSession, NativeAudioConsumer) {
    let (producer, consumer, _) =
        NativeAudioQueue::create(4, 800, AudioWakeRegistration::default()).unwrap();
    let mut service = AudioServiceSession::new(
        AudioServiceConfig {
            max_voices: 4,
            max_buses: 4,
            max_events: 32,
            pcm_cache_bytes: 4 * 1024 * 1024,
        },
        RoukakuChunkBackendSettings {
            sample_rate: 48_000,
            channels: 2,
            chunk_frames: 400,
            endpoint: Box::new(producer),
            deterministic_fixed_tick_hz: Some(60),
        },
    )
    .unwrap();
    let asset = AudioAssetRevision {
        package_id: "sample".into(),
        uri: "movie.audio".into(),
        revision: "generated".into(),
        byte_len: pcm.len() as u64 * 4,
    };
    service
        .prepare_pcm_shared(asset.clone(), 48_000, 2, pcm)
        .unwrap();
    service
        .apply(AudioServiceCommand::Play {
            voice_id: "movie".into(),
            bus: "movie".into(),
            asset,
            start_frame,
            looping: false,
        })
        .unwrap();
    (service, consumer)
}

fn queue_audio(scheduler: &mut MediaPlaybackSession, frames: u32, duration_us: u64) {
    scheduler
        .queue_audio(AudioFramePacket {
            generation: scheduler.generation,
            sequence: 1,
            resource_id: "movie.audio".into(),
            pts_us: 0,
            duration_us,
            sample_rate: 48_000,
            channels: 2,
            frame_count: frames,
        })
        .unwrap();
}

#[test]
fn wmf_movie_pts_follow_consumed_kira_audio_across_pause_seek_and_rebuild() {
    // Generated testsrc2 + sine; no downloaded media and no FFmpeg runtime dependency.
    let encoded = include_bytes!("../../roukaku-media/tests/fixtures/av-clock.mp4");
    let mut audio = roukaku_media::open_windows_audio_stream(encoded, 4 * 1024 * 1024).unwrap();
    let mut samples = Vec::new();
    while let Some(chunk) = audio.next_chunk().unwrap() {
        assert_eq!((chunk.sample_rate, chunk.channels), (48_000, 2));
        samples.extend(
            chunk
                .samples
                .into_iter()
                .map(|sample| f32::from(sample) / 32768.0),
        );
    }
    let mut decoder =
        roukaku_media::open_windows_video_stream(encoded, 100, 2 * 1024 * 1024).unwrap();
    let mut video = Vec::new();
    while let Some(frame) = decoder.next_frame().unwrap() {
        video.push(frame);
    }
    assert_eq!(video.len(), 75);
    assert!(samples.iter().any(|sample| sample.abs() > 0.01));
    let frames = samples.len() as u32 / 2;
    let duration_us = u64::from(frames) * 1_000_000 / 48_000;
    let video_end_us = video
        .last()
        .map(|frame| frame.pts_us + frame.duration_us)
        .unwrap();
    assert!(duration_us < video_end_us);
    let pcm = Arc::new(samples);
    let config = MediaPlaybackConfig {
        duration_us: duration_us.max(video_end_us),
        max_video_frames: 100,
        ..Default::default()
    };
    let threshold = config.max_video_lag_us;
    let lead = config.max_video_lead_us;
    let mut scheduler = MediaPlaybackSession::new(config).unwrap();
    queue_audio(&mut scheduler, frames, duration_us);
    for frame in &video {
        scheduler
            .queue_video(VideoFramePacket {
                generation: 1,
                sequence: frame.sequence,
                resource_id: "movie.video".into(),
                pts_us: frame.pts_us,
                duration_us: frame.duration_us,
                width: frame.width,
                height: frame.height,
                color: Default::default(),
            })
            .unwrap();
    }
    scheduler.play().unwrap();
    let (mut service, mut consumer) = mixer(pcm.clone(), 0);
    let mut completed_timeline = service.audible_timeline();
    let mut clock = AudioMasterClock::new(duration_us);
    let mut output_epoch = 1;
    let mut energy = 0.0_f64;
    let mut presented = 0;
    let mut maximum_drift = 0_u64;
    for step in 0..100 {
        if step == 20 {
            scheduler.pause().unwrap();
            let stopped = service
                .playback_progress("movie")
                .voice_position_us
                .unwrap();
            std::thread::sleep(Duration::from_millis(25));
            assert_eq!(
                service.playback_progress("movie").voice_position_us,
                Some(stopped)
            );
            scheduler.play().unwrap();
        }
        if step == 45 {
            // Same transport operation as Player: discard the old endpoint buffers,
            // recreate Kira at the target, and invalidate the scheduler generation.
            let target = 1_400_000;
            let generation = scheduler.seek(target).unwrap();
            let replacement = mixer(pcm.clone(), target * 48_000 / 1_000_000);
            service = replacement.0;
            consumer = replacement.1;
            output_epoch += 1;
            clock.seek(target);
            queue_audio(&mut scheduler, frames, duration_us);
            for (index, frame) in video
                .iter()
                .filter(|frame| frame.pts_us >= target)
                .enumerate()
            {
                scheduler
                    .queue_video(VideoFramePacket {
                        generation,
                        sequence: index as u64 + 1,
                        resource_id: "movie.video".into(),
                        pts_us: frame.pts_us,
                        duration_us: frame.duration_us,
                        width: frame.width,
                        height: frame.height,
                        color: Default::default(),
                    })
                    .unwrap();
            }
            scheduler.complete_seek().unwrap();
        }
        if step == 60 {
            let timeline = service.audible_timeline();
            let replacement = mixer(pcm.clone(), timeline.voices["movie"].cursor_frames);
            service = replacement.0;
            consumer = replacement.1;
            output_epoch += 1;
        }
        if step == 90 {
            assert!(service
                .playback_progress("movie")
                .voice_position_us
                .is_none());
            let mut replacement = mixer(pcm.clone(), 0);
            replacement
                .0
                .apply(AudioServiceCommand::Stop {
                    voice_id: "movie".into(),
                })
                .unwrap();
            service = replacement.0;
            consumer = replacement.1;
            output_epoch += 1;
        }
        service.poll_fixed_tick().unwrap();
        let mut output = [0.0_f32; 1600];
        consumer.record_output_latency(Duration::from_millis(2), 48_000, 2, output.len());
        assert!(!consumer.fill_output_f32(&mut output));
        energy += output
            .iter()
            .map(|sample| f64::from(*sample).powi(2))
            .sum::<f64>();
        let position = clock
            .update(service.playback_progress("movie"), output_epoch)
            .min(scheduler.config.duration_us);
        let tick = scheduler
            .tick(PlaybackTickRequest {
                sequence: scheduler.last_tick_sequence + 1,
                delta_us: 16_667,
                audio_playhead_us: Some(position),
            })
            .unwrap();
        if let Some(drift) = tick.av_drift_us {
            assert!(drift <= lead as i64 && drift >= -(threshold as i64));
            maximum_drift = maximum_drift.max(drift.unsigned_abs());
            presented += 1;
        }
    }
    scheduler
        .mark_eos(roukaku_media::MediaTrackKind::Audio, duration_us)
        .unwrap();
    scheduler
        .mark_eos(
            roukaku_media::MediaTrackKind::Video,
            video
                .last()
                .map(|frame| frame.pts_us + frame.duration_us)
                .unwrap(),
        )
        .unwrap();
    for _ in 0..100 {
        if scheduler.state == roukaku_media::MediaPlaybackState::Ended {
            break;
        }
        service.poll_fixed_tick().unwrap();
        let mut output = [0.0_f32; 1600];
        consumer.record_output_latency(Duration::from_millis(2), 48_000, 2, output.len());
        consumer.fill_output_f32(&mut output);
        let position = clock
            .update(service.playback_progress("movie"), output_epoch)
            .min(scheduler.config.duration_us);
        let tick = scheduler
            .tick(PlaybackTickRequest {
                sequence: scheduler.last_tick_sequence + 1,
                delta_us: 16_667,
                audio_playhead_us: Some(position),
            })
            .unwrap();
        if let Some(drift) = tick.av_drift_us {
            assert!(drift <= lead as i64 && drift >= -(threshold as i64));
            maximum_drift = maximum_drift.max(drift.unsigned_abs());
            presented += 1;
        }
    }
    assert_eq!(scheduler.state, roukaku_media::MediaPlaybackState::Ended);
    assert!(presented > 30 && energy > 1.0);
    completed_timeline
        .voices
        .get_mut("movie")
        .unwrap()
        .cursor_frames = u64::from(frames);
    service.restore_timeline(completed_timeline).unwrap();
    assert_eq!(service.active_voice_count(), 0);
    assert_eq!(service.take_events().into_iter().filter(|event|
        matches!(event, roukaku_audio_kira::AudioServiceEvent::VoiceCompleted { voice_id, .. } if voice_id == "movie")
    ).count(), 1);
    eprintln!("WMF/Kira sample: presented={presented}, max PTS/audio drift={maximum_drift}us, threshold={threshold}us");
}
