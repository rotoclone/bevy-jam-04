use std::{collections::HashMap, f32::consts::PI, ops::RangeInclusive, time::Duration};

use bevy::{
    audio::{Volume, VolumeLevel},
    ecs::query::WorldQuery,
    input::common_conditions::input_just_pressed,
    sprite::MaterialMesh2dBundle,
};
use bevy_asset_loader::{
    asset_collection::AssetCollection,
    loading_state::{LoadingState, LoadingStateAppExt},
};
use bevy_rapier2d::{
    dynamics::{
        AdditionalMassProperties, Damping, ExternalForce, ExternalImpulse, GravityScale,
        MassProperties, RigidBody, Velocity,
    },
    geometry::{ActiveEvents, Collider, Sensor},
    pipeline::CollisionEvent,
};
use bevy_tweening::{
    lens::{TransformPositionLens, TransformRotateZLens, TransformScaleLens},
    Animator, AnimatorState, Delay, EaseFunction, Tracks, Tween, TweenCompleted,
};
use iyes_progress::{ProgressCounter, ProgressPlugin};
use rand::{
    distributions::{Distribution, WeightedIndex},
    seq::SliceRandom,
    Rng,
};
use strum::{EnumIter, IntoEnumIterator};

use crate::*;

const LOADING_FONT: &str = "fonts/MajorMonoDisplay-Regular.ttf";

const PLAYER_SIZE: f32 = 5.0;
const PLAYER_MAX_SPEED: f32 = 70.0;
const PLAYER_MOVE_FORCE: f32 = 100000.0;
const PLAYER_DAMPING: f32 = 10.0;
const PLAYER_MASS: f32 = 100.0;
const PLAYER_INERTIA: f32 = 16000.0;

const ENEMY_SIZE: f32 = 4.0;
const ENEMY_MAX_SPEED: f32 = 40.0;
const ENEMY_MOVE_FORCE: f32 = 75000.0;
const ENEMY_DAMPING: f32 = 10.0;
const ENEMY_MASS: f32 = 100.0;
const ENEMY_INERTIA: f32 = 16000.0;

const HIT_IMPULSE: f32 = 50000.0;

const SWORD_WIDTH: f32 = 1.0;
const SWORD_LENGTH: f32 = 14.0;

const PLAYER_ATTACK_COOLDOWN: Duration = Duration::from_millis(1000);
const SWORD_SWING_ROTATION_DEGREES: f32 = 60.0;
const SWORD_SWING_TRANSLATION: f32 = 2.0;

const SWORD_ANIMATION_TIME: Duration = Duration::from_millis(75);
const SWORD_ANIMATION_END_DELAY: Duration = Duration::from_millis(100);
const SWORD_TAKE_OUT_TIME: Duration = Duration::from_millis(1);
const SWORD_PUT_AWAY_TIME: Duration = Duration::from_millis(150);

const SWORD_START_SCALE: Vec3 = Vec3::new(1.0, 0.0, 1.0);
const SWORD_END_SCALE: Vec3 = Vec3::ONE;
// manually converting degrees to radians because `f32::to_radians` isn't `const` for some reason
const SWORD_START_ROTATION: f32 = (SWORD_SWING_ROTATION_DEGREES / 2.0) * (PI / 180.0f32);
const SWORD_END_ROTATION: f32 = (-SWORD_SWING_ROTATION_DEGREES / 2.0) * (PI / 180.0f32);
const SWORD_START_TRANSLATION: Vec3 =
    Vec3::new(-SWORD_SWING_TRANSLATION / 2.0, PLAYER_SIZE * 0.5, SWORD_Z);
const SWORD_END_TRANSLATION: Vec3 =
    Vec3::new(SWORD_SWING_TRANSLATION / 2.0, PLAYER_SIZE * 0.5, SWORD_Z);

const SWORD_SWING_COMPLETE_EVENT_ID: u64 = 1;
const ATTACK_DONE_EVENT_ID: u64 = 2;

const SWORD_Z: f32 = -1.0;
const BACKGROUND_Z: f32 = -100.0;

const PLAY_AREA_SIZE: Vec2 = Vec2::new(1000.0, 1000.0);

const SPAWN_AREA_DEPTH: f32 = 25.0;
const SPAWN_AREA_BUFFER: f32 = ENEMY_SIZE;

