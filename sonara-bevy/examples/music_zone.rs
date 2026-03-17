// SPDX-License-Identifier: MPL-2.0

use bevy::prelude::*;
use sonara_bevy::{
    AudioRequestResult,
    prelude::{EventInstanceState, SonaraAudio, SonaraFirewheelPlugin},
};
use sonara_build::CompiledBankPackage;
use sonara_model::{EventId, ParameterId, ParameterValue};
use sonara_runtime::{EventInstanceId, Fade};

const FLOOR_SIZE: f32 = 22.0;
const WALK_SPEED: f32 = 5.0;
const PLAYER_HEIGHT: f32 = 0.6;
const COMBAT_RADIUS: f32 = 4.2;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Sonara Music Zone".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(SonaraFirewheelPlugin)
        .insert_non_send_resource(read_demo_ids())
        .insert_resource(MusicZoneState::default())
        .add_systems(Startup, setup_scene)
        .add_systems(Update, move_player)
        .add_systems(Update, update_music_zone)
        .add_systems(Update, update_music_loading_state)
        .add_systems(Update, sync_hud_text)
        .run();
}

struct DemoIds {
    event_id: EventId,
    music_state_id: ParameterId,
}

fn read_demo_ids() -> DemoIds {
    let package =
        CompiledBankPackage::read_json_file("sonara-app/assets/music_demo/core.bank.json")
            .expect("music demo compiled bank should load from JSON");
    let event = package
        .events()
        .iter()
        .find(|event| event.name == "music.play")
        .expect("compiled bank should contain music.play");

    DemoIds {
        event_id: event.id,
        music_state_id: *event
            .default_parameters
            .first()
            .expect("music.play should reference music_state"),
    }
}

#[derive(Component)]
struct Player;

#[derive(Component)]
struct HudText;

#[derive(Resource)]
struct MusicZoneState {
    current_state: &'static str,
    current_instance_id: Option<EventInstanceId>,
    inside_zone: bool,
    playback_status: &'static str,
    hud_text: String,
}

impl Default for MusicZoneState {
    fn default() -> Self {
        Self {
            current_state: "explore",
            current_instance_id: None,
            inside_zone: false,
            playback_status: "idle",
            hud_text: String::new(),
        }
    }
}

fn setup_scene(
    mut commands: Commands,
    mut audio: NonSendMut<SonaraAudio>,
    demo_ids: NonSend<DemoIds>,
    mut state: ResMut<MusicZoneState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let package =
        CompiledBankPackage::read_json_file("sonara-app/assets/music_demo/core.bank.json")
            .expect("music demo compiled bank should load from JSON");
    audio
        .load_compiled_bank(package)
        .expect("compiled bank should load in startup system");

    let results = {
        let mut update = audio.begin_update();
        update.set_global_param(
            demo_ids.music_state_id,
            ParameterValue::Enum("explore".into()),
        );
        update.play(demo_ids.event_id);
        update.apply().expect("initial music update should apply")
    };
    state.current_instance_id = results.iter().find_map(|result| match result {
        AudioRequestResult::Played { instance_id } => Some(*instance_id),
        AudioRequestResult::ParameterSet | AudioRequestResult::Stopped { .. } => None,
    });
    state.current_state = "explore";
    state.inside_zone = false;
    state.playback_status = "loading";
    refresh_hud_text(&mut state);

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
            base_color: Color::srgb(0.18, 0.42, 0.92),
            perceptual_roughness: 0.82,
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
            BackgroundColor(Color::srgba(0.06, 0.07, 0.09, 0.84)),
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
    mut audio: NonSendMut<SonaraAudio>,
    demo_ids: NonSend<DemoIds>,
    mut state: ResMut<MusicZoneState>,
    player_query: Query<&Transform, With<Player>>,
) {
    let Ok(transform) = player_query.single() else {
        return;
    };

    let inside_zone = transform.translation.xz().length() <= COMBAT_RADIUS;
    if inside_zone == state.inside_zone {
        return;
    }

    state.inside_zone = inside_zone;
    let next_state = if inside_zone { "combat" } else { "explore" };
    let results = {
        let mut update = audio.begin_update();
        update.set_global_param(
            demo_ids.music_state_id,
            ParameterValue::Enum(next_state.into()),
        );
        if let Some(instance_id) = state.current_instance_id {
            update.stop(instance_id, Fade::IMMEDIATE);
        }
        update.play(demo_ids.event_id);
        update.apply().expect("music zone update should apply")
    };

    state.current_instance_id = results.iter().find_map(|result| match result {
        AudioRequestResult::Played { instance_id } => Some(*instance_id),
        AudioRequestResult::ParameterSet | AudioRequestResult::Stopped { .. } => None,
    });
    state.current_state = next_state;
    state.playback_status = "loading";
    refresh_hud_text(&mut state);
}

fn update_music_loading_state(audio: NonSend<SonaraAudio>, mut state: ResMut<MusicZoneState>) {
    let next_status = match state.current_instance_id {
        Some(instance_id) => match audio.instance_state(instance_id) {
            EventInstanceState::PendingMedia => "loading",
            EventInstanceState::Playing => "playing",
            EventInstanceState::Stopped => "stopped",
        },
        None => "idle",
    };

    if next_status == state.playback_status {
        return;
    }

    state.playback_status = next_status;
    refresh_hud_text(&mut state);
}

fn refresh_hud_text(state: &mut MusicZoneState) {
    state.hud_text = format!(
        "Sonara music_zone\ncompiled bank file: sonara-app/assets/music_demo/core.bank.json\nWASD or arrow keys move the blue sphere\nenter the red circle -> music_state=combat\noutside the circle -> music_state=explore\ninside zone: {}\ncurrent music_state: {}\nplayback status: {}\ncurrent instance: {:?}",
        state.inside_zone, state.current_state, state.playback_status, state.current_instance_id
    );
}

fn sync_hud_text(state: Res<MusicZoneState>, mut hud_query: Query<&mut Text, With<HudText>>) {
    if !state.is_changed() {
        return;
    }

    let Ok(mut text) = hud_query.single_mut() else {
        return;
    };

    *text = Text::new(state.hud_text.clone());
}
