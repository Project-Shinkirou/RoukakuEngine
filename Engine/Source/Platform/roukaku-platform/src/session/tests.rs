#[test]
fn platform_accepts_balanced_pixel_masks_and_rejects_underflow() {
    use roukaku_media_core::SceneCommand;
    let mut frame = super::SceneFrame {
        sequence: 1,
        width: 8,
        height: 8,
        clear_rgba: [0, 0, 0, 255],
        commands: vec![
            SceneCommand::PushPixelMask { bits: u64::MAX },
            SceneCommand::PopPixelMask,
        ],
        semantics: None,
    };
    super::validate_scene_frame(&frame, 1024 * 1024).unwrap();
    frame.commands.pop();
    assert!(super::validate_scene_frame(&frame, 1024 * 1024).is_err());
    frame.commands = vec![SceneCommand::PopPixelMask];
    assert!(super::validate_scene_frame(&frame, 1024 * 1024).is_err());
}

#[test]
fn mask_reveal_validates_mode_phase_and_both_texture_payloads() {
    use roukaku_media_core::{MaskRevealMode, OwnedPixelBuffer, RectI, SceneCommand, TextureFrame};
    let command = |mode, phase| SceneCommand::MaskReveal {
        id: "reveal".into(),
        frame: TextureFrame {
            width: 2,
            height: 2,
            rgba8: OwnedPixelBuffer::from_vec(vec![255; 16]),
        },
        mask: TextureFrame {
            width: 2,
            height: 2,
            rgba8: OwnedPixelBuffer::from_vec(vec![128; 16]),
        },
        destination: RectI::new(0, 0, 2, 2),
        mode,
        phase,
        opacity: 1.0,
    };
    let mut frame = super::SceneFrame {
        sequence: 1,
        width: 2,
        height: 2,
        clear_rgba: [0, 0, 0, 255],
        commands: vec![],
        semantics: None,
    };
    for (mode, phase) in [
        (MaskRevealMode::Gradient, 0.0),
        (MaskRevealMode::Gradient, 512.0),
        (MaskRevealMode::Threshold, 256.0),
    ] {
        frame.commands = vec![command(mode, phase)];
        super::validate_scene_frame(&frame, 32).unwrap();
    }
    for (mode, phase) in [
        (MaskRevealMode::Gradient, -1.0),
        (MaskRevealMode::Gradient, 513.0),
        (MaskRevealMode::Threshold, 257.0),
        (MaskRevealMode::Gradient, f32::NAN),
    ] {
        frame.commands = vec![command(mode, phase)];
        assert!(super::validate_scene_frame(&frame, 32).is_err());
    }
    frame.commands = vec![command(MaskRevealMode::Gradient, 256.0)];
    assert!(super::validate_scene_frame(&frame, 31).is_err());
    if let SceneCommand::MaskReveal { mask, .. } = &mut frame.commands[0] {
        mask.rgba8 = OwnedPixelBuffer::from_vec(vec![128; 15]);
    }
    assert!(super::validate_scene_frame(&frame, 32).is_err());
}

use super::AudioWakeRegistration;
use std::{sync::Arc, thread, time::Duration};

#[test]
fn audio_wake_waits_for_an_edge_without_periodic_polling() {
    let registration = AudioWakeRegistration::default();
    let worker_registration = registration.clone();
    let worker = thread::spawn(move || {
        thread::sleep(Duration::from_millis(5));
        worker_registration.notify();
    });
    let sequence = registration
        .wait_timeout(0, Duration::from_secs(1))
        .expect("callback edge should wake the waiter");
    assert_eq!(sequence, 1);
    worker.join().expect("audio wake worker must stop");
}

#[test]
fn audio_wake_timeout_uses_an_absolute_deadline() {
    let registration = AudioWakeRegistration::default();
    let start = std::time::Instant::now();
    assert!(
        registration
            .wait_timeout(0, Duration::from_millis(10))
            .is_none()
    );
    assert!(start.elapsed() < Duration::from_millis(250));

    let registration = Arc::new(registration);
    let notifier = Arc::clone(&registration);
    let worker = thread::spawn(move || {
        thread::sleep(Duration::from_millis(5));
        notifier.notify();
    });
    let sequence = registration
        .wait_timeout(0, Duration::from_secs(1))
        .expect("a later edge should still wake the waiter");
    assert_eq!(sequence, 1);
    worker.join().expect("audio wake worker must stop");
}