const START_SPAWN_INTERVAL: Duration = Duration::from_millis(2000);
const NEXT_LEVEL_XP_MULTIPLIER: f64 = 2.0;
const STARTING_XP_THRESHOLD: u64 = 5;
const STARTING_HEALTH: u64 = 100;

const MOVE_LEFT_KEY: KeyCode = KeyCode::A;
const MOVE_RIGHT_KEY: KeyCode = KeyCode::D;
const MOVE_UP_KEY: KeyCode = KeyCode::W;
const MOVE_DOWN_KEY: KeyCode = KeyCode::S;
const ATTACK_INPUT: MouseButton = MouseButton::Left;

const BG_MUSIC_VOLUME: f32 = 0.5;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_loading_state(LoadingState::new(GameState::GameLoading))
            .add_collection_to_loading_state::<_, ImageAssets>(GameState::GameLoading)
            .add_collection_to_loading_state::<_, AudioAssets>(GameState::GameLoading)
            .add_plugins(ProgressPlugin::new(GameState::GameLoading).continue_to(GameState::Game))
            .add_systems(
                Update,
                display_loading_progress.run_if(in_state(GameState::GameLoading)),
            );

        app.add_systems(OnEnter(GameState::GameLoading), loading_setup)
            .add_systems(
                OnExit(GameState::GameLoading),
                despawn_components_system::<LoadingComponent>,
            );

        app.add_systems(
            OnEnter(GameState::Game),
            (game_setup, start_background_music),
        )
        .add_systems(
            OnExit(GameState::Game),
            (
                despawn_components_system::<GameComponent>,
                stop_background_music,
            ),
        );

        app.insert_resource(SpawnTimer(Timer::new(
            START_SPAWN_INTERVAL,
            TimerMode::Repeating,
        )))
        .insert_resource(SpawnAreas(Vec::new()))
        .insert_resource(generate_starting_spawn_weights())
        .insert_resource(EntitiesToDespawn(Vec::new()))
        .insert_resource(Level {
            current_level: 1,
            current_xp: 0,
            xp_needed: STARTING_XP_THRESHOLD,
        })
        .insert_resource(Health {
            current_health: STARTING_HEALTH,
            max_health: STARTING_HEALTH,
        })
        .add_systems(
            Update,
            (
                update_attack_cooldown.before(player_attack),
                player_movement,
                player_attack.run_if(input_just_pressed(ATTACK_INPUT)),
                tween_completed,
                move_camera.after(player_movement),
                keep_player_in_bounds.after(player_movement),
                spawn_enemies.run_if(in_state(GameState::Game)),
                move_enemies,
                collisions,
                update_level_display
                    .after(collisions)
                    .run_if(resource_changed::<Level>()),
                update_health_display
                    .after(collisions)
                    .run_if(resource_changed::<Health>()),
            ),
        )
        .add_systems(PostUpdate, despawn_entities);
    }
}

/// Generates the spawn weights that the game starts with
fn generate_starting_spawn_weights() -> SpawnWeights {
    let mut types = Vec::new();
    let mut weights = Vec::new();
    for enemy_type in EnemyType::iter() {
        let weight = match enemy_type {
            EnemyType::Regular => 0.55,
            EnemyType::SmallAndFast => 0.2,
            EnemyType::BigAndSlow => 0.2,
            EnemyType::Assassin => 0.05,
        };
        types.push(enemy_type);
        weights.push(weight);
    }

    SpawnWeights {
        types,
        weights: weights.clone(),
        dist: WeightedIndex::new(weights).expect("weights should be valid"),
    }
}

#[derive(AssetCollection, Resource)]
pub struct ImageAssets {
    #[asset(path = "images/bg.png")]
    background: Handle<Image>,
}

#[derive(AssetCollection, Resource)]
pub struct AudioAssets {
    /* TODO
    #[asset(path = "sounds/background_music.ogg")]
    background_music: Handle<AudioSource>,
    */
}

#[derive(Resource)]
struct SpawnTimer(Timer);

#[derive(Resource)]
struct SpawnAreas(Vec<Rect>);

struct EnemyParams {
    color: Color,
    size: RangeInclusive<f32>,
    max_speed: RangeInclusive<f32>,
    damage: u64,
    xp_reward: u64,
}

#[derive(Hash, PartialEq, Eq, Clone, Copy, EnumIter)]
enum EnemyType {
    Regular,
    SmallAndFast,
    BigAndSlow,
    Assassin,
}

