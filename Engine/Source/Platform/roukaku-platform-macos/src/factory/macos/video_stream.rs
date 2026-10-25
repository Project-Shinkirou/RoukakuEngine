use super::*;
use std::io::Write;

use objc2::{rc::Retained, runtime::AnyObject};
use objc2_av_foundation::{
    AVAssetReader, AVAssetReaderStatus, AVAssetReaderTrackOutput, AVMediaTypeVideo, AVURLAsset,
};
use objc2_core_media::{CMSampleBuffer, CMTime, CMTimeRange};
use objc2_core_video::{
    CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetHeight,
    CVPixelBufferGetWidth, CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags,
    CVPixelBufferUnlockBaseAddress,
};
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};

/// Pull-driven AVAssetReader with one look-ahead sample. Next transfers one frame
/// to the Player; its single pending frame supplies bounded backpressure.
pub(super) struct VideoStream {
    _input: tempfile::NamedTempFile,
    asset: Retained<AVURLAsset>,
    reading: Option<(Retained<AVAssetReader>, Retained<AVAssetReaderTrackOutput>)>,
    pending: Option<Retained<CMSampleBuffer>>,
    duration_us: Option<u64>,
    frame_count: u64,
    byte_count: u64,
    ended: bool,
}

#[allow(deprecated)]
impl VideoStream {
    pub(super) fn open(request: &PlatformDecodeRequest) -> Result<Self, PlatformError> {
        let mut input = tempfile::Builder::new()
            .suffix(&format!(".{}", request.codec))
            .tempfile()
            .map_err(|_| host_error("decode.stream.start", "temporary video input failed"))?;
        input
            .write_all(&request.bytes)
            .and_then(|()| input.flush())
            .map_err(|_| host_error("decode.stream.start", "temporary video write failed"))?;
        let path = input
            .path()
            .to_str()
            .ok_or_else(|| host_error("decode.stream.start", "video path is not UTF-8"))?;
        unsafe {
            let url = NSURL::fileURLWithPath(&NSString::from_str(path));
            let asset = AVURLAsset::URLAssetWithURL_options(&url, None);
            let duration_us = time_us(asset.duration());
            let mut stream = Self {
                _input: input,
                asset,
                reading: None,
                pending: None,
                duration_us,
                frame_count: 0,
                byte_count: 0,
                ended: false,
            };
            stream.seek(0)?;
            Ok(stream)
        }
    }

    unsafe fn reader(
        asset: &AVURLAsset,
        position_us: u64,
    ) -> Result<(Retained<AVAssetReader>, Retained<AVAssetReaderTrackOutput>), PlatformError> {
        unsafe {
            let media_type = AVMediaTypeVideo
                .ok_or_else(|| host_error("decode.stream.start", "video media type unavailable"))?;
            let track = asset
                .tracksWithMediaType(media_type)
                .firstObject()
                .ok_or_else(|| host_error("decode.stream.start", "container has no video track"))?;
            let settings = NSDictionary::from_slices(
                &[&*NSString::from_str("PixelFormatType")],
                &[&*NSNumber::numberWithUnsignedInt(u32::from_be_bytes(
                    *b"BGRA",
                ))],
            );
            let output = AVAssetReaderTrackOutput::assetReaderTrackOutputWithTrack_outputSettings(
                &track,
                Some(settings.cast_unchecked::<NSString, AnyObject>()),
            );
            output.setAlwaysCopiesSampleData(false);
            let reader = AVAssetReader::assetReaderWithAsset_error(asset)
                .map_err(|_| host_error("decode.stream.start", "AVAssetReader open failed"))?;
            if !reader.canAddOutput(&output) {
                return Err(host_error("decode.stream.start", "video output rejected"));
            }
            reader.addOutput(&output);
            if position_us > 0 {
                let start = CMTime::new(
                    i64::try_from(position_us).map_err(|_| {
                        host_error("decode.stream.seek", "seek timestamp overflowed")
                    })?,
                    1_000_000,
                );
                reader.setTimeRange(CMTimeRange {
                    start,
                    duration: asset.duration().subtract(start),
                });
            }
            if !reader.startReading() {
                return Err(host_error(
                    "decode.stream.start",
                    "AVAssetReader start failed",
                ));
            }
            Ok((reader, output))
        }
    }

    pub(super) fn descriptor(&self) -> DecodeOutput {
        DecodeOutput::VideoStreamStart {
            duration_estimate_us: self.duration_us,
            frame_count: None,
            decoded_byte_count: None,
        }
    }

    /// AVAssetReader cannot rewind: replace it, retaining the owned temporary input.
    pub(super) fn seek(&mut self, position_us: u64) -> Result<(), PlatformError> {
        let (reader, output) = unsafe { Self::reader(&self.asset, position_us)? };
        if let Some((previous, _)) = self.reading.take() {
            unsafe {
                previous.cancelReading();
            }
        }
        self.reading = Some((reader, output));
        self.pending = None;
        self.frame_count = 0;
        self.byte_count = 0;
        self.ended = false;
        Ok(())
    }

