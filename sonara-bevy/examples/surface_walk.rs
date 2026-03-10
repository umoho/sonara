use bevy::prelude::*;
use camino::Utf8PathBuf;
use sonara_bevy::{
    AudioRequestResult,
    prelude::{AudioEmitter, SonaraAudio, SonaraFirewheelPlugin},
};
use sonara_build::build_bank;
use sonara_model::{
    AudioAsset, Event, EventContentNode, EventContentRoot, EventId, EventKind, NodeId, NodeRef,
    ParameterId, ParameterValue, SamplerNode, SpatialMode, SwitchCase, SwitchNode,
};
use uuid::Uuid;

const TILE_SIZE: f32 = 2.2;
const TILE_GAP: f32 = 0.12;
const WALK_SEGMENT_SECONDS: f32 = 0.7;

fn main() {
    let event_id = EventId::new();
    let surface_id = ParameterId::new();
    let wood_asset = Uuid::now_v7();
    let stone_asset = Uuid::now_v7();

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Sonara Surface Walk".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(SonaraFirewheelPlugin)
        .insert_non_send_resource(DemoIds {
            event_id,
            surface_id,
            wood_asset,
            stone_asset,
        })
        .insert_resource(SurfaceStrip::default())
        .add_systems(Startup, setup_scene)
        .add_systems(Update, animate_walker)
        .run();
}

struct DemoIds {
    event_id: EventId,
    surface_id: ParameterId,
    wood_asset: Uuid,
    stone_asset: Uuid,
}

#[derive(Resource)]
struct SurfaceStrip {
    tiles: Vec<SurfaceKind>,
}

impl Default for SurfaceStrip {
    fn default() -> Self {
        Self {
            tiles: vec![
                SurfaceKind::Wood,
                SurfaceKind::Wood,
                SurfaceKind::Stone,
                SurfaceKind::Stone,
                SurfaceKind::Wood,
                SurfaceKind::Stone,
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SurfaceKind {
    Wood,
    Stone,
}

impl SurfaceKind {
    fn color(self) -> Color {
        match self {
            Self::Wood => Color::srgb(0.55, 0.35, 0.18),
            Self::Stone => Color::srgb(0.48, 0.52, 0.58),
        }
    }

    fn variant(self) -> &'static str {
        match self {
            Self::Wood => "wood",
            Self::Stone => "stone",
        }
    }
}

#[derive(Component)]
struct Walker {
    path: Vec<usize>,
    segment_index: usize,
    segment_progress: f32,
}

fn setup_scene(
    mut commands: Commands,
    mut audio: NonSendMut<SonaraAudio>,
    demo_ids: NonSend<DemoIds>,
    surface_strip: Res<SurfaceStrip>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let switch_id = NodeId::new();
    let wood_node_id = NodeId::new();
    let stone_node_id = NodeId::new();
    let wood_path = Utf8PathBuf::from("sonara-app/assets/demo/footstep_wood.wav");
    let stone_path = Utf8PathBuf::from("sonara-app/assets/demo/footstep_stone.wav");

    let mut wood_audio_asset = AudioAsset::new("footstep_wood", wood_path);
    wood_audio_asset.id = demo_ids.wood_asset;
    let mut stone_audio_asset = AudioAsset::new("footstep_stone", stone_path);
    stone_audio_asset.id = demo_ids.stone_asset;

    let event = Event {
        id: demo_ids.event_id,
        name: "player.footstep".into(),
        kind: EventKind::OneShot,
        root: EventContentRoot {
            root: NodeRef { id: switch_id },
            nodes: vec![
                EventContentNode::Switch(SwitchNode {
                    id: switch_id,
                    parameter_id: demo_ids.surface_id,
                    cases: vec![
                        SwitchCase {
                            variant: "wood".into(),
                            child: NodeRef { id: wood_node_id },
                        },
                        SwitchCase {
                            variant: "stone".into(),
                            child: NodeRef { id: stone_node_id },
                        },
                    ],
                    default_case: Some(NodeRef { id: wood_node_id }),
                }),
                EventContentNode::Sampler(SamplerNode {
                    id: wood_node_id,
                    asset_id: demo_ids.wood_asset,
                }),
                EventContentNode::Sampler(SamplerNode {
                    id: stone_node_id,
                    asset_id: demo_ids.stone_asset,
                }),
            ],
        },
        default_bus: None,
        spatial: SpatialMode::ThreeD,
        default_parameters: Vec::new(),
        voice_limit: None,
        steal_policy: None,
    };

    let bank = build_bank(
        "core",
        &[event.clone()],
        &[wood_audio_asset.clone(), stone_audio_asset.clone()],
    )
    .expect("bank should build");
    audio
        .load_bank(bank, vec![event])
        .expect("bank should load in startup system");

    let tile_mesh = meshes.add(Cuboid::new(TILE_SIZE, 0.2, TILE_SIZE));
    for (index, surface) in surface_strip.tiles.iter().copied().enumerate() {
        commands.spawn((
            Mesh3d(tile_mesh.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: surface.color(),
                perceptual_roughness: 1.0,
                ..default()
            })),
            Transform::from_xyz(tile_center_x(index), 0.0, 0.0),
        ));
    }

    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.42).mesh().uv(32, 18))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.72, 0.92),
            perceptual_roughness: 0.35,
            metallic: 0.05,
            ..default()
        })),
        Transform::from_xyz(tile_center_x(0), 0.55, 0.0),
        Walker {
            path: (0..surface_strip.tiles.len()).collect(),
            segment_index: 0,
            segment_progress: 0.0,
        },
        AudioEmitter::default(),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(3.2, 7.8, 11.5).looking_at(Vec3::new(5.2, 0.0, 0.0), Vec3::Y),
    ));

    commands.spawn((
        PointLight {
            intensity: 120_000.0,
            shadows_enabled: true,
            range: 40.0,
            ..default()
        },
        Transform::from_xyz(5.0, 10.0, 6.0),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 11_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.8, 0.0)),
    ));

    println!("Sonara surface_walk");
    println!("brown tiles = wood, gray tiles = stone");
    println!("the blue sphere walks across the strip and triggers matching footsteps");
}