impl EnemyType {
    /// Gets the parameters describing the provided enemy type
    fn get_params(&self) -> EnemyParams {
        match self {
            EnemyType::Regular => EnemyParams {
                color: Color::RED,
                size: 4.0..=4.0,
                max_speed: 30.0..=50.0,
                damage: 5,
                xp_reward: 1,
            },
            EnemyType::SmallAndFast => EnemyParams {
                color: Color::SEA_GREEN,
                size: 2.0..=2.0,
                max_speed: 50.0..=70.0,
                damage: 3,
                xp_reward: 1,
            },
            EnemyType::BigAndSlow => EnemyParams {
                color: Color::ORANGE_RED,
                size: 7.0..=7.0,
                max_speed: 10.0..=30.0,
                damage: 10,
                xp_reward: 1,
            },
            EnemyType::Assassin => EnemyParams {
                color: Color::ANTIQUE_WHITE,
                size: 3.0..=3.0,
                max_speed: 70.0..=90.0,
                damage: 15,
                xp_reward: 2,
            },
        }
    }
}

#[derive(Resource)]
struct SpawnWeights {
    types: Vec<EnemyType>,
    weights: Vec<f32>,
    dist: WeightedIndex<f32>,
}

impl SpawnWeights {
    /// Picks a random enemy type based on the weights
    fn choose_random_enemy_type(&self) -> EnemyType {
        self.types[self.dist.sample(&mut rand::thread_rng())]
    }
}

#[derive(Resource)]
struct EntitiesToDespawn(Vec<Entity>);

#[derive(Resource)]
struct Level {
    current_level: u64,
    current_xp: u64,
    xp_needed: u64,
}

impl Level {
    /// Advances to the next level
    fn advance(&mut self) {
        self.current_level += 1;

        let new_xp_needed = self.xp_needed as f64 * NEXT_LEVEL_XP_MULTIPLIER;
        self.xp_needed = new_xp_needed.round() as u64;
    }
}

#[derive(Resource)]
struct Health {
    current_health: u64,
    max_health: u64,
}

#[derive(Component)]
struct LoadingComponent;

#[derive(Component)]
struct LoadingText;

#[derive(Component)]
struct GameComponent;

#[derive(Component)]
struct BackgroundMusic;

#[derive(Component)]
struct Player;

#[derive(Component)]
struct AttackCooldown(Timer);

#[derive(Component)]
struct SwordPivot;

#[derive(Component)]
struct Sword {
    active: bool,
}

#[derive(Component)]
struct Attacking(bool);

#[derive(Component)]
struct Enemy {
    damage: u64,
    xp_reward: u64,
    max_speed: f32,
}

#[derive(Component)]
struct LevelText;

#[derive(Component)]
struct XpText;

#[derive(Component)]
struct HealthText;

/// Sets up the loading screen.
fn loading_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .spawn(
            TextBundle::from_section(
                "loading...\n0%",
                TextStyle {
                    font: asset_server.load(LOADING_FONT),
                    font_size: 50.0,
                    color: Color::WHITE,
                },
            )
            .with_text_alignment(TextAlignment::Center)
            .with_style(Style {
                margin: UiRect::all(Val::Auto),
                ..default()
            }),
        )
        .insert(LoadingComponent)
        .insert(LoadingText);
}

fn display_loading_progress(
    progress: Option<Res<ProgressCounter>>,
    mut loading_text_query: Query<&mut Text, With<LoadingText>>,
    mut last_done: Local<u32>,
) {
    if let Some(progress) = progress.map(|counter| counter.progress()) {
        if progress.done > *last_done {
            *last_done = progress.done;
            let percent_done = (progress.done as f32 / progress.total as f32) * 100.0;
            for mut loading_text in loading_text_query.iter_mut() {
                loading_text.sections[0].value = format!("loading...\n{percent_done:.0}%");
            }
        }
    }
}

