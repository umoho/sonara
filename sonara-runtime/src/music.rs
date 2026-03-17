// SPDX-License-Identifier: MPL-2.0

use sonara_model::{
    Clip, EdgeTrigger, EntryPolicy, MusicEdge, MusicGraph, MusicNode, MusicNodeId, PlaybackTarget,
    TrackGroupId, TrackGroupMode, TrackRole,
};

use crate::bank::SonaraRuntime;
use crate::error::RuntimeError;
use crate::ids::MusicSessionId;
use crate::types::{
    ActiveMusicSession, Fade, MusicPhase, MusicStatus, NextCueMatch, PendingMusicTransition,
    ResolvedMusicPlayback, ResumeMemoryEntry, TrackGroupState,
};

impl SonaraRuntime {
    /// 启动一个音乐图会话，使用图中声明的初始节点。
    pub fn play_music_graph(
        &mut self,
        graph_id: sonara_model::MusicGraphId,
    ) -> Result<MusicSessionId, RuntimeError> {
        self.play_music_graph_in_node(graph_id, None)
    }

    /// 启动一个音乐图会话，并显式指定初始节点。
    pub fn play_music_graph_in_node(
        &mut self,
        graph_id: sonara_model::MusicGraphId,
        initial_node: Option<MusicNodeId>,
    ) -> Result<MusicSessionId, RuntimeError> {
        let graph = self
            .music_graphs
            .get(&graph_id)
            .ok_or(RuntimeError::MusicGraphNotLoaded(graph_id))?;
        let active_node = resolve_music_graph_node(graph, initial_node)?;
        let node = lookup_music_node(graph, active_node)?;
        let session_id = MusicSessionId(self.next_music_session_id);
        self.next_music_session_id += 1;
        let track_group_states = graph
            .groups
            .iter()
            .map(|group| (group.id, TrackGroupState { active: true }))
            .collect();

        self.music_sessions.insert(
            session_id,
            ActiveMusicSession {
                id: session_id,
                graph_id,
                desired_target_node: active_node,
                active_node,
                current_entry: node.default_entry.clone(),
                phase: MusicPhase::Stable,
                pending_transition: None,
                track_group_states,
            },
        );

        self.enter_music_node(
            session_id,
            active_node,
            active_node,
            node.default_entry.clone(),
        )?;

        Ok(session_id)
    }

