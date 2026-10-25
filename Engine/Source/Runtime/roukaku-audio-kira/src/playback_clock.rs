use crate::AudioPlaybackProgress;

/// Media clock for a non-looping PCM voice. Device-consumed mixer silence keeps a
/// longer video track moving after audio EOS; host time never enters this clock.
#[derive(Debug, Clone)]
pub struct AudioMasterClock {
    voice_end_us: u64,
    position_us: u64,
    output_position_us: u64,
    output_epoch: Option<u64>,
}

impl AudioMasterClock {
    pub fn new(voice_end_us: u64) -> Self {
        Self {
            voice_end_us,
            position_us: 0,
            output_position_us: 0,
            output_epoch: None,
        }
    }

    pub fn seek(&mut self, position_us: u64) {
        self.position_us = position_us;
        self.output_epoch = None;
    }

    pub fn update(&mut self, progress: AudioPlaybackProgress, output_epoch: u64) -> u64 {
        let position = if let Some(voice) = progress.voice_position_us {
            voice.max(self.position_us)
        } else if self.output_epoch == Some(output_epoch) {
            self.position_us
                .saturating_add(
                    progress
                        .output_position_us
                        .saturating_sub(self.output_position_us),
                )
                .max(self.voice_end_us)
        } else {
            self.position_us.max(self.voice_end_us)
        };
        self.position_us = position;
        self.output_position_us = progress.output_position_us;
        self.output_epoch = Some(output_epoch);
        position
    }
}