/// Sets up the game
fn game_setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    image_assets: Res<ImageAssets>,
    mut spawn_areas: ResMut<SpawnAreas>,
    asset_server: Res<AssetServer>,
) {
    // background
    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                custom_size: Some(PLAY_AREA_SIZE),
                color: Color::WHITE.with_a(0.05),
                ..default()
            },
            texture: image_assets.background.clone(),
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, BACKGROUND_Z)),
            ..default()
        })
        .insert(GameComponent);

    // spawn areas
    spawn_areas.0 = vec![
        // left
        Rect::new(
            (-PLAY_AREA_SIZE.x / 2.0) - (SPAWN_AREA_DEPTH + SPAWN_AREA_BUFFER),
            -PLAY_AREA_SIZE.y / 2.0,
            (-PLAY_AREA_SIZE.x / 2.0) - SPAWN_AREA_BUFFER,
            PLAY_AREA_SIZE.y / 2.0,
        ),
        // right
        Rect::new(
            (PLAY_AREA_SIZE.x / 2.0) + (SPAWN_AREA_DEPTH + SPAWN_AREA_BUFFER),
            -PLAY_AREA_SIZE.y / 2.0,
            (PLAY_AREA_SIZE.x / 2.0) + SPAWN_AREA_BUFFER,
            PLAY_AREA_SIZE.y / 2.0,
        ),
        // top
        Rect::new(
            -PLAY_AREA_SIZE.x / 2.0,
            (PLAY_AREA_SIZE.y / 2.0) + (SPAWN_AREA_DEPTH + SPAWN_AREA_BUFFER),
            PLAY_AREA_SIZE.x / 2.0,
            (PLAY_AREA_SIZE.y / 2.0) + SPAWN_AREA_BUFFER,
        ),
        // bottom
        Rect::new(
            -PLAY_AREA_SIZE.x / 2.0,
            (-PLAY_AREA_SIZE.y / 2.0) - (SPAWN_AREA_DEPTH + SPAWN_AREA_BUFFER),
            PLAY_AREA_SIZE.x / 2.0,
            (-PLAY_AREA_SIZE.y / 2.0) - SPAWN_AREA_BUFFER,
        ),
    ];

    // player
    let mut attack_cooldown = AttackCooldown(Timer::new(PLAYER_ATTACK_COOLDOWN, TimerMode::Once));
    attack_cooldown.0.set_elapsed(PLAYER_ATTACK_COOLDOWN);

    let sword_swing_tween = Tween::new(
        EaseFunction::QuadraticOut,
        SWORD_TAKE_OUT_TIME,
        TransformScaleLens {
            start: SWORD_START_SCALE,
            end: SWORD_END_SCALE,
        },
    )
    .then(Tracks::new(vec![
        Tween::new(
            EaseFunction::QuadraticOut,
            SWORD_ANIMATION_TIME,
            TransformRotateZLens {
                start: SWORD_START_ROTATION,
                end: SWORD_END_ROTATION,
            },
        )
        .with_completed_event(SWORD_SWING_COMPLETE_EVENT_ID),
        Tween::new(
            EaseFunction::QuadraticOut,
            SWORD_ANIMATION_TIME,
            TransformPositionLens {
                start: SWORD_START_TRANSLATION,
                end: SWORD_END_TRANSLATION,
            },
        ),
    ]))
    .then(Delay::new(SWORD_ANIMATION_END_DELAY))
    .then(
        Tween::new(
            EaseFunction::QuadraticIn,
            SWORD_PUT_AWAY_TIME,
            TransformScaleLens {
                start: SWORD_END_SCALE,
                end: SWORD_START_SCALE,
            },
        )
        .with_completed_event(ATTACK_DONE_EVENT_ID),
    );

    commands
        .spawn(MaterialMesh2dBundle {
            mesh: meshes.add(shape::Circle::new(PLAYER_SIZE).into()).into(),
            material: materials.add(ColorMaterial::from(Color::PURPLE)),
            transform: Transform::from_translation(Vec3::new(0., 0., 0.)),
            ..default()
        })
        .insert(GameComponent)
        .insert(Collider::ball(PLAYER_SIZE))
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(RigidBody::Dynamic)
        .insert(AdditionalMassProperties::MassProperties(MassProperties {
            mass: PLAYER_MASS,
            principal_inertia: PLAYER_INERTIA,
            ..default()
        }))
        .insert(ExternalForce::default())
        .insert(ExternalImpulse::default())
        .insert(Velocity::default())
        .insert(Damping {
            linear_damping: PLAYER_DAMPING,
            ..default()
        })
        .insert(GravityScale(0.0))
        .insert(Player)
        .insert(Attacking(false))
        .insert(attack_cooldown)
        .with_children(|parent| {
            // sword pivot
            parent
                .spawn(SpatialBundle::from_transform(
                    Transform::from_translation(SWORD_START_TRANSLATION)
                        .with_scale(SWORD_START_SCALE)
                        .with_rotation(Quat::from_rotation_z(SWORD_START_ROTATION)),
                ))
                .insert(SwordPivot)
                .insert(Animator::new(sword_swing_tween).with_state(AnimatorState::Paused))
                .with_children(|pivot| {
                    // sword
                    pivot
                        .spawn(MaterialMesh2dBundle {
                            mesh: meshes
                                .add(shape::Quad::new(Vec2::new(SWORD_WIDTH, SWORD_LENGTH)).into())
                                .into(),
                            material: materials.add(ColorMaterial::from(Color::GRAY)),
                            transform: Transform::from_translation(Vec3::new(
                                0.,
                                SWORD_LENGTH / 2.0,
                                0.,
                            )),
                            ..default()
                        })
                        .insert(Collider::cuboid(SWORD_WIDTH / 2.0, SWORD_LENGTH / 2.0))
                        .insert(Sensor)
                        .insert(Sword { active: false });
                });
        });

    // health display
    commands
        .spawn(
            TextBundle::from_section(
                format!("Health: {STARTING_HEALTH}/{STARTING_HEALTH}"),
                TextStyle {
                    font: asset_server.load(MONO_FONT),
                    font_size: 35.0,
                    color: Color::WHITE,
                },
            )
            .with_text_alignment(TextAlignment::Center)
            .with_style(Style {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                margin: UiRect {
                    left: Val::Auto,
                    right: Val::Auto,
                    ..default()
                },
                ..default()
            }),
        )
        .insert(GameComponent)
        .insert(HealthText);

    // right sidebar
    commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(33.3),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                margin: UiRect {
                    left: Val::Px(5.0),
                    top: Val::Px(5.0),
                    ..default()
                },
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Start,
                align_items: AlignItems::FlexEnd,
                ..default()
            },
            ..default()
        })
        .insert(GameComponent)
        .with_children(|parent| {
            // level display
            parent
                .spawn(
                    TextBundle::from_section(
                        "Level 1",
                        TextStyle {
                            font: asset_server.load(MONO_FONT),
                            font_size: 28.0,
                            color: Color::Rgba {
                                red: 0.75,
                                green: 0.75,
                                blue: 0.75,
                                alpha: 1.0,
                            },
                        },
                    )
                    .with_text_alignment(TextAlignment::Center)
                    .with_style(Style {
                        margin: UiRect {
                            bottom: Val::Px(5.0),
                            ..default()
                        },
                        ..default()
                    }),
                )
                .insert(LevelText);

            // xp display
            parent
                .spawn(
                    TextBundle::from_section(
                        format!("XP: 0/{STARTING_XP_THRESHOLD}"),
                        TextStyle {
                            font: asset_server.load(MONO_FONT),
                            font_size: 33.0,
                            color: Color::WHITE,
                        },
                    )
                    .with_text_alignment(TextAlignment::Center)
                    .with_style(Style {
                        margin: UiRect {
                            bottom: Val::Px(5.0),
                            ..default()
                        },
                        ..default()
                    }),
                )
                .insert(XpText);
        });
}

