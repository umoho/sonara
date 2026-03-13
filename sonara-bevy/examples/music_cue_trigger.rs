use bevy::prelude::*;
use sonara_bevy::MusicPhase;
use sonara_bevy::prelude::{SonaraAudio, SonaraFirewheelPlugin};
use sonara_build::CompiledBankPackage;
use sonara_model::{
    Bank, Clip, CuePoint, EntryPolicy, ExitPolicy, MemoryPolicy, MusicGraph, MusicGraphId,
    MusicStateId, MusicStateNode, PlaybackTarget, TimeRange, TransitionRule,
};
use sonara_runtime::{Fade, MusicSessionId};

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.06, 0.08, 0.11)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Sonara Music Cue Trigger".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(SonaraFirewheelPlugin)
        .insert_resource(CueTriggerState::default())
        .add_systems(Startup, setup_scene)
        .add_systems(Update, handle_demo_input)
        .add_systems(Update, sync_ui_text)
        .run();
}

#[derive(Component)]
struct HudText;

#[derive(Component)]
struct PromptText;

#[derive(Resource)]
struct CueTriggerState {
    session_id: Option<MusicSessionId>,
    graph_id: MusicGraphId,
    preheat_state: MusicStateId,
    combat_state: MusicStateId,
    hud_text: String,
    prompt_text: String,
}

impl Default for CueTriggerState {
    fn default() -> Self {
        Self {
            session_id: None,
            graph_id: MusicGraphId::new(),
            preheat_state: MusicStateId::new(),
            combat_state: MusicStateId::new(),
            hud_text: String::new(),
            prompt_text: String::new(),
        }
    }
}

fn setup_scene(
    mut commands: Commands,
    mut audio: NonSendMut<SonaraAudio>,
    mut state: ResMut<CueTriggerState>,
) {
    let package =
        CompiledBankPackage::read_json_file("sonara-app/assets/music_demo/core.bank.json")
            .expect("music demo compiled bank should load from JSON");
    let manifest = package.bank.manifest.clone();
    let preheat_asset = manifest
        .assets
        .first()
        .cloned()
        .expect("music demo should contain preheat asset");
    let combat_asset = manifest
        .assets
        .get(1)
        .cloned()
        .expect("music demo should contain combat asset");

    let mut preheat_clip = Clip::new("preheat_loop", preheat_asset.id);
    preheat_clip.loop_range = Some(TimeRange::new(0.0, 1.0));
    preheat_clip.cues = battle_ready_cues();

    let bridge_clip = Clip {
        id: sonara_model::ClipId::new(),
        name: "combat_bridge".into(),
        asset_id: combat_asset.id,
        source_range: Some(TimeRange::new(0.0, 3.0)),
        loop_range: None,
        cues: Vec::new(),
        sync_domain: None,
    };

    let mut combat_clip = Clip::new("combat_loop", combat_asset.id);
    combat_clip.loop_range = Some(TimeRange::new(0.0, 1.0));
    let mut combat_entry = CuePoint::new("combat_in", 3.0);
    combat_entry.tags.push("combat_in".into());
    combat_clip.cues.push(combat_entry);

    let mut graph = MusicGraph::new("cue_trigger");
    graph.initial_state = Some(state.preheat_state);
    graph.states.push(MusicStateNode {
        id: state.preheat_state,
        name: "preheat".into(),
        target: PlaybackTarget::Clip {
            clip_id: preheat_clip.id,
        },
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
    });
    graph.states.push(MusicStateNode {
        id: state.combat_state,
        name: "combat".into(),
        target: PlaybackTarget::Clip {
            clip_id: combat_clip.id,
        },
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::EntryCue {
            tag: "combat_in".into(),
        },
    });
    graph.transitions.push(TransitionRule {
        from: state.preheat_state,
        to: state.combat_state,
        exit: ExitPolicy::NextMatchingCue {
            tag: "battle_ready".into(),
        },
        bridge_clip: Some(bridge_clip.id),
        destination: EntryPolicy::EntryCue {
            tag: "combat_in".into(),
        },
    });
    graph.transitions.push(TransitionRule {
        from: state.combat_state,
        to: state.preheat_state,
        exit: ExitPolicy::Immediate,
        bridge_clip: None,
        destination: EntryPolicy::ClipStart,
    });

    state.graph_id = graph.id;

    let mut bank = Bank::new("cue_trigger");
    bank.manifest = manifest;
    bank.objects
        .clips
        .extend([preheat_clip.id, bridge_clip.id, combat_clip.id]);
    bank.objects.music_graphs.push(graph.id);

    audio
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![preheat_clip, bridge_clip, combat_clip],
            Vec::new(),
            Vec::new(),
            vec![graph],
        )
        .expect("cue trigger definitions should load");

    state.session_id = Some(
        audio
            .play_music_graph_in_state(state.graph_id, state.preheat_state)
            .expect("cue trigger music graph should start"),
    );
    refresh_ui_text(&audio, &mut state);

    commands.spawn(Camera2d);

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(16.0),
                top: px(16.0),
                padding: UiRect::axes(px(14.0), px(12.0)),
                border_radius: BorderRadius::all(px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.07, 0.09, 0.86)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(""),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.93, 0.94, 0.97)),
                HudText,
            ));
        });

    commands
        .spawn((Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        max_width: px(520.0),
                        padding: UiRect::axes(px(22.0), px(18.0)),
                        border_radius: BorderRadius::all(px(14.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.05, 0.06, 0.08, 0.82)),
                ))
                .with_children(|prompt| {
                    prompt.spawn((
                        Text::new(""),
                        TextFont {
                            font_size: 28.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.98, 0.97, 0.92)),
                        PromptText,
                    ));
                });
        });
}

