// SPDX-License-Identifier: MPL-2.0

use bevy::prelude::*;
use camino::Utf8PathBuf;
use sonara_bevy::prelude::{SonaraAudio, SonaraFirewheelPlugin};
use sonara_model::{
    Bank, BankAsset, Bus, BusEffectSlot, Clip, ImportSettings, MusicGraph, MusicNode, MusicNodeId,
    PlaybackTarget, StreamingMode, TimeRange, Track, TrackBinding, TrackRole,
};
use sonara_runtime::{MusicPhase, MusicSessionId};
use uuid::Uuid;

const FLOOR_SIZE: f32 = 22.0;
const WALK_SPEED: f32 = 5.0;
const PLAYER_HEIGHT: f32 = 0.6;
const WATER_RADIUS: f32 = 4.0;
const UNDERWATER_BUS_GAIN: f32 = 0.74;
const UNDERWATER_CUTOFF_HZ: f32 = 650.0;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Sonara Underwater Music".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(SonaraFirewheelPlugin)
        .insert_resource(build_demo())
        .insert_resource(DemoState::default())
        .add_systems(Startup, setup_scene)
        .add_systems(
            Update,
            (move_player, update_underwater_music, sync_hud_text).chain(),
        )
        .run();
}

#[derive(Resource, Clone)]
struct DemoConfig {
    bank: Bank,
    music_bus: Bus,
    low_pass_slot: BusEffectSlot,
    clip: Clip,
    graph: MusicGraph,
}

fn build_demo() -> DemoConfig {
    let asset_id = Uuid::now_v7();
    let mut bank = Bank::new("underwater_music");
    bank.manifest.assets.push(BankAsset {
        id: asset_id,
        name: "shop_loop".into(),
        source_path: Utf8PathBuf::from("private_assets/underwater/Shop_Loop.wav"),
        import_settings: ImportSettings::default(),
        streaming: StreamingMode::Resident,
    });
    bank.manifest.resident_media.push(asset_id);

    let mut music_bus = Bus::new("music_underwater");
    let mut low_pass_slot = BusEffectSlot::low_pass(UNDERWATER_CUTOFF_HZ);
    low_pass_slot
        .low_pass_effect_mut()
        .expect("slot should be low-pass")
        .enabled = false;
    music_bus.effect_slots.push(low_pass_slot.clone());

    let mut clip = Clip::new("shop_loop", asset_id);
    // Current backend only needs `Some(_)` on a full-asset clip to repeat endlessly.
    clip.loop_range = Some(TimeRange::new(0.0, 60.0));

    let mut track = Track::new("music_main", TrackRole::Main);
    track.output_bus = Some(music_bus.id);

    let node_id = MusicNodeId::new();
    let mut graph = MusicGraph::new("underwater_music");
    graph.initial_node = Some(node_id);
    graph.tracks.push(track.clone());
    graph.nodes.push(MusicNode {
        id: node_id,
        name: "loop".into(),
        bindings: vec![TrackBinding {
            track_id: track.id,
            target: PlaybackTarget::Clip { clip_id: clip.id },
        }],
        memory_slot: None,
        memory_policy: Default::default(),
        default_entry: Default::default(),
        externally_targetable: false,
        completion_source: Some(track.id),
    });

    bank.objects.buses.push(music_bus.id);
    bank.objects.clips.push(clip.id);
    bank.objects.music_graphs.push(graph.id);

    DemoConfig {
        bank,
        music_bus,
        low_pass_slot,
        clip,
        graph,
    }
}

#[derive(Resource, Default)]
struct DemoState {
    session_id: Option<MusicSessionId>,
    inside_water: bool,
    hud_text: String,
}

#[derive(Component)]
struct Player;

#[derive(Component)]
struct HudText;

