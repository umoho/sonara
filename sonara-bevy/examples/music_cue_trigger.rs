use bevy::prelude::*;
use sonara_bevy::MusicPhase;
use sonara_bevy::prelude::{SonaraAudio, SonaraFirewheelPlugin};
use sonara_build::CompiledBankPackage;
use sonara_model::{MusicGraphId, MusicNodeId};
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
    intro_node: MusicNodeId,
    warmup_node: MusicNodeId,
    climax_node: MusicNodeId,
    hud_text: String,
    prompt_text: String,
}

impl Default for CueTriggerState {
    fn default() -> Self {
        Self {
            session_id: None,
            graph_id: MusicGraphId::new(),
            intro_node: MusicNodeId::new(),
            warmup_node: MusicNodeId::new(),
            climax_node: MusicNodeId::new(),
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
        CompiledBankPackage::read_json_file("sonara-app/assets/music_demo/cue_trigger.bank.json")
            .expect("cue trigger compiled bank should load from JSON");
    let graph = package
        .music_graphs
        .first()
        .cloned()
        .expect("cue trigger bank should contain a music graph");
    state.graph_id = graph.id;
    state.intro_node = graph
        .nodes
        .iter()
        .find(|music_node| music_node.name == "intro")
        .map(|music_node| music_node.id)
        .expect("cue trigger graph should contain intro node");
    state.warmup_node = graph
        .nodes
        .iter()
        .find(|music_node| music_node.name == "warmup")
        .map(|music_node| music_node.id)
        .expect("cue trigger graph should contain warmup node");
    state.climax_node = graph
        .nodes
        .iter()
        .find(|music_node| music_node.name == "climax")
        .map(|music_node| music_node.id)
        .expect("cue trigger graph should contain climax node");

    audio
        .load_compiled_bank(package)
        .expect("cue trigger compiled bank should load");

    state.session_id = Some(
        audio
            .play_music_graph_in_node(state.graph_id, state.intro_node)
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
                        max_width: px(560.0),
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

    if status.active_node != state.warmup_node || status.desired_target_node != state.warmup_node {
        return;
    }

    audio
        .request_music_node(session_id, state.climax_node)
        .expect("climax node request should succeed");
}

fn restart_demo(audio: &mut SonaraAudio, state: &mut CueTriggerState) {
    if let Some(session_id) = state.session_id.take() {
        audio
            .stop_music_session(session_id, Fade::IMMEDIATE)
            .expect("music session should stop before restart");
    }

    state.session_id = Some(
        audio
            .play_music_graph_in_node(state.graph_id, state.intro_node)
            .expect("music graph should restart in intro"),
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
        MusicPhase::WaitingExitCue => "waiting for a configured exit cue",
        MusicPhase::WaitingNodeCompletion if status.active_node == state.intro_node => {
            "intro is auto-advancing into warmup"
        }
        MusicPhase::WaitingNodeCompletion if status.active_node == state.warmup_node => {
            if status.desired_target_node == state.climax_node {
                "warmup will finish this pass, then enter transition"
            } else {
                "warmup is looping via its self-edge"
            }
        }
        MusicPhase::WaitingNodeCompletion => "transition node is playing",
        MusicPhase::Stable if status.active_node == state.climax_node => "climax active",
        MusicPhase::Stable => "node is stable",
        MusicPhase::Stopped => "session stopped",
        MusicPhase::EnteringDestination => "entering destination",
    };

    state.prompt_text = match status.phase {
        MusicPhase::WaitingNodeCompletion if status.active_node == state.intro_node => {
            if pending_media {
                "Loading intro...\nThe graph will naturally move into warmup once playback starts"
                    .into()
            } else {
                "Intro is playing\nIt will naturally move into warmup when this node completes"
                    .into()
            }
        }
        MusicPhase::WaitingNodeCompletion if status.active_node == state.warmup_node => {
            if pending_media {
                "Warmup is loading...\nOnce ready it will keep looping until you request climax"
                    .into()
            } else if status.desired_target_node == state.climax_node {
                "Climax requested\nWarmup will finish this pass, then enter transition".into()
            } else {
                "Press Space to request climax\nWarmup keeps looping by following its self-edge"
                    .into()
            }
        }
        MusicPhase::WaitingNodeCompletion => {
            "Transition is playing...\nA short stinger is triggered on entry, then climax will enter".into()
        }
        MusicPhase::Stable if status.active_node == state.climax_node => {
            "Climax is active\nIt loops by self-edge; press R to restart from intro".into()
        }
        MusicPhase::Stable => "Current node is stable\nPress R to restart from intro".into(),
        MusicPhase::Stopped => "Session stopped\nPress R to restart the demo".into(),
        _ => "Transition in progress".into(),
    };

    state.hud_text = format!(
        "Sonara music_cue_trigger\n\nGraph: intro -> warmup -> transition -> climax\nSelf-edges: warmup -> warmup, climax -> climax\nPress Space during warmup to request climax\nThe transition node also triggers a stinger on entry\nPress R to restart from intro\n\nactive_node: {}\ndesired_target_node: {}\nphase: {:?}\nhint: {}\nloading_media: {}\nplayhead_seconds: {}",
        state_label(status.active_node, state),
        state_label(status.desired_target_node, state),
        status.phase,
        phase_hint,
        if pending_media { "yes" } else { "no" },
        playhead_seconds
            .map(|seconds| format!("{seconds:.2}"))
            .unwrap_or_else(|| "n/a".into()),
    );
}

fn state_label(node_id: MusicNodeId, state: &CueTriggerState) -> &'static str {
    if node_id == state.intro_node {
        "intro"
    } else if node_id == state.warmup_node {
        "warmup"
    } else if node_id == state.climax_node {
        "climax"
    } else {
        "transition"
    }
}
