// SPDX-License-Identifier: MPL-2.0

use bevy::prelude::*;
use sonara_bevy::prelude::{SonaraAudio, SonaraFirewheelPlugin};
use sonara_build::CompiledBankPackage;
use sonara_model::{
    Bank, Clip, EdgeTrigger, EntryPolicy, MemoryPolicy, MusicEdge, MusicGraph, MusicNode,
    MusicNodeId, PlaybackTarget, ResumeSlot, Track, TrackBinding, TrackRole,
};
use sonara_runtime::MusicSessionId;

const FLOOR_SIZE: f32 = 22.0;
const WALK_SPEED: f32 = 5.0;
const PLAYER_HEIGHT: f32 = 0.6;
const COMBAT_RADIUS: f32 = 4.2;
const RESUME_TTL_SECONDS: f32 = 8.0;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Sonara Music Resume".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(SonaraFirewheelPlugin)
        .insert_resource(ResumeZoneState::default())
        .add_systems(Startup, setup_scene)
        .add_systems(Update, move_player)
        .add_systems(Update, update_music_zone)
        .add_systems(Update, sync_hud_text)
        .run();
}

#[derive(Component)]
struct Player;

#[derive(Component)]
struct HudText;

#[derive(Resource)]
struct ResumeZoneState {
    session_id: Option<MusicSessionId>,
    explore_state: MusicNodeId,
    combat_state: MusicNodeId,
    inside_zone: bool,
    explore_left_at: Option<f64>,
    combat_left_at: Option<f64>,
    hud_text: String,
}

impl Default for ResumeZoneState {
    fn default() -> Self {
        Self {
            session_id: None,
            explore_state: MusicNodeId::new(),
            combat_state: MusicNodeId::new(),
            inside_zone: false,
            explore_left_at: None,
            combat_left_at: None,
            hud_text: String::new(),
        }
    }
}

fn setup_scene(
    mut commands: Commands,
    mut audio: NonSendMut<SonaraAudio>,
    mut state: ResMut<ResumeZoneState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let package =
        CompiledBankPackage::read_json_file("sonara-app/assets/music_demo/core.bank.json")
            .expect("music demo compiled bank should load from JSON");
    let manifest = package.bank.manifest.clone();
    let explore_asset = manifest
        .assets
        .first()
        .cloned()
        .expect("music demo should contain explore asset");
    let combat_asset = manifest
        .assets
        .get(1)
        .cloned()
        .expect("music demo should contain combat asset");

    let explore_clip = Clip::new("explore_main", explore_asset.id);
    let combat_clip = Clip::new("combat_main", combat_asset.id);
    let explore_slot = ResumeSlot::new("explore_memory");
    let combat_slot = ResumeSlot::new("combat_memory");
    let main_track = Track::new("music_main", TrackRole::Main);
    let mut graph = MusicGraph::new("resume_demo");
    graph.initial_node = Some(state.explore_state);
    graph.tracks.push(main_track.clone());
    graph.nodes.push(MusicNode {
        id: state.explore_state,
        name: "explore".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip {
                clip_id: explore_clip.id,
            },
        }],
        memory_slot: Some(explore_slot.id),
        memory_policy: MemoryPolicy {
            ttl_seconds: Some(RESUME_TTL_SECONDS),
            reset_to: EntryPolicy::ClipStart,
        },
        default_entry: EntryPolicy::Resume,
        externally_targetable: true,
        completion_source: None,
    });
    graph.nodes.push(MusicNode {
        id: state.combat_state,
        name: "combat".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip {
                clip_id: combat_clip.id,
            },
        }],
        memory_slot: Some(combat_slot.id),
        memory_policy: MemoryPolicy {
            ttl_seconds: Some(RESUME_TTL_SECONDS),
            reset_to: EntryPolicy::ClipStart,
        },
        default_entry: EntryPolicy::Resume,
        externally_targetable: true,
        completion_source: None,
    });
    graph.edges.push(MusicEdge {
        from: state.explore_state,
        to: state.combat_state,
        requested_target: None,
        trigger: EdgeTrigger::Immediate,
        destination: EntryPolicy::Resume,
    });
    graph.edges.push(MusicEdge {
        from: state.combat_state,
        to: state.explore_state,
        requested_target: None,
        trigger: EdgeTrigger::Immediate,
        destination: EntryPolicy::Resume,
    });

    let graph_id = graph.id;
    let mut bank = Bank::new("resume_demo");
    bank.manifest = manifest;
    bank.objects.clips.extend([explore_clip.id, combat_clip.id]);
    bank.objects
        .resume_slots
        .extend([explore_slot.id, combat_slot.id]);
    bank.objects.music_graphs.push(graph_id);

    audio
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![explore_clip, combat_clip],
            vec![explore_slot, combat_slot],
            Vec::new(),
            vec![graph],
        )
        .expect("resume demo definitions should load");

    state.session_id = Some(
        audio
            .play_music_graph(graph_id)
            .expect("resume demo music graph should start"),
    );
    state.inside_zone = false;
    refresh_hud_text(&audio, &mut state, 0.0);

    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(FLOOR_SIZE, FLOOR_SIZE))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.24, 0.58, 0.24),
            perceptual_roughness: 0.96,
            ..default()
        })),
        Transform::default(),
    ));

    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(COMBAT_RADIUS, 0.06).mesh().resolution(64))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.82, 0.22, 0.22),
            emissive: Color::srgb(0.45, 0.05, 0.05).into(),
            perceptual_roughness: 0.78,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.04, 0.0),
    ));

    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.48).mesh().uv(32, 18))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.72, 0.92),
            perceptual_roughness: 0.35,
            metallic: 0.05,
            ..default()
        })),
        Transform::from_xyz(-7.0, PLAYER_HEIGHT, 0.0),
        Player,
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-1.0, 11.5, 13.5).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        PointLight {
            intensity: 180_000.0,
            shadows_enabled: true,
            range: 60.0,
            color: Color::srgb(1.0, 0.94, 0.84),
            ..default()
        },
        Transform::from_xyz(5.5, 13.0, 5.5),
    ));

    commands.spawn((
        PointLight {
            intensity: 70_000.0,
            shadows_enabled: false,
            range: 45.0,
            color: Color::srgb(0.72, 0.86, 1.0),
            ..default()
        },
        Transform::from_xyz(-6.0, 7.5, -4.0),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 18_000.0,
            shadows_enabled: true,
            color: Color::srgb(1.0, 0.97, 0.9),
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.95, -0.75, 0.0)),
    ));

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
}

