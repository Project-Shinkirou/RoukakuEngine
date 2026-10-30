use super::*;

impl NativeVnHostCommandSource {
    pub fn launch(&mut self) -> Result<PlayerHostCommandBatch, NativeVnHostError> {
        self.ensure_presentation_active()?;
        self.step(NativeVnStepCommand::LaunchDefault)
    }

    pub fn take_audio_preload_requests(&mut self) -> Vec<NativeVnAudioPreloadRequest> {
        std::mem::take(&mut self.pending_audio_preloads)
    }

    /// An authored `preload` warms its asset off the presentation-critical path:
    /// images through the bounded prefetcher, audio through the media host cache.
    pub(super) fn schedule_preload(&mut self, asset_id: &str) -> Result<(), NativeVnHostError> {
        if self.asset_store.contains_image(asset_id) {
            if !self.live_texture_ids.contains(asset_id)
                && !self.textures.contains_key(asset_id)
                && !self.image_prefetch_inflight.contains(asset_id)
                && !self.asset_store.is_image_cached(asset_id)?
                && self.image_prefetcher.try_schedule(asset_id.to_string())?
            {
                self.image_prefetch_inflight.insert(asset_id.to_string());
            }
            return Ok(());
        }
        if self.asset_store.contains_audio(asset_id) {
            if self
                .pending_audio_preloads
                .iter()
                .any(|request| request.asset_id == asset_id)
            {
                return Ok(());
            }
            let asset = self.asset_store.load_media(asset_id)?;
            self.pending_audio_preloads
                .push(NativeVnAudioPreloadRequest {
                    asset_id: asset_id.to_string(),
                    codec: asset.codec,
                    encoded_bytes: asset.bytes,
                    encoded_length: asset.byte_length,
                });
            return Ok(());
        }
        if self.asset_store.contains_media(asset_id) {
            return Ok(());
        }
        Err(NativeVnHostError::Asset(format!(
            "ROUKAKU_PLAYER_PRELOAD_ASSET_MISSING: {asset_id}"
        )))
    }
}
