use std::{collections::BTreeSet, rc::Rc};

use roukaku_runtime::TaskScope;

use super::{Context, PropertyTrack, SegmentStatus, TimelineDeclaration};

pub(super) struct Segment {
    pub status: SegmentStatus,
    declaration: Rc<TimelineDeclaration>,
    elapsed: f64,
    fired: Vec<bool>,
    scope: TaskScope,
}

impl TimelineDeclaration {
    pub fn validate(&self) -> mlua::Result<()> {
        if !self.duration.is_finite() || self.duration < 0.0 {
            return Err(mlua::Error::runtime(
                "timeline duration must be finite and nonnegative",
            ));
        }
        let mut properties = BTreeSet::new();
        for track in &self.tracks {
            if track.target.is_empty()
                || track.property.is_empty()
                || track.keys.is_empty()
                || !properties.insert((&track.target, &track.property))
            {
                return Err(mlua::Error::runtime(
                    "timeline requires distinct nonempty target/property tracks and keys",
                ));
            }
            let mut previous = None;
            for key in &track.keys {
                if !key.time.is_finite()
                    || !key.value.is_finite()
                    || key.time < 0.0
                    || key.time > self.duration
                    || previous.is_some_and(|time| key.time <= time)
                {
                    return Err(mlua::Error::runtime(
                        "keyframe times must increase within timeline duration; values must be finite",
                    ));
                }
                previous = Some(key.time);
            }
        }
        for callback in &self.callbacks {
            if !callback.time.is_finite() || callback.time < 0.0 || callback.time > self.duration {
                return Err(mlua::Error::runtime("callback time outside timeline"));
            }
        }
        Ok(())
    }

    /// Pure arbitrary-time evaluation. Does not dispatch free logic or touch a host.
    pub fn evaluate(&self, time: f64, mut apply: impl FnMut(&str, &str, f64)) -> mlua::Result<()> {
        self.validate()?;
        if !time.is_finite() {
            return Err(mlua::Error::runtime("timeline time must be finite"));
        }
        for track in &self.tracks {
            apply(
                &track.target,
                &track.property,
                sample(track, time.clamp(0.0, self.duration)),
            );
        }
        Ok(())
    }
}

fn sample(track: &PropertyTrack, time: f64) -> f64 {
    let index = track.keys.partition_point(|key| key.time <= time);
    if index == 0 {
        return track.keys[0].value;
    }
    if index == track.keys.len() {
        return track.keys[index - 1].value;
    }
    let start = &track.keys[index - 1];
    let end = &track.keys[index];
    let fraction = (time - start.time) / (end.time - start.time);
    // Weighted interpolation also avoids overflow for opposite large endpoints.
    start.value * (1.0 - fraction) + end.value * fraction
}

fn apply(context: &Context, declaration: &TimelineDeclaration, time: f64) -> mlua::Result<()> {
    for track in &declaration.tracks {
        set_property(context, &track.target, &track.property, sample(track, time))?;
    }
    Ok(())
}

pub(super) fn set_property(
    context: &Context,
    target: &str,
    property: &str,
    value: f64,
) -> mlua::Result<()> {
    context.active()?;
    if target.is_empty() || property.is_empty() || !value.is_finite() {
        return Err(mlua::Error::runtime(
            "stage property requires nonempty identity and finite value",
        ));
    }
    let added_bytes = {
        let shared = context.shared.borrow();
        match shared.recoverable.scene.get(target) {
            Some(serde_json::Value::Object(properties)) if properties.contains_key(property) => 0,
            Some(serde_json::Value::Object(_)) => property.len().saturating_add(64),
            Some(_) => {
                return Err(mlua::Error::runtime(
                    "stage target collides with non-object scene state",
                ));
            }
            None => target
                .len()
                .saturating_add(property.len())
                .saturating_add(128),
        }
    };
    context.charge(added_bytes)?;
    context.host.set_property(target, property, value)?;
    let mut shared = context.shared.borrow_mut();
    let scene = &mut shared.recoverable.scene;
    if !scene.contains_key(target) {
        scene.insert(target.to_owned(), serde_json::json!({}));
    }
    let properties = scene
        .get_mut(target)
        .and_then(serde_json::Value::as_object_mut)
        .expect("validated scene object");
    if let Some(slot) = properties.get_mut(property) {
        *slot = serde_json::json!(value);
    } else {
        properties.insert(property.to_owned(), serde_json::json!(value));
    }
    Ok(())
}