/// Handles events for completed tweens
fn tween_completed(
    mut reader: EventReader<TweenCompleted>,
    mut sword_query: Query<&mut Sword>,
    mut player_attacking_query: Query<&mut Attacking, With<Player>>,
) {
    for ev in reader.read() {
        if ev.user_data == SWORD_SWING_COMPLETE_EVENT_ID {
            for mut sword in sword_query.iter_mut() {
                sword.active = false;
            }
        }

        if ev.user_data == ATTACK_DONE_EVENT_ID {
            for mut attacking in player_attacking_query.iter_mut() {
                attacking.0 = false;
            }
        }
    }
}

/// Updates remaining attack cooldowns
fn update_attack_cooldown(mut query: Query<&mut AttackCooldown>, time: Res<Time>) {
    for mut cooldown in query.iter_mut() {
        cooldown.0.tick(time.delta());
    }
}

/// Applies impulses to the player based on pressed keys
fn player_movement(
    mut player_query: Query<
        (
            &mut ExternalForce,
            &mut Velocity,
            &mut Transform,
            &Attacking,
        ),
        With<Player>,
    >,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    window_query: Query<&Window>,
    keycode: Res<Input<KeyCode>>,
) {
    let (camera, camera_transform) = camera_query.single();
    let Some(cursor_position) = window_query.single().cursor_position() else {
        return;
    };
    // Calculate a world position based on the cursor's position.
    let Some(cursor_world_position) =
        camera.viewport_to_world_2d(camera_transform, cursor_position)
    else {
        return;
    };

    for (mut force, mut velocity, mut transform, attacking) in &mut player_query {
        // translation
        if keycode.pressed(MOVE_LEFT_KEY) {
            force.force.x = -PLAYER_MOVE_FORCE;
        } else if keycode.pressed(MOVE_RIGHT_KEY) {
            force.force.x = PLAYER_MOVE_FORCE;
        } else {
            force.force.x = 0.0;
        }

        if keycode.pressed(MOVE_UP_KEY) {
            force.force.y = PLAYER_MOVE_FORCE;
        } else if keycode.pressed(MOVE_DOWN_KEY) {
            force.force.y = -PLAYER_MOVE_FORCE;
        } else {
            force.force.y = 0.0;
        }

        // don't allow rotation while attacking because rapid spinning can increase the effective size of the sword swing
        if !attacking.0 {
            // rotation
            if let Some(to_cursor) =
                (cursor_world_position - transform.translation.xy()).try_normalize()
            {
                let rotate_to_cursor = Quat::from_rotation_arc(Vec3::Y, to_cursor.extend(0.));
                transform.rotation = rotate_to_cursor;
            }
        }

        // prevent player from spinning around by itself
        velocity.angvel = 0.0;

        // clamp speed
        velocity.linvel = velocity.linvel.clamp_length_max(PLAYER_MAX_SPEED);
    }
}