    /// 请求一个音乐会话切换到目标节点。
    pub fn request_music_node(
        &mut self,
        session_id: MusicSessionId,
        target_node_id: MusicNodeId,
    ) -> Result<(), RuntimeError> {
        let (graph_id, active_node, phase) = {
            let session = self
                .music_sessions
                .get(&session_id)
                .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
            (session.graph_id, session.active_node, session.phase)
        };

        if phase == MusicPhase::Stopped {
            return Err(RuntimeError::MusicSessionPhaseMismatch {
                session_id,
                expected: MusicPhase::Stable,
                actual: phase,
            });
        }

        let graph = self
            .music_graphs
            .get(&graph_id)
            .ok_or(RuntimeError::MusicGraphNotLoaded(graph_id))?;
        let target_node = lookup_music_node(graph, target_node_id)?;
        if !target_node.externally_targetable {
            return Err(RuntimeError::MusicEdgeNotFound {
                graph_id,
                from: active_node,
                to: target_node_id,
            });
        }

        if active_node == target_node_id {
            let session = self
                .music_sessions
                .get_mut(&session_id)
                .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
            session.desired_target_node = target_node_id;
            session.phase = MusicPhase::Stable;
            session.pending_transition = None;
            return Ok(());
        }

        let transition = lookup_transition_rule(graph, active_node, target_node_id)?.clone();
        let pending_transition = Self::build_pending_transition(active_node, &transition);
        let session = self
            .music_sessions
            .get_mut(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
        session.desired_target_node = target_node_id;

        match transition.trigger {
            EdgeTrigger::Immediate => {
                self.enter_music_node(
                    session_id,
                    transition.to,
                    target_node_id,
                    transition.destination,
                )?;
                return Ok(());
            }
            EdgeTrigger::NextMatchingCue { .. } => {
                session.pending_transition = Some(pending_transition);
                session.phase = MusicPhase::WaitingExitCue;
            }
            EdgeTrigger::OnComplete => {
                session.pending_transition = Some(pending_transition);
                session.phase = MusicPhase::WaitingNodeCompletion;
            }
        }

        Ok(())
    }

    /// 预览一次音乐节点切换将使用的最小 transition 语义。
    pub fn preview_music_transition(
        &self,
        session_id: MusicSessionId,
        target_node_id: MusicNodeId,
    ) -> Result<Option<PendingMusicTransition>, RuntimeError> {
        let session = self
            .music_sessions
            .get(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
        if session.phase == MusicPhase::Stopped {
            return Err(RuntimeError::MusicSessionPhaseMismatch {
                session_id,
                expected: MusicPhase::Stable,
                actual: session.phase,
            });
        }

        let graph = self
            .music_graphs
            .get(&session.graph_id)
            .ok_or(RuntimeError::MusicGraphNotLoaded(session.graph_id))?;
        lookup_music_node(graph, target_node_id)?;

        if session.active_node == target_node_id {
            return Ok(None);
        }

        let transition = lookup_transition_rule(graph, session.active_node, target_node_id)?;
        Ok(Some(Self::build_pending_transition(
            session.active_node,
            transition,
        )))
    }

    fn build_pending_transition(
        from_node: MusicNodeId,
        transition: &MusicEdge,
    ) -> PendingMusicTransition {
        PendingMusicTransition {
            from_node,
            to_node: transition.to,
            requested_target_node: transition.requested_target.unwrap_or(transition.to),
            trigger: transition.trigger.clone(),
            destination: transition.destination.clone(),
        }
    }

    fn enter_music_node(
        &mut self,
        session_id: MusicSessionId,
        node_id: MusicNodeId,
        requested_target_node: MusicNodeId,
        entry_policy: EntryPolicy,
    ) -> Result<(), RuntimeError> {
        let (graph_id, next_edge) = {
            let session = self
                .music_sessions
                .get(&session_id)
                .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
            let graph = self
                .music_graphs
                .get(&session.graph_id)
                .ok_or(RuntimeError::MusicGraphNotLoaded(session.graph_id))?;
            lookup_music_node(graph, node_id)?;
            (
                session.graph_id,
                lookup_auto_transition_rule(graph, node_id, requested_target_node).cloned(),
            )
        };

        let session = self
            .music_sessions
            .get_mut(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
        session.active_node = node_id;
        session.current_entry = entry_policy;
        session.desired_target_node = requested_target_node;

        if let Some(edge) = next_edge {
            session.pending_transition = Some(Self::build_pending_transition(node_id, &edge));
            session.phase = MusicPhase::WaitingNodeCompletion;
        } else {
            session.phase = MusicPhase::Stable;
            session.pending_transition = None;
        }

        let _ = graph_id;
        Ok(())
    }

    /// 通知运行时：当前会话已到达允许退出的切点。
    pub fn complete_music_exit(&mut self, session_id: MusicSessionId) -> Result<(), RuntimeError> {
        let session = self
            .music_sessions
            .get_mut(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;

        if session.phase != MusicPhase::WaitingExitCue {
            return Err(RuntimeError::MusicSessionPhaseMismatch {
                session_id,
                expected: MusicPhase::WaitingExitCue,
                actual: session.phase,
            });
        }

        let pending = session
            .pending_transition
            .clone()
            .ok_or(RuntimeError::MusicSessionHasNoPendingTransition(session_id))?;

        let to_node = pending.to_node;
        let requested_target_node = pending.requested_target_node;
        let destination = pending.destination.clone();
        let _ = session;

        self.enter_music_node(session_id, to_node, requested_target_node, destination)
    }

    /// 通知运行时：当前自动推进节点已经完成，可以进入目标节点。
    pub fn complete_music_node_completion(
        &mut self,
        session_id: MusicSessionId,
    ) -> Result<(), RuntimeError> {
        let session = self
            .music_sessions
            .get_mut(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;

        if session.phase != MusicPhase::WaitingNodeCompletion {
            return Err(RuntimeError::MusicSessionPhaseMismatch {
                session_id,
                expected: MusicPhase::WaitingNodeCompletion,
                actual: session.phase,
            });
        }

        let pending = session
            .pending_transition
            .clone()
            .ok_or(RuntimeError::MusicSessionHasNoPendingTransition(session_id))?;

        let to_node = pending.to_node;
        let requested_target_node = pending.requested_target_node;
        let destination = pending.destination.clone();
        let _ = session;

        self.enter_music_node(session_id, to_node, requested_target_node, destination)
    }

    /// 停止一个音乐会话。
    pub fn stop_music_session(
        &mut self,
        session_id: MusicSessionId,
        _fade: Fade,
    ) -> Result<(), RuntimeError> {
        let session = self
            .music_sessions
            .get_mut(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
        session.phase = MusicPhase::Stopped;
        session.pending_transition = None;
        Ok(())
    }

    /// 查询一个音乐会话中某个显式 track group 的当前状态。
    pub fn music_track_group_state(
        &self,
        session_id: MusicSessionId,
        group_id: TrackGroupId,
    ) -> Result<TrackGroupState, RuntimeError> {
        let session = self
            .music_sessions
            .get(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
        let graph = self
            .music_graphs
            .get(&session.graph_id)
            .ok_or(RuntimeError::MusicGraphNotLoaded(session.graph_id))?;
        graph
            .group(group_id)
            .ok_or(RuntimeError::MusicTrackGroupNotFound {
                graph_id: graph.id,
                group_id,
            })?;
        Ok(Self::track_group_state_for_session(session, group_id))
    }

    /// 设置一个音乐会话中某个显式 track group 的开关状态。
    pub fn set_music_track_group_active(
        &mut self,
        session_id: MusicSessionId,
        group_id: TrackGroupId,
        active: bool,
    ) -> Result<(), RuntimeError> {
        let (graph_id, group_mode, exclusive_groups) = {
            let session = self
                .music_sessions
                .get(&session_id)
                .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
            let graph = self
                .music_graphs
                .get(&session.graph_id)
                .ok_or(RuntimeError::MusicGraphNotLoaded(session.graph_id))?;
            let group = graph
                .group(group_id)
                .ok_or(RuntimeError::MusicTrackGroupNotFound {
                    graph_id: graph.id,
                    group_id,
                })?;
            (
                graph.id,
                group.mode,
                graph
                    .groups
                    .iter()
                    .filter(|candidate| candidate.mode == TrackGroupMode::Exclusive)
                    .map(|candidate| candidate.id)
                    .collect::<Vec<_>>(),
            )
        };

        let session = self
            .music_sessions
            .get_mut(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;

        if active && group_mode == TrackGroupMode::Exclusive {
            for exclusive_group_id in exclusive_groups {
                session.track_group_states.insert(
                    exclusive_group_id,
                    TrackGroupState {
                        active: exclusive_group_id == group_id,
                    },
                );
            }
        } else {
            session
                .track_group_states
                .insert(group_id, TrackGroupState { active });
        }

        let _ = graph_id;
        Ok(())
    }

    /// 读取音乐会话当前对游戏侧可见的状态。
    pub fn music_status(&self, session_id: MusicSessionId) -> Result<MusicStatus, RuntimeError> {
        let session = self
            .music_sessions
            .get(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
        let graph = self
            .music_graphs
            .get(&session.graph_id)
            .ok_or(RuntimeError::MusicGraphNotLoaded(session.graph_id))?;
        let state = lookup_music_node(graph, session.active_node)?;
        let current_binding = self.resolve_active_primary_binding(session, graph, state);

        Ok(MusicStatus {
            session_id,
            graph_id: session.graph_id,
            desired_target_node: session.desired_target_node,
            active_node: session.active_node,
            phase: session.phase,
            current_track_id: current_binding.map(|binding| binding.track_id),
            current_target: current_binding.map(|binding| binding.target.clone()),
            pending_transition: session.pending_transition.clone(),
            track_group_states: session.track_group_states.clone(),
        })
    }

    /// 保存一个音乐会话当前状态对应的播放头到记忆槽。
    ///
    /// 只有当当前可听内容仍然对应 active state 时，才会写入记忆槽。
    pub fn save_music_session_resume_position(
        &mut self,
        session_id: MusicSessionId,
        position_seconds: f64,
        saved_at_seconds: f64,
    ) -> Result<bool, RuntimeError> {
        let session = self
            .music_sessions
            .get(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;

        if !position_seconds.is_finite()
            || position_seconds < 0.0
            || !saved_at_seconds.is_finite()
            || saved_at_seconds < 0.0
        {
            return Ok(false);
        }

        if !matches!(
            session.phase,
            MusicPhase::Stable | MusicPhase::WaitingExitCue
        ) {
            return Ok(false);
        }

        let graph = self
            .music_graphs
            .get(&session.graph_id)
            .ok_or(RuntimeError::MusicGraphNotLoaded(session.graph_id))?;
        let state = lookup_music_node(graph, session.active_node)?;
        let Some(slot_id) = state.memory_slot else {
            return Ok(false);
        };

        self.resume_memories.insert(
            slot_id,
            ResumeMemoryEntry {
                position_seconds,
                saved_at_seconds,
            },
        );
        Ok(true)
    }

    /// 为当前音乐会话解析出真正应该播放的 clip 与入口偏移。
    pub fn resolve_music_playback(
        &self,
        session_id: MusicSessionId,
        now_seconds: f64,
    ) -> Result<ResolvedMusicPlayback, RuntimeError> {
        let session = self
            .music_sessions
            .get(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
        let graph = self
            .music_graphs
            .get(&session.graph_id)
            .ok_or(RuntimeError::MusicGraphNotLoaded(session.graph_id))?;
        let state = lookup_music_node(graph, session.active_node)?;
        let binding = self
            .resolve_active_primary_binding(session, graph, state)
            .ok_or(RuntimeError::MusicNodeHasNoActiveTrack {
                graph_id: graph.id,
                node_id: session.active_node,
            })?;
        let clip_id = match &binding.target {
            PlaybackTarget::Clip { clip_id } => clip_id,
        };
        let entry_offset_seconds =
            self.resolve_entry_offset_seconds(state, graph, &session.current_entry, now_seconds);

        Ok(ResolvedMusicPlayback {
            clip_id: *clip_id,
            track_id: Some(binding.track_id),
            entry_offset_seconds,
        })
    }

    /// 为当前活动节点解析一条 stinger track 播放目标。
    pub fn resolve_music_stinger_playback(
        &self,
        session_id: MusicSessionId,
    ) -> Result<Option<ResolvedMusicPlayback>, RuntimeError> {
        let session = self
            .music_sessions
            .get(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
        let graph = self
            .music_graphs
            .get(&session.graph_id)
            .ok_or(RuntimeError::MusicGraphNotLoaded(session.graph_id))?;
        let state = lookup_music_node(graph, session.active_node)?;
        let Some(binding) =
            self.resolve_active_binding_for_role(session, graph, state, TrackRole::Stinger)
        else {
            return Ok(None);
        };
        let clip_id = match &binding.target {
            PlaybackTarget::Clip { clip_id } => *clip_id,
        };

        Ok(Some(ResolvedMusicPlayback {
            clip_id,
            track_id: Some(binding.track_id),
            entry_offset_seconds: 0.0,
        }))
    }

    /// 为当前活动节点解析所有当前激活的 track 播放目标。
    pub fn resolve_music_node_playbacks(
        &self,
        session_id: MusicSessionId,
        now_seconds: f64,
    ) -> Result<Vec<ResolvedMusicPlayback>, RuntimeError> {
        let session = self
            .music_sessions
            .get(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
        let graph = self
            .music_graphs
            .get(&session.graph_id)
            .ok_or(RuntimeError::MusicGraphNotLoaded(session.graph_id))?;
        let node = lookup_music_node(graph, session.active_node)?;
        let entry_offset_seconds =
            self.resolve_entry_offset_seconds(node, graph, &session.current_entry, now_seconds);

        Ok(self
            .active_bindings(session, graph, node)
            .into_iter()
            .map(|binding| {
                let clip_id = match &binding.target {
                    PlaybackTarget::Clip { clip_id } => *clip_id,
                };
                ResolvedMusicPlayback {
                    clip_id,
                    track_id: Some(binding.track_id),
                    entry_offset_seconds,
                }
            })
            .collect())
    }

    /// 为当前 waiting transition 解析下一个合法退出 cue。
    pub fn find_next_music_exit_cue(
        &self,
        session_id: MusicSessionId,
        current_position_seconds: f64,
    ) -> Result<Option<NextCueMatch>, RuntimeError> {
        let session = self
            .music_sessions
            .get(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
        let Some(pending) = &session.pending_transition else {
            return Ok(None);
        };
        let EdgeTrigger::NextMatchingCue { tag } = &pending.trigger else {
            return Ok(None);
        };

        let graph = self
            .music_graphs
            .get(&session.graph_id)
            .ok_or(RuntimeError::MusicGraphNotLoaded(session.graph_id))?;
        let state = lookup_music_node(graph, session.active_node)?;
        let clip_id = match state
            .primary_target(graph)
            .ok_or(RuntimeError::MusicNodeNotFound {
                graph_id: graph.id,
                node_id: session.active_node,
            })? {
            PlaybackTarget::Clip { clip_id } => clip_id,
        };
        let Some(clip) = self.clips.get(&clip_id) else {
            return Ok(None);
        };

        Ok(find_next_matching_cue_in_clip(
            clip,
            tag,
            current_position_seconds,
        ))
    }

    fn track_group_state_for_session(
        session: &ActiveMusicSession,
        group_id: TrackGroupId,
    ) -> TrackGroupState {
        session
            .track_group_states
            .get(&group_id)
            .copied()
            .unwrap_or(TrackGroupState { active: true })
    }

    fn binding_is_active(
        &self,
        session: &ActiveMusicSession,
        graph: &MusicGraph,
        binding: &sonara_model::TrackBinding,
    ) -> bool {
        let Some(track) = graph.track(binding.track_id) else {
            return false;
        };
        let Some(group_id) = track.group else {
            return true;
        };

        Self::track_group_state_for_session(session, group_id).active
    }

    fn active_bindings<'a>(
        &self,
        session: &ActiveMusicSession,
        graph: &'a MusicGraph,
        node: &'a MusicNode,
    ) -> Vec<&'a sonara_model::TrackBinding> {
        node.bindings
            .iter()
            .filter(|binding| self.binding_is_active(session, graph, binding))
            .collect()
    }

    fn resolve_active_primary_binding<'a>(
        &self,
        session: &ActiveMusicSession,
        graph: &'a MusicGraph,
        node: &'a MusicNode,
    ) -> Option<&'a sonara_model::TrackBinding> {
        if let Some(track_id) = node.completion_source {
            if let Some(binding) = node.binding_for_track(track_id) {
                if self.binding_is_active(session, graph, binding) {
                    return Some(binding);
                }
            }
        }

        if let Some(track) = graph.main_track() {
            if let Some(binding) = node.binding_for_track(track.id) {
                if self.binding_is_active(session, graph, binding) {
                    return Some(binding);
                }
            }
        }

        self.active_bindings(session, graph, node)
            .into_iter()
            .next()
    }

    fn resolve_active_binding_for_role<'a>(
        &self,
        session: &ActiveMusicSession,
        graph: &'a MusicGraph,
        node: &'a MusicNode,
        role: TrackRole,
    ) -> Option<&'a sonara_model::TrackBinding> {
        node.bindings.iter().find(|binding| {
            graph
                .track(binding.track_id)
                .map(|track| track.role == role)
                .unwrap_or(false)
                && self.binding_is_active(session, graph, binding)
        })
    }

    fn resolve_entry_offset_seconds(
        &self,
        state: &MusicNode,
        graph: &MusicGraph,
        entry_policy: &EntryPolicy,
        now_seconds: f64,
    ) -> f64 {
        match entry_policy {
            EntryPolicy::Resume => self
                .resolve_resume_offset_seconds(state, now_seconds)
                .unwrap_or_else(|| {
                    self.resolve_reset_entry_offset_seconds(state, graph, now_seconds)
                }),
            EntryPolicy::EntryCue { tag } => {
                self.resolve_entry_cue_offset_seconds(state, graph, tag)
            }
            EntryPolicy::ClipStart
            | EntryPolicy::ResumeNextMatchingCue { .. }
            | EntryPolicy::SameSyncPosition => 0.0,
        }
    }

    fn resolve_resume_offset_seconds(&self, state: &MusicNode, now_seconds: f64) -> Option<f64> {
        let slot_id = state.memory_slot?;
        let entry = self.resume_memories.get(&slot_id)?;
        let ttl_seconds = state
            .memory_policy
            .ttl_seconds
            .map(|ttl| ttl.max(0.0) as f64);

        if let Some(ttl_seconds) = ttl_seconds {
            if now_seconds.is_finite() && now_seconds >= 0.0 {
                let age_seconds = (now_seconds - entry.saved_at_seconds).max(0.0);
                if age_seconds > ttl_seconds {
                    return None;
                }
            }
        }

        Some(entry.position_seconds.max(0.0))
    }

    fn resolve_reset_entry_offset_seconds(
        &self,
        state: &MusicNode,
        graph: &MusicGraph,
        now_seconds: f64,
    ) -> f64 {
        match &state.memory_policy.reset_to {
            EntryPolicy::Resume => 0.0,
            EntryPolicy::ClipStart
            | EntryPolicy::ResumeNextMatchingCue { .. }
            | EntryPolicy::SameSyncPosition => {
                let _ = now_seconds;
                0.0
            }
            EntryPolicy::EntryCue { tag } => {
                self.resolve_entry_cue_offset_seconds(state, graph, tag)
            }
        }
    }

    fn resolve_entry_cue_offset_seconds(
        &self,
        state: &MusicNode,
        graph: &MusicGraph,
        tag: &str,
    ) -> f64 {
        let Some(target) = state.primary_target(graph) else {
            return 0.0;
        };
        let clip_id = match target {
            PlaybackTarget::Clip { clip_id } => clip_id,
        };
        let Some(clip) = self.clips.get(&clip_id) else {
            return 0.0;
        };

        clip.cues
            .iter()
            .filter(|cue| cue.tags.iter().any(|candidate| candidate.as_str() == tag))
            .map(|cue| cue.position_seconds.max(0.0) as f64)
            .min_by(|left, right| left.total_cmp(right))
            .unwrap_or(0.0)
    }
}

fn resolve_music_graph_node(
    graph: &MusicGraph,
    requested_node: Option<MusicNodeId>,
) -> Result<MusicNodeId, RuntimeError> {
    if let Some(node_id) = requested_node.or(graph.initial_node) {
        lookup_music_node(graph, node_id)?;
        return Ok(node_id);
    }

    graph
        .nodes
        .iter()
        .find(|node| node.externally_targetable)
        .or_else(|| graph.nodes.first())
        .map(|node| node.id)
        .ok_or(RuntimeError::MusicGraphHasNoNodes(graph.id))
}

fn lookup_music_node(graph: &MusicGraph, node_id: MusicNodeId) -> Result<&MusicNode, RuntimeError> {
    graph
        .nodes
        .iter()
        .find(|node| node.id == node_id)
        .ok_or(RuntimeError::MusicNodeNotFound {
            graph_id: graph.id,
            node_id,
        })
}

fn lookup_transition_rule(
    graph: &MusicGraph,
    from: MusicNodeId,
    requested_target_node: MusicNodeId,
) -> Result<&MusicEdge, RuntimeError> {
    graph
        .edges
        .iter()
        .find(|edge| {
            edge.from == from && edge.requested_target.unwrap_or(edge.to) == requested_target_node
        })
        .ok_or(RuntimeError::MusicEdgeNotFound {
            graph_id: graph.id,
            from,
            to: requested_target_node,
        })
}

fn lookup_auto_transition_rule(
    graph: &MusicGraph,
    from: MusicNodeId,
    requested_target_node: MusicNodeId,
) -> Option<&MusicEdge> {
    graph.edges.iter().find(|edge| {
        edge.from == from
            && matches!(edge.trigger, EdgeTrigger::OnComplete)
            && edge
                .requested_target
                .map(|target| target == requested_target_node)
                .unwrap_or(true)
    })
}

fn find_next_matching_cue_in_clip(
    clip: &Clip,
    tag: &str,
    current_position_seconds: f64,
) -> Option<NextCueMatch> {
    let current_position_seconds = if current_position_seconds.is_finite() {
        current_position_seconds.max(0.0)
    } else {
        0.0
    };

    let mut matching_positions: Vec<f64> = clip
        .cues
        .iter()
        .filter(|cue| cue.tags.iter().any(|candidate| candidate.as_str() == tag))
        .map(|cue| cue.position_seconds.max(0.0) as f64)
        .collect();
    matching_positions.sort_by(|left, right| left.total_cmp(right));

    if let Some(position) = matching_positions
        .iter()
        .copied()
        .find(|position| *position >= current_position_seconds)
    {
        return Some(NextCueMatch {
            cue_position_seconds: position,
            requires_wrap: false,
        });
    }

    let first_position = matching_positions.first().copied()?;
    clip.loop_range.as_ref()?;

    Some(NextCueMatch {
        cue_position_seconds: first_position,
        requires_wrap: true,
    })
}
