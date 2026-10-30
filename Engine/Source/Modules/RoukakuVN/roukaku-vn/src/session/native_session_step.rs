use super::*;

const MAX_FAST_FORWARD_LINES: usize = 4096;

fn data_string(data: &serde_json::Value, key: &str) -> Option<String> {
    data.get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

impl VnSession {
    pub(super) fn launch(&mut self, paragraph: ParagraphId) -> Result<(), VnError> {
        self.state.system_stack.clear();
        self.state.pending_choice = None;
        self.state.pending_wait = None;
        self.menu_frame = false;
        self.wait = None;
        let status = self.authoring.start_paragraph(paragraph);
        self.after_story(status)
    }

    pub(super) fn resume_action(
        &mut self,
        node: &str,
        payload: serde_json::Value,
    ) -> Result<(), VnError> {
        self.resume(&format!("action:{node}"), payload)
    }

    /// Resume the story wait only when the live wait has exactly this action.
    fn resume(&mut self, action: &str, payload: serde_json::Value) -> Result<(), VnError> {
        let wait = self
            .wait
            .clone()
            .filter(|wait| wait.action == action)
            .ok_or_else(|| {
                VnError::diagnostic(
                    "ROUKAKU_VN_WAIT_MISMATCH",
                    format!("story is not waiting for {action}"),
                )
            })?;
        let accepted = self.authoring.input(wait.token, action, payload);
        let accepted = match accepted {
            Ok(accepted) => accepted,
            Err(error) => {
                self.after_story(Err(error))?;
                return Ok(());
            }
        };
        if !accepted {
            return Err(VnError::diagnostic(
                "ROUKAKU_VN_WAIT_STALE",
                "story input token is stale",
            ));
        }
        let status = Ok(self.authoring.status());
        self.after_story(status)
    }

    /// Apply buffered story events, then project the coroutine status into display waits.
    pub(super) fn after_story(&mut self, status: mlua::Result<StoryStatus>) -> Result<(), VnError> {
        self.apply_story_events()?;
        let status = status.map_err(authoring_error)?;
        self.wait = None;
        self.state.pending_wait = None;
        self.state.pending_choice = None;
        if self.menu_frame {
            self.state.system_stack.clear();
            self.menu_frame = false;
        }
        match status {
            StoryStatus::Waiting(wait) => self.project_wait(wait),
            StoryStatus::Completed => {
                self.pending.completed_route = self.state.cursor.take().map(|cursor| {
                    paragraph_route_id(&ParagraphId {
                        module: cursor.story_id,
                        entry: cursor.state_id,
                    })
                });
                self.state.system_stack.clear();
                Ok(())
            }
            StoryStatus::Idle | StoryStatus::Cancelled | StoryStatus::Failed => {
                Err(VnError::diagnostic(
                    "ROUKAKU_VN_STORY_FAILED",
                    format!("story stopped without a wait: {status:?}"),
                ))
            }
        }
    }

    fn project_wait(&mut self, wait: InputWait) -> Result<(), VnError> {
        let await_id = Some(wait.token.to_string());
        let node = self
            .state
            .cursor
            .as_ref()
            .map(|cursor| cursor.command_id.clone())
            .unwrap_or_default();
        let (kind, fence, command_id) = match wait.action.as_str() {
            "advance" => (VnWaitKind::Dialogue, wait.action.clone(), node),
            "choose" => {
                let choice = self.last_choice.clone().ok_or_else(|| {
                    VnError::diagnostic(
                        "ROUKAKU_VN_CHOICE_MISSING",
                        "choice wait has no recorded choice",
                    )
                })?;
                let id = choice.choice_id.clone();
                self.state.pending_choice = Some(choice);
                (VnWaitKind::Choice, wait.action.clone(), id)
            }
            "menu.select" => {
                let menu = self.last_choice.clone().ok_or_else(|| {
                    VnError::diagnostic("ROUKAKU_VN_MENU_MISSING", "menu wait has no record")
                })?;
                let cursor = self.state.cursor.clone().ok_or_else(|| {
                    VnError::diagnostic("ROUKAKU_VN_MENU_MISSING", "menu wait has no paragraph")
                })?;
                self.state.system_stack = vec![VnSystemFrame {
                    return_to: cursor,
                    return_wait: None,
                    return_choice: None,
                    page: SystemPageKind::parse(&menu.key),
                }];
                self.menu_frame = true;
                (VnWaitKind::SystemPage, wait.action.clone(), menu.choice_id)
            }
            action => match (
                action.strip_prefix("stage:"),
                action.strip_prefix("action:"),
            ) {
                (Some(fence), _) => (VnWaitKind::Fence, fence.to_owned(), fence.to_owned()),
                (_, Some(node)) => (VnWaitKind::Fence, format!("policy.{node}"), node.to_owned()),
                _ => (VnWaitKind::Input, action.to_owned(), action.to_owned()),
            },
        };
        self.state.pending_wait = Some(VnWaitState {
            await_id,
            ..VnWaitState::new(kind, fence, command_id)
        });
        self.wait = Some(wait);
        Ok(())
    }

    pub(super) fn apply_story_events(&mut self) -> Result<(), VnError> {
        for event in self.host.take_events() {
            match event {
                super::story_host::StoryEvent::ParagraphStarted(paragraph) => {
                    let id = paragraph_route_id(&paragraph);
                    self.state.cursor = Some(VnCommandCursor {
                        story_id: paragraph.module.clone(),
                        state_id: paragraph.entry.clone(),
                        scene_id: paragraph.entry.clone(),
                        command_id: id.clone(),
                        ordinal: 0,
                    });
                    self.paragraph_backlog_start = self.state.backlog.len();
                    self.state.route_coverage.insert(id.clone());
                    self.pending.coverage_reached.insert(id);
                }
                super::story_host::StoryEvent::Present { node, command } => {
                    if let StageCommand::SetSkipAllowed { allowed } = &command {
                        self.state.system.skip_allowed = *allowed;
                    }
                    self.pending.present(&node, command);
                }
                super::story_host::StoryEvent::Record(record) => self.apply_record(record),
                super::story_host::StoryEvent::Property(mut sample) => {
                    sample.presentation_index = self.pending.presentations.len();
                    self.pending.properties.push(sample);
                }
                super::story_host::StoryEvent::ActionStart {
                    node,
                    command,
                    params,
                } => self.queue_policy_action(&node, &command, &params)?,
            }
        }
        Ok(())
    }

    fn apply_record(&mut self, record: NodeRecord) {
        let node = record.occurrence.node.clone();
        if let Some(cursor) = self.state.cursor.as_mut() {
            cursor.command_id = node.clone();
            cursor.ordinal = cursor.ordinal.saturating_add(1);
        }
        match record.content {
            NodeContent::Line { data } => {
                let key = data_string(&data, "key").unwrap_or_else(|| node.clone());
                let speaker = data_string(&data, "speaker");
                let voice = data_string(&data, "voice");
                let window = data_string(&data, "window");
                let read = !self.state.read_state.insert(node.clone());
                if let Some(voice) = &voice {
                    self.state.voice_replay.insert(
                        voice.clone(),
                        VoiceReplayEntry {
                            voice: voice.clone(),
                            line_key: key.clone(),
                            speaker: speaker.clone(),
                        },
                    );
                }
                self.state.backlog.push(BacklogEntry {
                    command_id: node,
                    key: key.clone(),
                    speaker: speaker.clone(),
                    voice: voice.clone(),
                    story_id: record.occurrence.paragraph.module.clone(),
                    state_id: record.occurrence.paragraph.entry.clone(),
                    route_position: self.state.backlog.len(),
                    read,
                    layout: BacklogLayoutMetadata {
                        window: window.clone(),
                    },
                });
                self.pending
                    .presentations
                    .push(PresentationCommand::Dialogue {
                        key,
                        speaker,
                        voice,
                        window,
                    });
            }
            NodeContent::Choice { options, data, .. } => {
                // Menus carry their system page kind; choices carry their prompt key.
                let menu_kind = data_string(&data, "kind").filter(|_| data.get("key").is_none());
                let key = menu_kind
                    .clone()
                    .or_else(|| data_string(&data, "key"))
                    .unwrap_or_else(|| node.clone());
                let options = options
                    .into_iter()
                    .map(|option| ChoiceOption {
                        target: option.id.clone(),
                        id: option.id,
                        key: option.text,
                        enabled_when: None,
                    })
                    .collect::<Vec<_>>();
                self.pending.presentations.push(match &menu_kind {
                    Some(kind) => PresentationCommand::SystemPage {
                        page: SystemPageKind::parse(kind),
                    },
                    None => PresentationCommand::Choice {
                        key: key.clone(),
                        options: options.clone(),
                    },
                });
                self.last_choice = Some(PendingChoice {
                    choice_id: node,
                    key,
                    enabled_option_ids: options.iter().map(|option| option.id.clone()).collect(),
                    options,
                });
            }
        }
    }

    pub(super) fn apply_command(&mut self, command: VnPlayerCommand) -> Result<(), VnError> {
        match command {
            VnPlayerCommand::Launch { story_id, state_id } => self.launch(ParagraphId {
                module: story_id,
                entry: state_id,
            })?,
            VnPlayerCommand::Advance => {
                if self.state.system.reading_mode == ReadingMode::Hidden {
                    self.state.system.reading_mode = ReadingMode::Manual;
                } else if self.overlay_depth() == 0
                    && self
                        .wait
                        .as_ref()
                        .is_some_and(|wait| wait.action == "advance")
                {
                    self.resume("advance", serde_json::Value::Null)?;
                }
            }
            VnPlayerCommand::Choose { option_id } => {
                let known = self
                    .state
                    .pending_choice
                    .as_ref()
                    .is_some_and(|choice| choice.enabled_option_ids.contains(&option_id));
                if !known || self.overlay_depth() != 0 {
                    return Err(VnError::diagnostic(
                        "ROUKAKU_VN_CHOICE_OPTION",
                        format!("choice option {option_id} is not available"),
                    ));
                }
                self.resume("choose", serde_json::Value::String(option_id))?;
            }
            VnPlayerCommand::OpenSystem { page } => {
                let return_to = self.state.cursor.clone().ok_or_else(|| {
                    VnError::diagnostic(
                        "ROUKAKU_VN_SYSTEM_STACK",
                        "system pages require an active story",
                    )
                })?;
                self.state.system_stack.push(VnSystemFrame {
                    return_to,
                    return_wait: self.state.pending_wait.take(),
                    return_choice: self.state.pending_choice.take(),
                    page,
                });
                let command_id = format!("system.{}", self.state.system_stack.len());
                self.state.pending_wait = Some(VnWaitState::new(
                    VnWaitKind::SystemPage,
                    "system",
                    command_id,
                ));
            }
            VnPlayerCommand::SwitchSystemPage { page } => {
                let frame = self.state.system_stack.last_mut().ok_or_else(|| {
                    VnError::diagnostic(
                        "ROUKAKU_VN_SYSTEM_STACK",
                        "system.switch was supplied without an open system page",
                    )
                })?;
                frame.page = page;
            }
            VnPlayerCommand::ReturnSystem => {
                if self.overlay_depth() > 0 {
                    let frame = self.state.system_stack.pop().ok_or_else(|| {
                        VnError::diagnostic("ROUKAKU_VN_SYSTEM_STACK", "system stack is empty")
                    })?;
                    self.state.pending_wait = frame.return_wait;
                    self.state.pending_choice = frame.return_choice;
                } else if self.menu_frame {
                    self.resume("menu.select", serde_json::Value::String("return".into()))?;
                } else {
                    return Err(VnError::diagnostic(
                        "ROUKAKU_VN_SYSTEM_STACK",
                        "system.return was supplied without an open system page",
                    ));
                }
            }
            VnPlayerCommand::InvokeSystemAction { action_id } => {
                if !self.menu_frame || self.overlay_depth() != 0 {
                    return Err(VnError::diagnostic(
                        "ROUKAKU_VN_SYSTEM_ACTION_UNAVAILABLE",
                        format!("system action {action_id} requires a story menu"),
                    ));
                }
                self.resume("menu.select", serde_json::Value::String(action_id))?;
            }
            VnPlayerCommand::ReplayVoice { voice } => {
                if !self.state.voice_replay.contains_key(&voice) {
                    return Err(VnError::diagnostic(
                        "ROUKAKU_VN_VOICE_REPLAY_MISSING",
                        format!("voice replay entry {voice} is not available"),
                    ));
                }
                let id = format!("voice.replay.{voice}");
                self.pending.present(
                    &id,
                    StageCommand::Audio(AudioCue {
                        id: id.clone(),
                        bus: VnAudioBus::Voice,
                        asset: voice,
                        looped: false,
                        fade_ms: 0,
                        sync: VnAudioSync::None,
                    }),
                );
            }
            VnPlayerCommand::SetAuto { enabled } => self.state.system.auto_enabled = enabled,
            VnPlayerCommand::SetSkip { mode } => self.state.system.skip_mode = mode,
            VnPlayerCommand::SetReadingMode { mode } => self.state.system.reading_mode = mode,
            VnPlayerCommand::SetAudioEnabled { enabled } => {
                self.state.system.audio_enabled = enabled;
                for (bus, id) in [(VnAudioBus::Bgm, "audio.bgm"), (VnAudioBus::Se, "audio.se")] {
                    self.pending
                        .present(id, StageCommand::SetAudioBusEnabled { bus, enabled });
                }
            }
            VnPlayerCommand::SetConfig { key, value } => {
                validate_system_value("config key", &key, 256)?;
                validate_system_value("config value", &value, 16_384)?;
                if key == "display.language" {
                    if value.len() > 64
                        || !value
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
                    {
                        return Err(VnError::diagnostic(
                            "ROUKAKU_VN_LOCALE_IDENTITY",
                            "display.language must be a safe locale identifier",
                        ));
                    }
                    self.state.locale = value.clone();
                }
                self.state.system.config.insert(key, value);
            }
            VnPlayerCommand::SubmitText { input_id, value } => {
                validate_system_value("text input id", &input_id, 256)?;
                validate_system_value("text input value", &value, 16_384)?;
                self.state
                    .system
                    .config
                    .insert(format!("text_input.{input_id}"), value);
            }
            VnPlayerCommand::Unlock { kind, id } => match kind {
                SystemUnlockKind::Gallery => {
                    self.state.system.gallery_unlocks.insert(id);
                }
                SystemUnlockKind::Replay => {
                    self.state.system.replay_unlocks.insert(id);
                }
            },
            VnPlayerCommand::StartReplay { replay_id } => {
                if !self.state.system.replay_unlocks.contains(&replay_id) {
                    return Err(VnError::diagnostic(
                        "ROUKAKU_VN_REPLAY_LOCKED",
                        format!("replay {replay_id} is not unlocked"),
                    ));
                }
                self.launch(paragraph_target(&replay_id)?)?;
            }
            VnPlayerCommand::PreviewGallery { item_id } => {
                if !self.state.system.gallery_unlocks.contains(&item_id) {
                    return Err(VnError::diagnostic(
                        "ROUKAKU_VN_GALLERY_LOCKED",
                        format!("gallery item {item_id} is not unlocked"),
                    ));
                }
                self.pending
                    .presentations
                    .push(PresentationCommand::Marker {
                        id: format!("gallery.preview.{item_id}"),
                    });
            }
            VnPlayerCommand::JumpRoute { node_id } => {
                if !self.state.route_coverage.contains(&node_id) {
                    return Err(VnError::diagnostic(
                        "ROUKAKU_VN_ROUTE_JUMP_DENIED",
                        format!("route node {node_id} is not a reached paragraph"),
                    ));
                }
                self.launch(paragraph_target(&node_id)?)?;
            }
            VnPlayerCommand::JumpBacklog { command_id } => self.rollback_to_line(&command_id)?,
            VnPlayerCommand::CompleteWait { fence } => self.complete_fence(&fence)?,
        }
        self.fast_forward()
    }

    /// Story-owned menu frames are not overlays; user-opened pages are.
    fn overlay_depth(&self) -> usize {
        self.state
            .system_stack
            .len()
            .saturating_sub(usize::from(self.menu_frame))
    }

    fn complete_fence(&mut self, fence: &str) -> Result<(), VnError> {
        let callbacks = match self.authoring.signal_fence(fence) {
            Ok(callbacks) => callbacks,
            Err(error) => {
                self.failed = true;
                return Err(authoring_error(error));
            }
        };
        self.apply_story_events()?;
        let action = format!("stage:{fence}");
        if self.wait.as_ref().is_some_and(|wait| wait.action == action) {
            return self.resume(&action, serde_json::Value::Null);
        }
        if callbacks == 0 {
            return Err(VnError::diagnostic(
                "ROUKAKU_VN_WAIT_FENCE",
                format!("fence {fence} has no pending wait or callback"),
            ));
        }
        Ok(())
    }

    /// Controlled replay from the paragraph entry through the selected line.
    fn rollback_to_line(&mut self, command_id: &str) -> Result<(), VnError> {
        let occurrence = self
            .authoring
            .records()
            .into_iter()
            .rev()
            .find(|record| {
                record.occurrence.node == command_id
                    && matches!(record.content, NodeContent::Line { .. })
            })
            .map(|record| record.occurrence)
            .ok_or_else(|| {
                VnError::diagnostic(
                    "ROUKAKU_VN_BACKLOG_JUMP_DENIED",
                    format!("backlog line {command_id} is not in the live paragraph"),
                )
            })?;
        self.state.backlog.truncate(self.paragraph_backlog_start);
        self.state.system_stack.clear();
        self.menu_frame = false;
        let outcome = self.authoring.rollback_to(&occurrence).map(|_| ());
        let status = outcome.map(|()| self.authoring.status());
        self.after_story(status)
    }

    fn fast_forward(&mut self) -> Result<(), VnError> {
        for _ in 0..MAX_FAST_FORWARD_LINES {
            let skipping = self.state.system.reading_mode == ReadingMode::FastForward
                && self.state.system.skip_allowed
                && self.overlay_depth() == 0
                && self
                    .wait
                    .as_ref()
                    .is_some_and(|wait| wait.action == "advance");
            if !skipping {
                return Ok(());
            }
            self.resume("advance", serde_json::Value::Null)?;
        }
        Ok(())
    }
}

fn paragraph_target(id: &str) -> Result<ParagraphId, VnError> {
    parse_route_id(id).ok_or_else(|| {
        VnError::diagnostic(
            "ROUKAKU_VN_PARAGRAPH_TARGET",
            format!("{id} is not a module/entry paragraph identity"),
        )
    })
}

fn validate_system_value(label: &str, value: &str, max_bytes: usize) -> Result<(), VnError> {
    if value.trim().is_empty()
        || value.len() > max_bytes
        || value.chars().any(|character| character.is_control())
    {
        return Err(VnError::diagnostic(
            "ROUKAKU_VN_SYSTEM_VALUE_INVALID",
            format!("{label} must be non-empty, bounded, and contain no control characters"),
        ));
    }
    Ok(())
}
