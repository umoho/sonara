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
const GRID_WIDTH: usize = 6;
const GRID_DEPTH: usize = 4;
const WALK_SPEED: f32 = 4.5;
const FOOTSTEP_DISTANCE: f32 = 1.35;
const FLOOR_Y: f32 = 0.0;
const WALKER_HEIGHT: f32 = 0.55;

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
        .insert_resource(SurfaceGrid::default())
        .add_systems(Startup, setup_scene)
        .add_systems(Update, control_walker)
        .run();
}

struct DemoIds {
    event_id: EventId,
    surface_id: ParameterId,
    wood_asset: Uuid,
    stone_asset: Uuid,
}

#[derive(Resource)]
struct SurfaceGrid {
    tiles: Vec<SurfaceKind>,
}

impl Default for SurfaceGrid {
    fn default() -> Self {
        let mut tiles = Vec::with_capacity(GRID_WIDTH * GRID_DEPTH);
        for z in 0..GRID_DEPTH {
            for x in 0..GRID_WIDTH {
                let surface = if (x + z) % 2 == 0 {
                    SurfaceKind::Wood
                } else {
                    SurfaceKind::Stone
                };
                tiles.push(surface);
            }
        }
        Self { tiles }
    }
}

impl SurfaceGrid {
    fn tile(&self, x: usize, z: usize) -> SurfaceKind {
        self.tiles[z * GRID_WIDTH + x]
    }

    fn surface_at(&self, position: Vec3) -> Option<(usize, usize, SurfaceKind)> {
        let pitch = TILE_SIZE + TILE_GAP;
        let half_width = (GRID_WIDTH as f32 - 1.0) * pitch * 0.5;
        let half_depth = (GRID_DEPTH as f32 - 1.0) * pitch * 0.5;
        let x = ((position.x + half_width) / pitch).round() as isize;
        let z = ((position.z + half_depth) / pitch).round() as isize;

        if !(0..GRID_WIDTH as isize).contains(&x) || !(0..GRID_DEPTH as isize).contains(&z) {
            return None;
        }

        let x = x as usize;
        let z = z as usize;
        Some((x, z, self.tile(x, z)))
    }

    fn world_bounds(&self) -> (f32, f32) {
        let pitch = TILE_SIZE + TILE_GAP;
        let half_width = (GRID_WIDTH as f32 - 1.0) * pitch * 0.5;
        let half_depth = (GRID_DEPTH as f32 - 1.0) * pitch * 0.5;
        (half_width, half_depth)
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
    distance_since_step: f32,
}

fn setup_scene(
    mut commands: Commands,
    mut audio: NonSendMut<SonaraAudio>,
    demo_ids: NonSend<DemoIds>,
    surface_grid: Res<SurfaceGrid>,
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
    for z in 0..GRID_DEPTH {
        for x in 0..GRID_WIDTH {
            let surface = surface_grid.tile(x, z);
            commands.spawn((
                Mesh3d(tile_mesh.clone()),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: surface.color(),
                    perceptual_roughness: 1.0,
                    ..default()
                })),
                Transform::from_xyz(tile_center_x(x), FLOOR_Y, tile_center_z(z)),
            ));
        }
    }

    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.42).mesh().uv(32, 18))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.72, 0.92),
            perceptual_roughness: 0.35,
            metallic: 0.05,
            ..default()
        })),
        Transform::from_xyz(tile_center_x(0), WALKER_HEIGHT, tile_center_z(0)),
        Walker {
            distance_since_step: 0.0,
        },
        AudioEmitter::default(),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 10.0, 12.0).looking_at(Vec3::new(0.0, 0.0, -0.6), Vec3::Y),
    ));

    commands.spawn((
        PointLight {
            intensity: 120_000.0,
            shadows_enabled: true,
            range: 45.0,
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
    println!("WASD or arrow keys move the blue sphere");
    println!("brown tiles = wood, gray tiles = stone");
    println!("footsteps follow the surface under the sphere");
}

fn control_walker(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut audio: NonSendMut<SonaraAudio>,
    demo_ids: NonSend<DemoIds>,
    surface_grid: Res<SurfaceGrid>,
    mut walker_query: Query<(&mut Transform, &mut Walker, &mut AudioEmitter)>,
) {
    let Ok((mut transform, mut walker, mut emitter)) = walker_query.single_mut() else {
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
        walker.distance_since_step = 0.0;
        transform.translation.y = WALKER_HEIGHT;
        return;
    }

    let direction = input.normalize();
    let delta = Vec3::new(direction.x, 0.0, direction.y) * WALK_SPEED * time.delta_secs();
    let (half_width, half_depth) = surface_grid.world_bounds();
    let mut next_position = transform.translation + delta;
    next_position.x = next_position.x.clamp(-half_width, half_width);
    next_position.z = next_position.z.clamp(-half_depth, half_depth);

    let traveled = transform.translation.distance(next_position);
    if traveled <= f32::EPSILON {
        return;
    }

    transform.translation = next_position;
    walker.distance_since_step += traveled;

    while walker.distance_since_step >= FOOTSTEP_DISTANCE {
        walker.distance_since_step -= FOOTSTEP_DISTANCE;

        let Some((tile_x, tile_z, surface)) = surface_grid.surface_at(transform.translation) else {
            continue;
        };

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
            "tile=({}, {}) surface={} resolved_assets={:?}",
            tile_x,
            tile_z,
            surface.variant(),
            plan.asset_ids
        );
    }

    let step_phase = (walker.distance_since_step / FOOTSTEP_DISTANCE) * std::f32::consts::PI;
    transform.translation.y = WALKER_HEIGHT + step_phase.sin().abs() * 0.08;
}

fn tile_center_x(index: usize) -> f32 {
    let pitch = TILE_SIZE + TILE_GAP;
    index as f32 * pitch - (GRID_WIDTH as f32 - 1.0) * pitch * 0.5
}

fn tile_center_z(index: usize) -> f32 {
    let pitch = TILE_SIZE + TILE_GAP;
    index as f32 * pitch - (GRID_DEPTH as f32 - 1.0) * pitch * 0.5
}