fn animate_walker(
    time: Res<Time>,
    mut audio: NonSendMut<SonaraAudio>,
    demo_ids: NonSend<DemoIds>,
    surface_strip: Res<SurfaceStrip>,
    mut walker_query: Query<(&mut Transform, &mut Walker, &mut AudioEmitter)>,
) {
    let Ok((mut transform, mut walker, mut emitter)) = walker_query.single_mut() else {
        return;
    };

    if walker.segment_index >= walker.path.len().saturating_sub(1) {
        return;
    }

    walker.segment_progress += time.delta_secs() / WALK_SEGMENT_SECONDS;

    while walker.segment_progress >= 1.0 {
        walker.segment_progress -= 1.0;
        walker.segment_index += 1;
        let next_tile = walker.path[walker.segment_index];
        let surface = surface_strip.tiles[next_tile];

        let results = {
            let mut update = audio.begin_update();
            update.set_emitter_param_on(
                &mut emitter,
                demo_ids.surface_id,
                ParameterValue::Enum(surface.variant().into()),
            );
            update.play_from_emitter(&mut emitter, demo_ids.event_id);
            update.apply().expect("footstep update should apply")
        };

        let instance_id = match results.last() {
            Some(AudioRequestResult::Played { instance_id }) => *instance_id,
            other => panic!("expected played result, got {other:?}"),
        };
        let plan = audio
            .active_plan(instance_id)
            .expect("plan should exist after footstep play");

        println!(
            "tile {} surface={} resolved_assets={:?}",
            next_tile,
            surface.variant(),
            plan.asset_ids
        );
    }

    let current_tile = walker.path[walker.segment_index];
    let next_tile = walker
        .path
        .get(walker.segment_index + 1)
        .copied()
        .unwrap_or(current_tile);
    let current_x = tile_center_x(current_tile);
    let next_x = tile_center_x(next_tile);
    transform.translation.x = current_x + (next_x - current_x) * walker.segment_progress;
    transform.translation.y = 0.55 + walker.segment_progress.sin().abs() * 0.08;
}

fn tile_center_x(index: usize) -> f32 {
    index as f32 * (TILE_SIZE + TILE_GAP)
}