    pub(super) fn next(&mut self) -> Result<DecodeOutput, PlatformError> {
        if self.ended {
            return Err(host_error(
                "decode.stream.next",
                "video EOS already delivered",
            ));
        }
        unsafe {
            let (reader, output) = self.reading.as_ref().expect("reader is initialized");
            let sample = self
                .pending
                .take()
                .or_else(|| output.copyNextSampleBuffer());
            let Some(sample) = sample else {
                if reader.status() != AVAssetReaderStatus::Completed {
                    return Err(host_error(
                        "decode.stream.next",
                        "AVAssetReader failed before EOS",
                    ));
                }
                self.ended = true;
                return Ok(DecodeOutput::VideoStreamEnd {
                    frame_count: self.frame_count,
                    decoded_byte_count: self.byte_count,
                });
            };
            let pts_us = time_us(sample.presentation_time_stamp())
                .ok_or_else(|| host_error("decode.stream.next", "invalid video PTS"))?;
            self.pending = output.copyNextSampleBuffer();
            let duration_us = self
                .pending
                .as_ref()
                .and_then(|next| time_us(next.presentation_time_stamp()))
                .and_then(|next| next.checked_sub(pts_us))
                .filter(|duration| *duration > 0)
                .or_else(|| time_us(sample.duration()).filter(|duration| *duration > 0))
                .ok_or_else(|| host_error("decode.stream.next", "invalid video duration"))?;
            let image = sample
                .image_buffer()
                .ok_or_else(|| host_error("decode.stream.next", "video sample has no image"))?;
            let width = CVPixelBufferGetWidth(&image);
            let height = CVPixelBufferGetHeight(&image);
            let length = width
                .checked_mul(height)
                .and_then(|pixels| pixels.checked_mul(4))
                .filter(|bytes| *bytes <= 64 * 1024 * 1024)
                .ok_or_else(|| {
                    host_error("decode.stream.next", "video frame exceeds byte budget")
                })?;
            let width_u32 = u32::try_from(width)
                .map_err(|_| host_error("decode.stream.next", "video width overflowed"))?;
            let height_u32 = u32::try_from(height)
                .map_err(|_| host_error("decode.stream.next", "video height overflowed"))?;
            let flags = CVPixelBufferLockFlags(1);
            if CVPixelBufferLockBaseAddress(&image, flags) != 0 {
                return Err(host_error("decode.stream.next", "video buffer lock failed"));
            }
            let base = CVPixelBufferGetBaseAddress(&image).cast::<u8>();
            let stride = CVPixelBufferGetBytesPerRow(&image);
            let mut bytes = Vec::with_capacity(length);
            if !base.is_null() && stride >= width * 4 {
                for row in 0..height {
                    bytes.extend_from_slice(std::slice::from_raw_parts(
                        base.add(row * stride),
                        width * 4,
                    ));
                }
            }
            let _ = CVPixelBufferUnlockBaseAddress(&image, flags);
            if bytes.len() != length || length == 0 {
                return Err(host_error(
                    "decode.stream.next",
                    "invalid video buffer layout",
                ));
            }
            self.frame_count += 1;
            self.byte_count = self
                .byte_count
                .checked_add(length as u64)
                .ok_or_else(|| host_error("decode.stream.next", "video byte count overflowed"))?;
            Ok(DecodeOutput::VideoFrame {
                sequence: self.frame_count,
                pts_us,
                duration_us,
                width: width_u32,
                height: height_u32,
                bgra8: bytes.into(),
            })
        }
    }
}

impl Drop for VideoStream {
    fn drop(&mut self) {
        if let Some((reader, _)) = self.reading.take() {
            unsafe {
                reader.cancelReading();
            }
        }
    }
}

fn time_us(time: CMTime) -> Option<u64> {
    let seconds = unsafe { time.seconds() };
    (seconds.is_finite() && seconds >= 0.0 && seconds <= u64::MAX as f64 / 1_000_000.0)
        .then(|| (seconds * 1_000_000.0).round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_reader_stream_preserves_pts_seeks_and_cancels() {
        let request = PlatformDecodeRequest {
            sequence: 1,
            kind: DecodeKind::Video,
            codec: "mp4".into(),
            description: Vec::new(),
            sample_rate: None,
            channels: None,
            coded_width: None,
            coded_height: None,
            keyframe: true,
            stream_action: roukaku_platform::DecodeStreamAction::Start,
            bytes: include_bytes!(
                "../../../../../Runtime/roukaku-media/tests/fixtures/av-clock.mp4"
            )
            .to_vec()
            .into(),
        };
        let mut stream = VideoStream::open(&request).unwrap();
        let DecodeOutput::VideoFrame {
            pts_us,
            duration_us,
            bgra8,
            ..
        } = stream.next().unwrap()
        else {
            panic!("first sample must be a frame");
        };
        assert_eq!(pts_us, 0);
        assert_eq!(duration_us, 40_000);
        assert_eq!(bgra8.len(), 64 * 64 * 4);
        stream.seek(1_400_000).unwrap();
        let DecodeOutput::VideoFrame {
            pts_us, sequence, ..
        } = stream.next().unwrap()
        else {
            panic!("seek must produce a frame");
        };
        assert!(pts_us >= 1_400_000);
        assert_eq!(sequence, 1);
        let reader = stream.reading.as_ref().unwrap().0.clone();
        drop(stream);
        assert_eq!(unsafe { reader.status() }, AVAssetReaderStatus::Cancelled);
    }
}