pub(super) fn start(context: &Context, declaration: TimelineDeclaration) -> mlua::Result<u64> {
    context.active()?;
    declaration.validate()?;
    // Validate references without retaining old functions across module replacement.
    for callback in &declaration.callbacks {
        context.resolve(&callback.call)?;
        context.resolve(&callback.complete)?;
    }
    for reference in declaration.on_complete.iter().chain(&declaration.on_cancel) {
        context.resolve(reference)?;
    }
    let declaration = Rc::new(declaration);
    let id = {
        let mut shared = context.shared.borrow_mut();
        if shared.paragraph.is_none() || shared.cancelling {
            return Err(mlua::Error::runtime("segments require an active paragraph"));
        }
        let id = super::live_id()?;
        let scope = shared.scope.child();
        shared.segments.insert(
            id,
            Segment {
                status: SegmentStatus::Running,
                fired: vec![false; declaration.callbacks.len()],
                elapsed: 0.0,
                declaration: Rc::clone(&declaration),
                scope,
            },
        );
        shared.active_segments.insert(id);
        id
    };
    // First sample and time-zero callbacks have the same semantics as any tick.
    advance(context, id, 0.0)?;
    Ok(id)
}

pub(super) fn advance(context: &Context, id: u64, seconds: f64) -> mlua::Result<()> {
    context.active()?;
    let (declaration, time, cancelled) = {
        let mut shared = context.shared.borrow_mut();
        let segment = shared
            .segments
            .get_mut(&id)
            .ok_or_else(|| mlua::Error::runtime("unknown segment"))?;
        if segment.status != SegmentStatus::Running {
            return Ok(());
        }
        segment.elapsed = (segment.elapsed + seconds).min(segment.declaration.duration);
        (
            Rc::clone(&segment.declaration),
            segment.elapsed,
            segment.scope.is_cancelled(),
        )
    };
    if cancelled {
        finish(context, id, SegmentStatus::Cancelled)?;
        return Ok(());
    }
    apply(context, &declaration, time)?;
    for (index, callback) in declaration.callbacks.iter().enumerate() {
        if callback.time <= time {
            let run = {
                let mut shared = context.shared.borrow_mut();
                let segment = shared
                    .segments
                    .get_mut(&id)
                    .expect("segment remains registered");
                if segment.status != SegmentStatus::Running {
                    return Ok(());
                }
                !std::mem::replace(&mut segment.fired[index], true)
            };
            if run {
                context.invoke(&callback.call)?;
            }
        }
    }
    if time >= declaration.duration {
        finish_inner(context, id, SegmentStatus::Completed, true)?;
    }
    Ok(())
}

pub(super) fn finish(context: &Context, id: u64, outcome: SegmentStatus) -> mlua::Result<bool> {
    finish_inner(context, id, outcome, false)
}

fn finish_inner(
    context: &Context,
    id: u64,
    outcome: SegmentStatus,
    sampled: bool,
) -> mlua::Result<bool> {
    if outcome == SegmentStatus::Completed {
        context.active()?;
    }
    let (declaration, fired, scope) = {
        let mut shared = context.shared.borrow_mut();
        let segment = shared
            .segments
            .get_mut(&id)
            .ok_or_else(|| mlua::Error::runtime("unknown segment"))?;
        if segment.status != SegmentStatus::Running {
            return Ok(false);
        }
        segment.status = outcome;
        (
            Rc::clone(&segment.declaration),
            std::mem::take(&mut segment.fired),
            segment.scope.clone(),
        )
    };
    context.shared.borrow_mut().active_segments.remove(&id);
    // Terminal ownership is claimed before running user code: reentrancy and
    // retries cannot repeat a required result, even if a callback reports an error.
    let result = (|| {
        if outcome == SegmentStatus::Completed {
            if !sampled {
                apply(context, &declaration, declaration.duration)?;
            }
            for (callback, fired) in declaration.callbacks.iter().zip(fired) {
                if !fired {
                    context.invoke(&callback.complete)?;
                }
            }
            if let Some(reference) = &declaration.on_complete {
                context.invoke(reference)?;
            }
        } else if !scope.is_cancelled()
            && let Some(reference) = &declaration.on_cancel
        {
            context.invoke(reference)?;
        }
        Ok(())
    })();
    scope.cancel();
    let released = context.host.release_segment(id);
    // Release always runs, even on callback/property failure; report both errors.
    match (result, released) {
        (Err(error), Err(release)) => Err(mlua::Error::runtime(format!(
            "{error}; segment release: {release}"
        ))),
        (Err(error), _) | (_, Err(error)) => Err(error),
        _ => Ok(true),
    }
}
