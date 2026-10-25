use roukaku_media::container_audio_start_us;

#[test]
fn real_movie_tracks_distinguish_audio_silent_and_corrupt_inputs() {
    assert_eq!(
        container_audio_start_us(
            "mp4",
            include_bytes!("fixtures/av-clock.mp4").to_vec().into()
        )
        .unwrap(),
        Some(0)
    );
    assert_eq!(
        container_audio_start_us("mp4", include_bytes!("fixtures/silent.mp4").to_vec().into())
            .unwrap(),
        None
    );
    assert_eq!(
        container_audio_start_us(
            "webm",
            include_bytes!("fixtures/silent.webm").to_vec().into()
        )
        .unwrap(),
        None
    );
    assert!(container_audio_start_us("mp4", vec![0, 1, 2].into()).is_err());
}
