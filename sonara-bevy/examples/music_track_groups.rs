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
        .add_systems(Update, handle_card_clicks)
        .add_systems(Update, sync_ui_text)
        .run();
}

#[derive(Component)]
struct HudText;

#[derive(Component)]
struct TrackCard(TrackCardKind);

#[derive(Clone, Copy, PartialEq, Eq)]
enum TrackCardKind {
    NoBassOrHigh,
    NoBeachSfx,
    BeachSfx,
}

#[derive(Resource)]
struct TrackGroupsDemoState {
    session_id: Option<MusicSessionId>,
    graph_id: MusicGraphId,
    loop_node: MusicNodeId,
    no_bass_or_high_group: TrackGroupId,
    no_beach_sfx_group: TrackGroupId,
    beach_sfx_group: TrackGroupId,
    hud_text: String,
}

impl Default for TrackGroupsDemoState {
    fn default() -> Self {
        Self {
            session_id: None,
            graph_id: MusicGraphId::new(),
            loop_node: MusicNodeId::new(),
            no_bass_or_high_group: TrackGroupId::new(),
            no_beach_sfx_group: TrackGroupId::new(),
            beach_sfx_group: TrackGroupId::new(),
            hud_text: String::new(),
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
    state.no_bass_or_high_group = graph
        .groups
        .iter()
        .find(|group| group.name == "no_bass_or_high")
        .map(|group| group.id)
        .expect("track groups graph should contain no_bass_or_high group");
    state.no_beach_sfx_group = graph
        .groups
        .iter()
        .find(|group| group.name == "no_beach_sfx")
        .map(|group| group.id)
        .expect("track groups graph should contain no_beach_sfx group");
    state.beach_sfx_group = graph
        .groups
        .iter()
        .find(|group| group.name == "beach_sfx")
        .map(|group| group.id)
        .expect("track groups graph should contain beach_sfx group");

    audio
        .load_compiled_bank(package)
        .expect("track groups compiled bank should load");

    state.session_id = Some(
        audio
            .play_music_graph_in_node(state.graph_id, state.loop_node)
            .expect("track groups graph should start"),
    );
    apply_default_mix(&mut audio, &state);
    refresh_hud_text(&audio, &mut state);

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
            BackgroundColor(Color::srgba(0.04, 0.05, 0.07, 0.54)),
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
                .spawn((Node {
                    width: px(664.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: px(24.0),
                    ..default()
                },))
                .with_children(|layout| {
                    layout
                        .spawn((Node {
                            width: px(664.0),
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            ..default()
                        },))
                        .with_children(|row| {
                            spawn_track_card(
                                row,
                                TrackCardKind::NoBassOrHigh,
                                "No Bass or High\nPress [1]",
                                px(320.0),
                                px(180.0),
                            );
                            spawn_track_card(
                                row,
                                TrackCardKind::NoBeachSfx,
                                "No Beach SFX\nPress [2]",
                                px(320.0),
                                px(180.0),
                            );
                        });

                    layout
                        .spawn((Node {
                            width: px(664.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },))
                        .with_children(|row| {
                            spawn_track_card(
                                row,
                                TrackCardKind::BeachSfx,
                                "Beach SFX\nPress [Space]",
                                px(664.0),
                                px(112.0),
                            );
                        });
                });
        });

    commands
        .spawn((Node {
            position_type: PositionType::Absolute,
            right: px(16.0),
            bottom: px(16.0),
            width: px(520.0),
            ..default()
        },))
        .with_children(|parent| {
            parent.spawn((
                Text::new(
                    "COVE OF SAND & SNOW\nVoltz Supreme\nhttps://voltzsupreme.itch.io/jrpg-moods",
                ),
                TextFont {
                    font_size: 17.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.92, 0.95)),
                TextLayout::new_with_justify(Justify::Right),
            ));
        });
}

fn apply_default_mix(audio: &mut SonaraAudio, state: &TrackGroupsDemoState) {
    let Some(session_id) = state.session_id else {
        return;
    };

    audio
        .set_music_track_group_active(session_id, state.no_beach_sfx_group, true)
        .expect("no_beach_sfx group should become the active exclusive style");
    audio
        .set_music_track_group_active(session_id, state.beach_sfx_group, false)
        .expect("beach_sfx layer should start muted");
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
        activate_no_bass_or_high(&mut audio, &state, session_id);
    }