fn setup_scene(
    mut commands: Commands,
    mut audio: NonSendMut<SonaraAudio>,
    demo: Res<DemoConfig>,
    mut state: ResMut<DemoState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    audio
        .load_bank_with_definitions(
            demo.bank.clone(),
            Vec::new(),
            vec![demo.music_bus.clone()],
            Vec::new(),
            vec![demo.clip.clone()],
            Vec::new(),
            Vec::new(),
            vec![demo.graph.clone()],
        )
        .expect("underwater music definitions should load");
    audio
        .set_bus_gain(demo.music_bus.id, 1.0)
        .expect("dry music bus gain should apply");
    audio
        .set_bus_effect_slot(demo.music_bus.id, demo.low_pass_slot.clone())
        .expect("dry music bus low-pass should apply");

    state.session_id = Some(
        audio
            .play_music_graph(demo.graph.id)
            .expect("underwater music graph should start"),
    );
    refresh_hud_text(&audio, &demo, &mut state);

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
        Mesh3d(meshes.add(Cylinder::new(WATER_RADIUS, 0.06).mesh().resolution(64))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.12, 0.45, 0.82, 0.72),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.2,
            reflectance: 0.05,
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
    let half_extent = FLOOR_SIZE * 0.5 - 0.8;
    let mut next_position = transform.translation + delta;
    next_position.x = next_position.x.clamp(-half_extent, half_extent);
    next_position.z = next_position.z.clamp(-half_extent, half_extent);
    transform.translation = next_position;
}

fn update_underwater_music(
    mut audio: NonSendMut<SonaraAudio>,
    demo: Res<DemoConfig>,
    mut state: ResMut<DemoState>,
    player_query: Query<&Transform, With<Player>>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };

    let inside_water = player_transform.translation.xz().length() <= WATER_RADIUS;
    if inside_water != state.inside_water {
        state.inside_water = inside_water;

        let mut low_pass_slot = demo.low_pass_slot.clone();
        let low_pass = low_pass_slot
            .low_pass_effect_mut()
            .expect("demo low-pass slot should be editable");
        low_pass.enabled = inside_water;
        low_pass.set_cutoff_hz(UNDERWATER_CUTOFF_HZ);

        audio
            .set_bus_gain(
                demo.music_bus.id,
                if inside_water {
                    UNDERWATER_BUS_GAIN
                } else {
                    1.0
                },
            )
            .expect("music bus gain should update");
        audio
            .set_bus_effect_slot(demo.music_bus.id, low_pass_slot)
            .expect("music bus effect slot should update");
    }

    refresh_hud_text(&audio, &demo, &mut state);
}

fn refresh_hud_text(audio: &SonaraAudio, demo: &DemoConfig, state: &mut DemoState) {
    let Some(session_id) = state.session_id else {
        state.hud_text = "music session: none".into();
        return;
    };

    let playhead = audio
        .music_session_playhead_seconds(session_id)
        .unwrap_or(0.0);
    let phase = audio
        .music_status(session_id)
        .map(|status| status.phase)
        .unwrap_or(MusicPhase::Stopped);

    state.hud_text = if state.inside_water {
        format!(
            "Sonara underwater_music\n\nWASD / arrow keys move the listener proxy\ncentral blue disk = underwater zone\nmusic source: private_assets/underwater/Shop_Loop.wav\n\nenvironment: underwater\nmusic bus gain: {UNDERWATER_BUS_GAIN:.2}\nlow-pass: {} Hz\nsession phase: {:?}\nplayhead seconds: {:.2}\nbus id: {:?}",
            UNDERWATER_CUTOFF_HZ as i32, phase, playhead, demo.music_bus.id
        )
    } else {
        format!(
            "Sonara underwater_music\n\nWASD / arrow keys move the listener proxy\ncentral blue disk = underwater zone\nmusic source: private_assets/underwater/Shop_Loop.wav\n\nenvironment: surface\nmusic bus gain: 1.00\nlow-pass: off\nsession phase: {:?}\nplayhead seconds: {:.2}\nbus id: {:?}",
            phase, playhead, demo.music_bus.id
        )
    };
}

fn sync_hud_text(state: Res<DemoState>, mut hud_query: Query<&mut Text, With<HudText>>) {
    if !state.is_changed() {
        return;
    }

    let Ok(mut text) = hud_query.single_mut() else {
        return;
    };

    *text = Text::new(state.hud_text.clone());
}