/// Prevents the player from leaving the play area by clamping its transform
fn keep_player_in_bounds(mut player_query: Query<&mut Transform, With<Player>>) {
    let max_x = PLAY_AREA_SIZE.x / 2.0 - PLAYER_SIZE / 2.0;
    let min_x = -PLAY_AREA_SIZE.x / 2.0 + PLAYER_SIZE / 2.0;
    let max_y = PLAY_AREA_SIZE.y / 2.0 - PLAYER_SIZE / 2.0;
    let min_y = -PLAY_AREA_SIZE.y / 2.0 + PLAYER_SIZE / 2.0;
    for mut transform in player_query.iter_mut() {
        transform.translation = transform.translation.clamp(
            Vec3::new(min_x, min_y, transform.translation.z),
            Vec3::new(max_x, max_y, transform.translation.z),
        );
    }
}

/// Makes the player attack
fn player_attack(
    mut player_query: Query<(&mut AttackCooldown, &mut Attacking), With<Player>>,
    mut sword_pivot_query: Query<(&mut Animator<Transform>, &mut Transform), With<SwordPivot>>,
    mut sword_query: Query<&mut Sword>,
) {
    for (mut cooldown, mut attacking) in player_query.iter_mut() {
        if !cooldown.0.finished() {
            continue;
        }

        //TODO only animate the sword for this particular player somehow
        for (mut animator, mut transform) in sword_pivot_query.iter_mut() {
            animator.stop();

            transform.scale = SWORD_START_SCALE;
            transform.rotation.z = SWORD_START_ROTATION;
            transform.translation = SWORD_START_TRANSLATION;

            attacking.0 = true;

            animator.state = AnimatorState::Playing;
        }

        for mut sword in sword_query.iter_mut() {
            sword.active = true;
        }

        cooldown.0.reset();
    }
}

/// Moves the camera to follow the player
fn move_camera(
    mut camera_query: Query<(&mut LookTransform, &OrthographicProjection), With<MainCamera>>,
    player_query: Query<&Transform, With<Player>>,
) {
    if let Ok(player_transform) = player_query.get_single() {
        for (mut look_transform, projection) in camera_query.iter_mut() {
            let max_x = (PLAY_AREA_SIZE.x / 2.0) - (projection.area.width() / 2.0);
            let min_x = (-PLAY_AREA_SIZE.x / 2.0) + (projection.area.width() / 2.0);
            let max_y = (PLAY_AREA_SIZE.y / 2.0) - (projection.area.height() / 2.0);
            let min_y = (-PLAY_AREA_SIZE.y / 2.0) + (projection.area.height() / 2.0);

            let target_min_x;
            let target_max_x;
            if min_x <= max_x {
                look_transform.eye.x = player_transform.translation.x.clamp(min_x, max_x);
                target_min_x = min_x;
                target_max_x = max_x;
            } else {
                look_transform.eye.x = 0.0;
                target_min_x = 0.0;
                target_max_x = 0.0;
            }

            let target_min_y;
            let target_max_y;
            if min_y <= max_y {
                look_transform.eye.y = player_transform.translation.y.clamp(min_y, max_y);
                target_min_y = min_y;
                target_max_y = max_y;
            } else {
                look_transform.eye.y = 0.0;
                target_min_y = 0.0;
                target_max_y = 0.0;
            }

            look_transform.target = player_transform.translation.clamp(
                Vec3::new(target_min_x, target_min_y, look_transform.target.z),
                Vec3::new(target_max_x, target_max_y, look_transform.target.z),
            );
        }
    }
}