fn move_player(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<&mut Transform, With<Player>>,
) {
    let Ok(mut transform) = player_query.single_mut() else {
        return;
    };

    let mut input = Vec2::ZERO;
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        input.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        input.x += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp) {
        input.y -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown) {
        input.y += 1.0;
    }

    if input == Vec2::ZERO {
        return;
    }

    let direction = input.normalize();
    let delta = Vec3::new(direction.x, 0.0, direction.y) * WALK_SPEED * time.delta_secs();
    let mut next_position = transform.translation + delta;
    let half_floor = FLOOR_SIZE * 0.5 - 0.6;
    next_position.x = next_position.x.clamp(-half_floor, half_floor);
    next_position.z = next_position.z.clamp(-half_floor, half_floor);
    next_position.y = PLAYER_HEIGHT;
    transform.translation = next_position;
}

fn update_music_zone(
    time: Res<Time>,
    mut audio: NonSendMut<SonaraAudio>,
    mut state: ResMut<ResumeZoneState>,
    player_query: Query<&Transform, With<Player>>,
) {
    let Some(session_id) = state.session_id else {
        return;
    };
    let Ok(transform) = player_query.single() else {
        return;
    };

    let inside_zone = transform.translation.xz().length() <= COMBAT_RADIUS;
    if inside_zone == state.inside_zone {
        return;
    }

    let status = audio
        .music_status(session_id)
        .expect("music status should resolve for resume demo");
    let current_state = status.active_node;
    let now_seconds = time.elapsed_secs_f64();

    if current_state == state.explore_state {
        state.explore_left_at = Some(now_seconds);
    } else if current_state == state.combat_state {
        state.combat_left_at = Some(now_seconds);
    }

    let target_node = if inside_zone {
        state.combat_state
    } else {
        state.explore_state
    };
    audio
        .request_music_node(session_id, target_node)
        .expect("music node request should succeed");

    state.inside_zone = inside_zone;
}

fn sync_hud_text(
    time: Res<Time>,
    audio: NonSend<SonaraAudio>,
    mut state: ResMut<ResumeZoneState>,
    mut hud_query: Query<&mut Text, With<HudText>>,
) {
    refresh_hud_text(&audio, &mut state, time.elapsed_secs_f64());

    let Ok(mut text) = hud_query.single_mut() else {
        return;
    };

    *text = Text::new(state.hud_text.clone());
}

fn refresh_hud_text(audio: &SonaraAudio, state: &mut ResumeZoneState, now_seconds: f64) {
    let Some(session_id) = state.session_id else {
        state.hud_text = "resume demo not started".into();
        return;
    };

    let status = audio
        .music_status(session_id)
        .expect("music status should resolve for resume demo");
    let active_label = state_label(status.active_node, state);
    let desired_label = state_label(status.desired_target_node, state);
    let explore_away = away_text(state.explore_left_at, now_seconds);
    let combat_away = away_text(state.combat_left_at, now_seconds);

    state.hud_text = format!(
        "Sonara music_resume\nWASD or arrow keys move the blue sphere\nenter the red circle -> request combat\nleave the red circle -> request explore\n\nGoal: hear per-state resume memory\n- return within {ttl:.0}s: resumes near last exit point\n- stay away longer than {ttl:.0}s: restarts from clip start\n\ninside zone: {inside}\nactive_node: {active}\ndesired_target_node: {desired}\nphase: {phase:?}\nsession: {session:?}\n\nexplore away: {explore_away}\ncombat away: {combat_away}",
        ttl = RESUME_TTL_SECONDS,
        inside = state.inside_zone,
        active = active_label,
        desired = desired_label,
        phase = status.phase,
        session = session_id,
        explore_away = explore_away,
        combat_away = combat_away,
    );
}

fn state_label(state_id: MusicNodeId, state: &ResumeZoneState) -> &'static str {
    if state_id == state.explore_state {
        "explore"
    } else if state_id == state.combat_state {
        "combat"
    } else {
        "unknown"
    }
}

fn away_text(left_at: Option<f64>, now_seconds: f64) -> String {
    match left_at {
        Some(left_at) => format!("{:.1}s", (now_seconds - left_at).max(0.0)),
        None => "never".into(),
    }
}