    if keyboard.just_pressed(KeyCode::Digit2) {
        activate_no_beach_sfx(&mut audio, &state, session_id);
    }

    if keyboard.just_pressed(KeyCode::Space) {
        toggle_beach_sfx(&mut audio, &state, session_id);
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

fn handle_card_clicks(
    mut audio: NonSendMut<SonaraAudio>,
    state: Res<TrackGroupsDemoState>,
    interaction_query: Query<(&TrackCard, &Interaction), (Changed<Interaction>, With<Button>)>,
) {
    let Some(session_id) = state.session_id else {
        return;
    };

    for (track_card, interaction) in &interaction_query {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match track_card.0 {
            TrackCardKind::NoBassOrHigh => {
                activate_no_bass_or_high(&mut audio, &state, session_id);
            }
            TrackCardKind::NoBeachSfx => {
                activate_no_beach_sfx(&mut audio, &state, session_id);
            }
            TrackCardKind::BeachSfx => {
                toggle_beach_sfx(&mut audio, &state, session_id);
            }
        }
    }
}

fn sync_ui_text(
    audio: NonSend<SonaraAudio>,
    mut state: ResMut<TrackGroupsDemoState>,
    mut hud_query: Query<&mut Text, With<HudText>>,
    mut card_query: Query<(&TrackCard, &mut BackgroundColor)>,
) {
    refresh_hud_text(&audio, &mut state);

    if let Ok(mut hud_text) = hud_query.single_mut() {
        *hud_text = Text::new(state.hud_text.clone());
    }

    let Some(session_id) = state.session_id else {
        return;
    };

    let no_bass_or_high_active = audio
        .music_track_group_state(session_id, state.no_bass_or_high_group)
        .expect("no_bass_or_high group state should resolve")
        .active;
    let no_beach_sfx_active = audio
        .music_track_group_state(session_id, state.no_beach_sfx_group)
        .expect("no_beach_sfx group state should resolve")
        .active;
    let beach_sfx_active = audio
        .music_track_group_state(session_id, state.beach_sfx_group)
        .expect("beach_sfx group state should resolve")
        .active;

    for (track_card, mut background) in &mut card_query {
        let active = match track_card.0 {
            TrackCardKind::NoBassOrHigh => no_bass_or_high_active,
            TrackCardKind::NoBeachSfx => no_beach_sfx_active,
            TrackCardKind::BeachSfx => beach_sfx_active,
        };
        *background = BackgroundColor(track_card_color(track_card.0, active));
    }
}

fn refresh_hud_text(audio: &SonaraAudio, state: &mut TrackGroupsDemoState) {
    let Some(session_id) = state.session_id else {
        state.hud_text = "track groups demo not started".into();
        return;
    };

    let status = audio
        .music_status(session_id)
        .expect("music status should resolve for track groups demo");
    let pending_media = audio.music_session_pending_media(session_id);
    let playhead_seconds = audio.music_session_playhead_seconds(session_id);
    let no_bass_or_high_active = audio
        .music_track_group_state(session_id, state.no_bass_or_high_group)
        .expect("no_bass_or_high group state should resolve")
        .active;
    let no_beach_sfx_active = audio
        .music_track_group_state(session_id, state.no_beach_sfx_group)
        .expect("no_beach_sfx group state should resolve")
        .active;
    let beach_sfx_active = audio
        .music_track_group_state(session_id, state.beach_sfx_group)
        .expect("beach_sfx group state should resolve")
        .active;

    let phase_hint = match status.phase {
        MusicPhase::WaitingNodeCompletion => "shared loop is advancing via its self-edge",
        MusicPhase::Stable => "node is stable",
        MusicPhase::Stopped => "session stopped",
        MusicPhase::WaitingExitCue => "waiting for exit cue",
        MusicPhase::EnteringDestination => "entering destination",
    };

    state.hud_text = format!(
        "Sonara music_track_groups\n\nNode: shared_loop -> shared_loop [OnComplete]\nTracks: no_bass_or_high, no_beach_sfx, beach_sfx\nGroups: no_bass_or_high (Exclusive), no_beach_sfx (Exclusive), beach_sfx (Additive)\n\nactive_node: {}\ndesired_target_node: {}\nphase: {:?}\nhint: {}\nno_bass_or_high: {}\nno_beach_sfx: {}\nbeach_sfx: {}\nloading_media: {}\nshared_playhead_seconds: {}",
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
        if no_bass_or_high_active { "on" } else { "off" },
        if no_beach_sfx_active { "on" } else { "off" },
        if beach_sfx_active { "on" } else { "off" },
        if pending_media { "yes" } else { "no" },
        playhead_seconds
            .map(|seconds| format!("{seconds:.2}"))
            .unwrap_or_else(|| "n/a".into()),
    );
}

fn activate_no_bass_or_high(
    audio: &mut SonaraAudio,
    state: &TrackGroupsDemoState,
    session_id: MusicSessionId,
) {
    audio
        .set_music_track_group_active(session_id, state.no_bass_or_high_group, true)
        .expect("no_bass_or_high group should become active");
}

fn activate_no_beach_sfx(
    audio: &mut SonaraAudio,
    state: &TrackGroupsDemoState,
    session_id: MusicSessionId,
) {
    audio
        .set_music_track_group_active(session_id, state.no_beach_sfx_group, true)
        .expect("no_beach_sfx group should become active");
}

fn toggle_beach_sfx(
    audio: &mut SonaraAudio,
    state: &TrackGroupsDemoState,
    session_id: MusicSessionId,
) {
    let currently_active = audio
        .music_track_group_state(session_id, state.beach_sfx_group)
        .expect("beach_sfx group state should resolve")
        .active;
    audio
        .set_music_track_group_active(session_id, state.beach_sfx_group, !currently_active)
        .expect("beach_sfx group should toggle");
}

fn spawn_track_card(
    parent: &mut ChildSpawnerCommands,
    kind: TrackCardKind,
    label: &'static str,
    width: Val,
    height: Val,
) {
    parent
        .spawn((
            Node {
                width,
                height,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::axes(px(18.0), px(14.0)),
                border_radius: BorderRadius::all(px(24.0)),
                ..default()
            },
            Button,
            BackgroundColor(track_card_color(kind, false)),
            TrackCard(kind),
        ))
        .with_children(|card| {
            card.spawn((
                Text::new(label),
                TextFont {
                    font_size: 28.0,
                    ..default()
                },
                TextColor(Color::srgb(0.98, 0.98, 0.97)),
                TextLayout::new_with_justify(Justify::Center),
            ));
        });
}

fn track_card_color(kind: TrackCardKind, active: bool) -> Color {
    match (kind, active) {
        (TrackCardKind::NoBassOrHigh, true) => Color::srgb(0.87, 0.71, 0.42),
        (TrackCardKind::NoBassOrHigh, false) => Color::srgba(0.33, 0.26, 0.16, 0.55),
        (TrackCardKind::NoBeachSfx, true) => Color::srgb(0.33, 0.55, 0.82),
        (TrackCardKind::NoBeachSfx, false) => Color::srgba(0.14, 0.22, 0.34, 0.55),
        (TrackCardKind::BeachSfx, true) => Color::srgb(0.23, 0.74, 0.65),
        (TrackCardKind::BeachSfx, false) => Color::srgba(0.12, 0.28, 0.25, 0.52),
    }
}
