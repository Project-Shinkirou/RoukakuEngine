use std::io::Cursor;

use symphonia::core::{
    formats::{probe::Hint, FormatOptions, TrackType},
    io::MediaSourceStream,
    meta::MetadataOptions,
};

use crate::MediaError;

/// Demux metadata only. Decoding remains owned by the selected platform provider.
/// A malformed container is an error, never silently interpreted as a silent movie.
pub fn container_audio_start_us(
    codec: &str,
    bytes: roukaku_byte_source::OwnedByteBuffer,
) -> Result<Option<u64>, MediaError> {
    let mut hint = Hint::new();
    hint.with_extension(codec);
    let stream = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let format = symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|error| MediaError::message(format!("probe movie tracks: {error}")))?;
    let Some(track) = format.default_track(TrackType::Audio) else {
        return Ok(None);
    };
    let start = track
        .time_base
        .and_then(|time_base| time_base.calc_time(track.start_ts))
        .ok_or_else(|| MediaError::message("movie audio track has no usable time base"))?;
    let start_us = u64::try_from(start.as_micros().max(0))
        .map_err(|_| MediaError::message("movie audio start timestamp overflowed"))?;
    Ok(Some(start_us))
}
