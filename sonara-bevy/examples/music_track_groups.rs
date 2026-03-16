// SPDX-License-Identifier: MPL-2.0

use bevy::prelude::*;
use sonara_bevy::MusicPhase;
use sonara_bevy::prelude::{SonaraAudio, SonaraFirewheelPlugin};
use sonara_build::CompiledBankPackage;
use sonara_model::{MusicGraphId, MusicNodeId, TrackGroupId};
use sonara_runtime::{Fade, MusicSessionId};

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.05, 0.07, 0.1)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Sonara Music Track Groups".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(SonaraFirewheelPlugin)
        .insert_resource(TrackGroupsDemoState::default())
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
struct TrackGroupsDemoState {
    session_id: Option<MusicSessionId>,
    graph_id: MusicGraphId,
    loop_node: MusicNodeId,
    day_group: TrackGroupId,
    night_group: TrackGroupId,
    energy_group: TrackGroupId,
    hud_text: String,
    prompt_text: String,
}

impl Default for TrackGroupsDemoState {
    fn default() -> Self {
        Self {
            session_id: None,
            graph_id: MusicGraphId::new(),
            loop_node: MusicNodeId::new(),
            day_group: TrackGroupId::new(),
            night_group: TrackGroupId::new(),
            energy_group: TrackGroupId::new(),
            hud_text: String::new(),
            prompt_text: String::new(),
        }
    }
}

fn setup_scene(
    mut commands: Commands,
    mut audio: NonSendMut<SonaraAudio>,
    mut state: ResMut<TrackGroupsDemoState>,
) {
    let package =
        CompiledBankPackage::read_json_file("sonara-app/assets/music_demo/track_groups.bank.json")
            .expect("track groups compiled bank should load from JSON");
    let graph = package
        .music_graphs
        .first()
        .cloned()
        .expect("track groups bank should contain a music graph");
    state.graph_id = graph.id;
    state.loop_node = graph
        .nodes
        .iter()
        .find(|music_node| music_node.name == "shared_loop")
        .map(|music_node| music_node.id)
        .expect("track groups graph should contain shared_loop node");
    state.day_group = graph
        .groups
        .iter()
        .find(|group| group.name == "day_style")
        .map(|group| group.id)
        .expect("track groups graph should contain day_style group");
    state.night_group = graph
        .groups
        .iter()
        .find(|group| group.name == "night_style")
        .map(|group| group.id)
        .expect("track groups graph should contain night_style group");
    state.energy_group = graph
        .groups
        .iter()
        .find(|group| group.name == "energy_layer")
        .map(|group| group.id)
        .expect("track groups graph should contain energy_layer group");

    audio
        .load_compiled_bank(package)
        .expect("track groups compiled bank should load");

    state.session_id = Some(
        audio
            .play_music_graph_in_node(state.graph_id, state.loop_node)
            .expect("track groups graph should start"),
    );
    apply_default_mix(&mut audio, &state);
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
                        max_width: px(640.0),
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

fn apply_default_mix(audio: &mut SonaraAudio, state: &TrackGroupsDemoState) {
    let Some(session_id) = state.session_id else {
        return;
    };

    audio
        .set_music_track_group_active(session_id, state.day_group, true)
        .expect("day group should become the active exclusive style");
    audio
        .set_music_track_group_active(session_id, state.energy_group, false)
        .expect("energy layer should start muted");
}

fn handle_demo_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut audio: NonSendMut<SonaraAudio>,
    mut state: ResMut<TrackGroupsDemoState>,
) {
    let Some(session_id) = state.session_id else {
        return;
    };

    if keyboard.just_pressed(KeyCode::Digit1) {
        audio
            .set_music_track_group_active(session_id, state.day_group, true)
            .expect("day group should become active");
    }

    if keyboard.just_pressed(KeyCode::Digit2) {
        audio
            .set_music_track_group_active(session_id, state.night_group, true)
            .expect("night group should become active");
    }

    if keyboard.just_pressed(KeyCode::Space) {
        let currently_active = audio
            .music_track_group_state(session_id, state.energy_group)
            .expect("energy group state should resolve")
            .active;
        audio
            .set_music_track_group_active(session_id, state.energy_group, !currently_active)
            .expect("energy group should toggle");
    }

    if keyboard.just_pressed(KeyCode::KeyR) {
        if let Some(old_session_id) = state.session_id.take() {
            audio
                .stop_music_session(old_session_id, Fade::IMMEDIATE)
                .expect("music session should stop before restart");
        }
        state.session_id = Some(
            audio
                .play_music_graph_in_node(state.graph_id, state.loop_node)
                .expect("track groups graph should restart"),
        );
        apply_default_mix(&mut audio, &state);
    }
}

fn sync_ui_text(
    audio: NonSend<SonaraAudio>,
    mut state: ResMut<TrackGroupsDemoState>,
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

fn refresh_ui_text(audio: &SonaraAudio, state: &mut TrackGroupsDemoState) {
    let Some(session_id) = state.session_id else {
        state.hud_text = "track groups demo not started".into();
        state.prompt_text = "Press R to start the track groups demo".into();
        return;
    };

    let status = audio
        .music_status(session_id)
        .expect("music status should resolve for track groups demo");
    let pending_media = audio.music_session_pending_media(session_id);
    let playhead_seconds = audio.music_session_playhead_seconds(session_id);
    let day_active = audio
        .music_track_group_state(session_id, state.day_group)
        .expect("day group state should resolve")
        .active;
    let night_active = audio
        .music_track_group_state(session_id, state.night_group)
        .expect("night group state should resolve")
        .active;
    let energy_active = audio
        .music_track_group_state(session_id, state.energy_group)
        .expect("energy group state should resolve")
        .active;

    let phase_hint = match status.phase {
        MusicPhase::WaitingNodeCompletion => "shared loop is advancing via its self-edge",
        MusicPhase::Stable => "node is stable",
        MusicPhase::Stopped => "session stopped",
        MusicPhase::WaitingExitCue => "waiting for exit cue",
        MusicPhase::EnteringDestination => "entering destination",
    };

    state.prompt_text = if pending_media {
        "Loading loop variants...\n1 = Day style, 2 = Night style, Space = toggle Energy layer, R = restart".into()
    } else {
        "1 = Day style, 2 = Night style, Space = toggle Energy layer\nGroup changes should preserve the current shared playhead\nR = restart the demo from the beginning".into()
    };

    state.hud_text = format!(
        "Sonara music_track_groups\n\nNode: shared_loop -> shared_loop [OnComplete]\nTracks: day_main, night_main, energy_layer\nGroups: day_style (Exclusive), night_style (Exclusive), energy_layer (Additive)\n\nactive_node: {}\ndesired_target_node: {}\nphase: {:?}\nhint: {}\nday_style: {}\nnight_style: {}\nenergy_layer: {}\nloading_media: {}\nshared_playhead_seconds: {}",
        if status.active_node == state.loop_node {
            "shared_loop"
        } else {
            "unknown"
        },
        if status.desired_target_node == state.loop_node {
            "shared_loop"
        } else {
            "unknown"
        },
        status.phase,
        phase_hint,
        if day_active { "on" } else { "off" },
        if night_active { "on" } else { "off" },
        if energy_active { "on" } else { "off" },
        if pending_media { "yes" } else { "no" },
        playhead_seconds
            .map(|seconds| format!("{seconds:.2}"))
            .unwrap_or_else(|| "n/a".into()),
    );
}