/// Handles spawning enemies
fn spawn_enemies(
    commands: Commands,
    mut spawn_timer: ResMut<SpawnTimer>,
    spawn_areas: Res<SpawnAreas>,
    spawn_weights: Res<SpawnWeights>,
    time: Res<Time>,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
) {
    spawn_timer.0.tick(time.delta());

    if spawn_timer.0.just_finished() {
        spawn_random_enemy(commands, spawn_areas, spawn_weights, meshes, materials);
    }
}

/// Spawns a random enemy at a random location
fn spawn_random_enemy(
    commands: Commands,
    spawn_areas: Res<SpawnAreas>,
    spawn_weights: Res<SpawnWeights>,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
) {
    let mut rng = rand::thread_rng();
    if let Some(spawn_area) = spawn_areas.0.choose(&mut rng) {
        let x_coord = rng.gen_range(spawn_area.min.x..=spawn_area.max.x);
        let y_coord = rng.gen_range(spawn_area.min.y..=spawn_area.max.y);

        spawn_enemy(
            commands,
            Vec3::new(x_coord, y_coord, 0.0),
            spawn_weights.choose_random_enemy_type().get_params(),
            meshes,
            materials,
        );
    }
}

/// Spawns an enemy at the provided location
fn spawn_enemy(
    mut commands: Commands,
    location: Vec3,
    params: EnemyParams,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let mut rng = rand::thread_rng();
    let size = rng.gen_range(params.size);

    commands
        .spawn(MaterialMesh2dBundle {
            mesh: meshes.add(shape::Circle::new(size).into()).into(),
            material: materials.add(ColorMaterial::from(params.color)),
            transform: Transform::from_translation(location),
            ..default()
        })
        .insert(GameComponent)
        .insert(Collider::ball(size))
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(RigidBody::Dynamic)
        .insert(AdditionalMassProperties::MassProperties(MassProperties {
            mass: ENEMY_MASS,
            principal_inertia: ENEMY_INERTIA,
            ..default()
        }))
        .insert(ExternalForce::default())
        .insert(ExternalImpulse::default())
        .insert(Velocity::default())
        .insert(Damping {
            linear_damping: ENEMY_DAMPING,
            ..default()
        })
        .insert(GravityScale(0.0))
        .insert(Enemy {
            damage: params.damage,
            xp_reward: params.xp_reward,
            max_speed: rng.gen_range(params.max_speed),
        });
}

/// Handles moving enemies
fn move_enemies(
    mut enemy_query: Query<
        (&mut ExternalForce, &mut Velocity, &mut Transform, &Enemy),
        Without<Player>,
    >,
    player_query: Query<&Transform, With<Player>>,
) {
    if let Ok(player_transform) = player_query.get_single() {
        for (mut force, mut velocity, mut transform, enemy) in &mut enemy_query {
            // push enemy in direction of player
            let player_direction = player_transform.translation - transform.translation;
            let movement_force = player_direction.clamp_length(ENEMY_MOVE_FORCE, ENEMY_MOVE_FORCE);
            force.force = Vec2::new(movement_force.x, movement_force.y);

            // rotate to face player
            if let Some(to_player) = player_direction.try_normalize() {
                let rotate_to_player = Quat::from_rotation_arc(Vec3::Y, to_player);
                transform.rotation = rotate_to_player;
            }

            // prevent enemies from spinning around on their own
            velocity.angvel = 0.0;

            // clamp speed
            velocity.linvel = velocity.linvel.clamp_length_max(enemy.max_speed);
        }
    }
}