fn battle_ready_cues() -> Vec<CuePoint> {
    [4.0_f32, 8.0, 12.0, 16.0]
        .into_iter()
        .enumerate()
        .map(|(index, seconds)| {
            let mut cue = CuePoint::new(format!("battle_ready_{index}"), seconds);
            cue.tags.push("battle_ready".into());
            cue
        })
        .collect()
}

fn handle_demo_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut audio: NonSendMut<SonaraAudio>,
    mut state: ResMut<CueTriggerState>,
) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        restart_demo(&mut audio, &mut state);
        return;
    }

    if !keyboard.just_pressed(KeyCode::Space) {
        return;
    }

    let Some(session_id) = state.session_id else {
        return;
    };
    let Ok(status) = audio.music_status(session_id) else {
        return;
    };

    if status.phase != MusicPhase::Stable {
        return;
    }
    if status.active_state != state.preheat_state || status.desired_state != state.preheat_state {
        return;
    }

    audio
        .request_music_state(session_id, state.combat_state)
        .expect("combat request should succeed");
}

fn restart_demo(audio: &mut SonaraAudio, state: &mut CueTriggerState) {
    if let Some(session_id) = state.session_id.take() {
        audio
            .stop_music_session(session_id, Fade::IMMEDIATE)
            .expect("music session should stop before restart");
    }

    state.session_id = Some(
        audio
            .play_music_graph_in_state(state.graph_id, state.preheat_state)
            .expect("music graph should restart in preheat"),
    );
}

fn sync_ui_text(
    audio: NonSend<SonaraAudio>,
    mut state: ResMut<CueTriggerState>,
    mut hud_query: Query<&mut Text, With<HudText>>,
    mut prompt_query: Query<&mut Text, (With<PromptText>, Without<HudText>)>,
) {
    refresh_ui_text(&audio, &mut state);

    if let Ok(mut hud_text) = hud_query.single_mut() {
        *hud_text = Text::new(state.hud_text.clone());
    }

    if let Ok(mut prompt_text) = prompt_query.single_mut() {
        *prompt_text = Text::new(state.prompt_text.clone());
    }
}

fn refresh_ui_text(audio: &SonaraAudio, state: &mut CueTriggerState) {
    let Some(session_id) = state.session_id else {
        state.hud_text = "cue trigger not started".into();
        state.prompt_text = "Press R to start the cue trigger demo".into();
        return;
    };

    let status = audio
        .music_status(session_id)
        .expect("music status should resolve for cue trigger");
    let pending_media = audio.music_session_pending_media(session_id);
    let playhead_seconds = audio.music_session_playhead_seconds(session_id);

    let phase_hint = match status.phase {
        MusicPhase::WaitingExitCue => "waiting for next battle_ready cue",
        MusicPhase::PlayingBridge => "bridge is playing",
        MusicPhase::Stable if status.active_state == state.combat_state => "combat active",
        MusicPhase::Stable => "preheat active",
        _ => "transitioning",
    };

    state.prompt_text = match status.phase {
        MusicPhase::Stable if status.active_state == state.preheat_state => {
            if pending_media {
                "Loading music resources...\nYou can press Space early; Sonara will wait for the cue once playback starts".into()
            } else {
                "Press Space to trigger combat\nSonara will wait for the next battle_ready cue"
                    .into()
            }
        }
        MusicPhase::WaitingExitCue => {
            if pending_media {
                "Combat requested\nWaiting for music media to become ready...".into()
            } else {
                "Combat requested\nWaiting for the next battle_ready cue...".into()
            }
        }
        MusicPhase::PlayingBridge => {
            "Bridge playing...\nBoss music will enter after this clip".into()
        }
        MusicPhase::Stable if status.active_state == state.combat_state => {
            "Combat is active\nPress R to reset back to preheat".into()
        }
        MusicPhase::Stopped => "Session stopped\nPress R to restart the demo".into(),
        _ => "Transition in progress".into(),
    };

    state.hud_text = format!(
        "Sonara music_cue_trigger\n\nGoal: hear [2] waiting for the next cue before switching\nPress R to reset the session back to preheat\nThis demo intentionally locks the transition once started\n\nactive_state: {}\ndesired_state: {}\nphase: {:?}\nhint: {}\nloading_media: {}\nplayhead_seconds: {}",
        state_label(status.active_state, state),
        state_label(status.desired_state, state),
        status.phase,
        phase_hint,
        if pending_media { "yes" } else { "no" },
        playhead_seconds
            .map(|seconds| format!("{seconds:.2}"))
            .unwrap_or_else(|| "n/a".into()),
    );
}

fn state_label(state_id: MusicStateId, state: &CueTriggerState) -> &'static str {
    if state_id == state.preheat_state {
        "preheat"
    } else if state_id == state.combat_state {
        "combat"
    } else {
        "unknown"
    }
}
