// SPDX-License-Identifier: MPL-2.0

use bevy::prelude::*;
use sonara_bevy::{
    AudioRequestResult,
    prelude::{AudioEmitter, SonaraAudio, SonaraFirewheelPlugin},
};
use sonara_build::CompiledBankPackage;
use sonara_model::{EventId, ParameterId, ParameterValue};

const TILE_SIZE: f32 = 2.2;
const TILE_GAP: f32 = 0.12;
const GRID_WIDTH: usize = 6;
const GRID_DEPTH: usize = 4;
const WALK_SPEED: f32 = 4.5;
const FOOTSTEP_DISTANCE: f32 = 1.35;
const FLOOR_Y: f32 = 0.0;
const WALKER_HEIGHT: f32 = 0.55;

fn main() {
    let demo_ids = read_demo_ids();

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Sonara Surface Walk".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(SonaraFirewheelPlugin)
        .insert_non_send_resource(demo_ids)
        .insert_resource(SurfaceGrid::default())
        .insert_resource(HudState::default())
        .add_systems(Startup, setup_scene)
        .add_systems(Update, control_walker)
        .add_systems(Update, sync_hud_text)
        .run();
}

struct DemoIds {
    event_id: EventId,
    surface_id: ParameterId,
}

fn read_demo_ids() -> DemoIds {
    let package = CompiledBankPackage::read_json_file("sonara-app/assets/demo/core.bank.json")
        .expect("compiled demo bank should load from JSON");
    let event = package
        .events()
        .iter()
        .find(|event| event.name == "player.footstep")
        .expect("compiled bank should contain player.footstep");

    DemoIds {
        event_id: event.id,
        surface_id: *event
            .default_parameters
            .first()
            .expect("compiled footstep event should reference a surface parameter"),
    }
}

#[derive(Resource)]
struct HudState {
    lines: Vec<String>,
    latest_step: String,
}

impl Default for HudState {
    fn default() -> Self {
        Self {
            lines: vec![
                "Sonara surface_walk".into(),
                "compiled bank file: sonara-app/assets/demo/core.bank.json".into(),
                "WASD or arrow keys move the blue sphere".into(),
                "brown tiles = wood, gray tiles = stone".into(),
                "footsteps follow the surface under the sphere".into(),
            ],
            latest_step: "latest step: none".into(),
        }
    }
}

#[derive(Component)]
struct HudText;

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
    surface_grid: Res<SurfaceGrid>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let package = CompiledBankPackage::read_json_file("sonara-app/assets/demo/core.bank.json")
        .expect("compiled demo bank should load from JSON");
    audio
        .load_compiled_bank(package)
        .expect("compiled bank should load in startup system");

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

fn control_walker(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut audio: NonSendMut<SonaraAudio>,
    demo_ids: NonSend<DemoIds>,
    mut hud: ResMut<HudState>,
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
        hud.latest_step = format!(
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

fn sync_hud_text(hud: Res<HudState>, mut hud_query: Query<&mut Text, With<HudText>>) {
    if !hud.is_changed() {
        return;
    }

    let Ok(mut text) = hud_query.single_mut() else {
        return;
    };

    let mut content = hud.lines.join("\n");
    content.push('\n');
    content.push_str(&hud.latest_step);
    *text = Text::new(content);
}

fn tile_center_x(index: usize) -> f32 {
    let pitch = TILE_SIZE + TILE_GAP;
    index as f32 * pitch - (GRID_WIDTH as f32 - 1.0) * pitch * 0.5
}

fn tile_center_z(index: usize) -> f32 {
    let pitch = TILE_SIZE + TILE_GAP;
    index as f32 * pitch - (GRID_DEPTH as f32 - 1.0) * pitch * 0.5
}