/// Handles collisions between objects
fn collisions(
    mut collision_events: EventReader<CollisionEvent>,
    mut entities_to_despawn: ResMut<EntitiesToDespawn>,
    mut level: ResMut<Level>,
    mut health: ResMut<Health>,
    enemies_query: Query<(&Enemy, &Transform)>,
    sword_query: Query<&Sword>,
    mut player_query: Query<(&Player, &Transform, &mut ExternalImpulse)>,
) {
    for event in collision_events.read() {
        if let CollisionEvent::Started(a, b, _) = event {
            if let Some((enemy, enemy_entity)) =
                get_from_either::<Enemy, (&Enemy, &Transform)>(*a, *b, &enemies_query)
            {
                // an enemy has hit something
                if entities_to_despawn.0.contains(&enemy_entity) {
                    // this enemy is going to be despawned, so don't mess with it any more
                    continue;
                }

                if let Some((player, player_entity)) = get_from_either::<
                    Player,
                    (&Player, &Transform, &mut ExternalImpulse),
                >(*a, *b, &player_query)
                {
                    // an enemy has hit the player
                    health.current_health = health.current_health.saturating_sub(enemy.damage);

                    if let Ok(player_transform) =
                        player_query.get_component::<Transform>(player_entity)
                    {
                        if let Ok(enemy_transform) =
                            enemies_query.get_component::<Transform>(enemy_entity)
                        {
                            // push the player back
                            let enemy_to_player =
                                player_transform.translation - enemy_transform.translation;
                            let hit_force = enemy_to_player.clamp_length(HIT_IMPULSE, HIT_IMPULSE);
                            if let Ok(mut impulse) =
                                player_query.get_component_mut::<ExternalImpulse>(player_entity)
                            {
                                impulse.impulse = Vec2::new(hit_force.x, hit_force.y);
                            }
                        }
                    }
                } else if let Some((sword, sword_entity)) =
                    get_from_either::<Sword, &Sword>(*a, *b, &sword_query)
                {
                    // an enemy has hit the sword
                    if sword.active {
                        entities_to_despawn.0.push(enemy_entity);
                        level.current_xp += enemy.xp_reward;
                    }
                }
            }
        }
    }
}

fn get_from_either<'a, T: Component, Q: WorldQuery>(
    a: Entity,
    b: Entity,
    query: &'a Query<Q>,
) -> Option<(&'a T, Entity)> {
    if let Ok(component) = query.get_component::<T>(a) {
        return Some((component, a));
    }

    if let Ok(component) = query.get_component::<T>(b) {
        return Some((component, b));
    }

    None
}

/// Despawns entities that need to be despawned
fn despawn_entities(mut commands: Commands, mut entities_to_despawn: ResMut<EntitiesToDespawn>) {
    for entity in entities_to_despawn.0.drain(0..) {
        commands.entity(entity).despawn_recursive();
    }
}

/// Keeps the level display up to date
fn update_level_display(
    mut level: ResMut<Level>,
    mut level_text_query: Query<&mut Text, (With<LevelText>, Without<XpText>)>,
    mut xp_text_query: Query<&mut Text, (With<XpText>, Without<LevelText>)>,
) {
    while level.current_xp >= level.xp_needed {
        level.advance();
    }

    for mut text in level_text_query.iter_mut() {
        text.sections[0].value = format!("Level {}", level.current_level);
    }

    for mut text in xp_text_query.iter_mut() {
        text.sections[0].value = format!("XP: {}/{}", level.current_xp, level.xp_needed);
    }
}

/// Keeps the health display up to date
fn update_health_display(
    health: Res<Health>,
    mut health_text_query: Query<&mut Text, With<HealthText>>,
) {
    for mut text in health_text_query.iter_mut() {
        text.sections[0].value = format!("Health: {}/{}", health.current_health, health.max_health);
    }
}

/// Starts playing the background music
fn start_background_music(mut commands: Commands /* TODO audio_assets: Res<AudioAssets> */) {
    /* TODO
    commands.spawn((
        AudioBundle {
            source: audio_assets.background_music.clone(),
            settings: PlaybackSettings::LOOP
                .with_volume(Volume::Relative(VolumeLevel::new(BG_MUSIC_VOLUME))),
        },
        BackgroundMusic,
    ));
    */
}

/// Stops playing the background music
fn stop_background_music(music_controller: Query<&AudioSink, With<BackgroundMusic>>) {
    if let Ok(sink) = music_controller.get_single() {
        sink.stop();
    }
}
