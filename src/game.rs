use std::{f32::consts::PI, ops::RangeInclusive, time::Duration};

use bevy::{
    audio::{Volume, VolumeLevel},
    ecs::query::WorldQuery,
    input::common_conditions::input_pressed,
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
    Animator, AnimatorState, Delay, EaseFunction, EaseMethod, Sequence, Tracks, Tween,
    TweenCompleted,
};
use iyes_progress::{ProgressCounter, ProgressPlugin};
use rand::{
    distributions::{Distribution, WeightedIndex},
    seq::{IteratorRandom, SliceRandom},
    Rng,
};
use strum::{EnumIter, IntoEnumIterator};

use crate::*;

const LOADING_FONT: &str = "fonts/MajorMonoDisplay-Regular.ttf";

const PLAYER_SIZE: f32 = 5.0;
const PLAYER_MAX_SPEED: f32 = 70.0;
const PLAYER_MOVE_FORCE: f32 = 100000.0;
const PLAYER_DAMPING: f32 = 8.0;
const PLAYER_MASS: f32 = 100.0;
const PLAYER_INERTIA: f32 = 16000.0;

const ENEMY_MOVE_FORCE: f32 = 35000.0;
const ENEMY_DAMPING: f32 = 4.0;
const ENEMY_MASS: f32 = 50.0;
const ENEMY_INERTIA: f32 = 8000.0;

const HIT_IMPULSE: f32 = 50000.0;

const SWORD_WIDTH: f32 = 1.0;
const SWORD_LENGTH: f32 = 14.0;

const PLAYER_ATTACK_COOLDOWN: Duration = Duration::from_millis(1000);
const SWORD_SWING_ROTATION_DEGREES: f32 = 60.0;
const SWORD_SWING_TRANSLATION: f32 = 2.0;

const SWORD_ANIMATION_TIME: Duration = Duration::from_millis(60);
const SWORD_ANIMATION_END_DELAY: Duration = Duration::from_millis(100);
const SWORD_PUT_AWAY_TIME: Duration = Duration::from_millis(80);
const SWORD_SHADOW_DELAYS_AND_ALPHAS: [(Duration, f32); 10] = [
    (Duration::from_millis(5), 0.50),
    (Duration::from_millis(10), 0.45),
    (Duration::from_millis(15), 0.40),
    (Duration::from_millis(20), 0.35),
    (Duration::from_millis(25), 0.30),
    (Duration::from_millis(30), 0.25),
    (Duration::from_millis(35), 0.20),
    (Duration::from_millis(40), 0.15),
    (Duration::from_millis(45), 0.10),
    (Duration::from_millis(50), 0.05),
];

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

const HIT_SLOW_MO_TIME: Duration = Duration::from_millis(250);
const HIT_SLOW_MO_TIME_SCALE: f32 = 0.5;

const SWORD_Z: f32 = -1.0;
const BACKGROUND_Z: f32 = -100.0;

const PLAY_AREA_SIZE: Vec2 = Vec2::new(1000.0, 1000.0);

const SPAWN_AREA_DEPTH: f32 = 25.0;
const SPAWN_AREA_BUFFER: f32 = 10.0;

const START_SPAWN_INTERVAL: Duration = Duration::from_millis(1000);
const SPAWN_INTERVAL_CHANGE_INTERVAL: Duration = Duration::from_secs(5);
const SPAWN_INTERVAL_CHANGE_MULTIPLIER: f32 = 0.95;
const MIN_SPAWN_INTERVAL: Duration = Duration::from_millis(5);

const NEXT_LEVEL_ADDITIONAL_XP_MULTIPLIER: f64 = 1.5;
const STARTING_XP_THRESHOLD: u64 = 5;
const NUM_PERK_CHOICES: usize = 3;
const STARTING_HEALTH: u64 = 100;

const MAX_ZOOM_LEVEL: f32 = 1.0;
const ZOOM_LEVEL_MULTIPLIER: f32 = 1.05;

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

        // placeholder resources so things don't blow up before the game is set up
        app.insert_resource(Health {
            current_health: 1,
            max_health: 1,
        })
        .insert_resource(Level {
            current_level: 1,
            current_xp: 0,
            previous_xp_needed: 0,
            xp_needed: 1,
        })
        .insert_resource(EntitiesToDespawn(Vec::new()));

        app.add_event::<LevelUp>()
            .add_systems(
                Update,
                (
                    update_attack_cooldown.before(player_attack),
                    player_movement,
                    player_attack.run_if(input_pressed(ATTACK_INPUT)),
                    tween_completed,
                    move_camera.after(player_movement),
                    keep_player_in_bounds.after(player_movement),
                    spawn_enemies.run_if(in_state(GameState::Game)),
                    move_enemies,
                    collisions.run_if(in_state(GameState::Game)),
                    update_level_display
                        .after(collisions)
                        .run_if(resource_changed::<Level>()),
                    update_health_display
                        .after(collisions)
                        .run_if(resource_changed::<Health>()),
                    update_enemy_count_display,
                    slow_mo.run_if(in_state(GameState::Game)),
                    check_for_death.run_if(resource_changed::<Health>()),
                    level_up.after(update_level_display),
                ),
            )
            .add_systems(PostUpdate, despawn_entities);
    }
}

/// Sets up resources that the game starts with
fn insert_starting_resources(commands: &mut Commands) {
    commands.insert_resource(ZoomLevel(STARTING_ZOOM_LEVEL));
    commands.insert_resource(build_starting_spawn_timer());
    commands.insert_resource(build_starting_spawn_interval_change_timer());
    commands.insert_resource(build_spawn_areas());
    commands.insert_resource(build_starting_spawn_weights());
    commands.insert_resource(EntitiesToDespawn(Vec::new()));
    commands.insert_resource(Level {
        current_level: 1,
        current_xp: 0,
        previous_xp_needed: 0,
        xp_needed: STARTING_XP_THRESHOLD,
    });
    commands.insert_resource(Health {
        current_health: STARTING_HEALTH,
        max_health: STARTING_HEALTH,
    });

    let mut slow_mo_timer = Timer::new(HIT_SLOW_MO_TIME, TimerMode::Once);
    slow_mo_timer.pause();
    commands.insert_resource(SlowMoTimer {
        target_time_scale: 1.0,
        timer: slow_mo_timer,
    });
}

/// Builds the spawn areas
fn build_spawn_areas() -> SpawnAreas {
    SpawnAreas(vec![
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
    ])
}

/// Builds the spawn timer that the game starts with
fn build_starting_spawn_timer() -> SpawnTimer {
    SpawnTimer(Timer::new(START_SPAWN_INTERVAL, TimerMode::Repeating))
}

/// Builds the spawn interval change timer that the game starts with
fn build_starting_spawn_interval_change_timer() -> SpawnIntervalChangeTimer {
    SpawnIntervalChangeTimer(Timer::new(
        SPAWN_INTERVAL_CHANGE_INTERVAL,
        TimerMode::Repeating,
    ))
}

/// Builds the spawn weights that the game starts with
fn build_starting_spawn_weights() -> SpawnWeights {
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

#[derive(PartialEq, Eq, Clone, Copy, EnumIter)]
enum PerkType {
    LongerSword,
    WiderSwordSwing,
    ShorterAttackCooldown,
    HigherMaxSpeed,
    HigherMaxHealth,
    MorePerks,
    Heal,
    UnlockGrenade,
    LargerGrenadeExplosion,
    ShorterGrenadeCooldown,
    UnlockTeleport,
    ShorterTeleportCooldown,
    UnlockTeleportExplosion,
    LargerTeleportExplosion,
    UnlockHealthRegen,
    FasterHealthRegen,
    Retaliate,
    SlowerEnemies,
    Invincible,
}

impl PerkType {
    /// Chooses a number of random perks, given that the player already has certain perks.
    fn choose_random_perk_types(amount: usize, existing_perks: &[PerkType]) -> Vec<PerkType> {
        let has_more_perks = existing_perks.contains(&PerkType::MorePerks);
        let has_grenade = existing_perks.contains(&PerkType::UnlockGrenade);
        let has_teleport = existing_perks.contains(&PerkType::UnlockTeleport);
        let has_teleport_explosion = existing_perks.contains(&PerkType::UnlockTeleportExplosion);
        let has_health_regen = existing_perks.contains(&PerkType::UnlockHealthRegen);
        let has_retaliate = existing_perks.contains(&PerkType::Retaliate);
        let valid_perks = PerkType::iter().filter(|perk_type| match perk_type {
            PerkType::MorePerks => !has_more_perks,
            PerkType::UnlockGrenade => !has_grenade,
            PerkType::LargerGrenadeExplosion => has_grenade,
            PerkType::ShorterAttackCooldown => has_grenade,
            PerkType::UnlockTeleport => !has_teleport,
            PerkType::ShorterTeleportCooldown => has_teleport,
            PerkType::UnlockTeleportExplosion => has_teleport && !has_teleport_explosion,
            PerkType::LargerTeleportExplosion => has_teleport_explosion,
            PerkType::UnlockHealthRegen => !has_health_regen,
            PerkType::FasterHealthRegen => has_health_regen,
            PerkType::Retaliate => !has_retaliate,
            _ => true,
        });

        let mut rng = rand::thread_rng();
        valid_perks.choose_multiple(&mut rng, amount)
    }

    /// Gets the user-facing name and description of this perk type
    fn get_name_and_description(&self) -> (String, String) {
        let (name, desc) = match self {
            PerkType::LongerSword => ("Longer Sword", "Increases sword length by 10%"),
            PerkType::WiderSwordSwing => ("Wider Swing", "Increases sword swing arc by 10%"),
            PerkType::ShorterAttackCooldown => {
                ("Stronger Arms", "Decreases attack cooldown by 10%")
            }
            PerkType::HigherMaxSpeed => ("Stronger Legs", "Increases max run speed by 10%"),
            PerkType::HigherMaxHealth => ("Endurance", "Increases max health by 10%"),
            PerkType::MorePerks => (
                "Choosy",
                "Increases the number of perks to choose from each level by 1",
            ),
            PerkType::Heal => ("Second Wind", "Heals you to full health"),
            PerkType::UnlockGrenade => (
                "Secondary action: Grenade",
                "Allows you to throw grenades that do damage in an area. Replaces any existing secondary action you have.",
            ),
            PerkType::LargerGrenadeExplosion => (
                "Larger Grenades",
                "Increases grenade explosion radius by 10%",
            ),
            PerkType::ShorterGrenadeCooldown => {
                ("More Grenades", "Decreases grenade throw cooldown by 10%")
            }
            PerkType::UnlockTeleport => (
                "Secondary action: Teleport",
                "Allows you to teleport to the mouse cursor. Replaces any existing secondary action you have.",
            ),
            PerkType::ShorterTeleportCooldown => {
                ("More Teleporting", "Decreases the teleport cooldown by 10%")
            }
            PerkType::UnlockTeleportExplosion => ("Violent Teleportation", "When you teleport somewhere, you cause an explosion that kills enemies near your destination"),
            PerkType::LargerTeleportExplosion => ("More Violent Teleportation", "Increases teleportation explosion radius by 10%"),
            PerkType::UnlockHealthRegen => ("Resilient", "You will slowly regenerate health"),
            PerkType::FasterHealthRegen => ("More Resilient", "Increases health regeneration rate by 10%"),
            PerkType::Retaliate => ("Retaliation", "When an enemy hits you, they die"),
            PerkType::SlowerEnemies => ("Faster Reflexes", "All enemies move 5% slower"),
            PerkType::Invincible => ("Mind Over Matter", "Makes you invincible for a short period of time"),
        };

        (name.to_string(), desc.to_string())
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
struct SpawnIntervalChangeTimer(Timer);

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
pub struct Level {
    pub current_level: u64,
    pub current_xp: u64,
    previous_xp_needed: u64,
    xp_needed: u64,
}

impl Level {
    /// Advances to the next level
    fn advance(&mut self) {
        self.current_level += 1;

        let xp_since_last_level = self.xp_needed - self.previous_xp_needed;
        let additional_xp_needed = xp_since_last_level as f64 * NEXT_LEVEL_ADDITIONAL_XP_MULTIPLIER;

        self.previous_xp_needed = self.xp_needed;
        self.xp_needed += additional_xp_needed.round() as u64;
    }
}

#[derive(Resource)]
struct Health {
    current_health: u64,
    max_health: u64,
}

#[derive(Resource)]
struct SlowMoTimer {
    target_time_scale: f32,
    timer: Timer,
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
struct MaxSpeed(f32);

#[derive(Component)]
struct Perks(Vec<PerkType>);

impl Perks {
    /// Determines the number of perks to choose from on level up
    fn get_num_perk_choices(&self) -> usize {
        if self.0.contains(&PerkType::MorePerks) {
            NUM_PERK_CHOICES + 1
        } else {
            NUM_PERK_CHOICES
        }
    }
}

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
struct EnemyCountText;

#[derive(Component)]
struct HealthText;

#[derive(Event)]
struct LevelUp {
    new_level: u64,
}

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
    asset_server: Res<AssetServer>,
) {
    insert_starting_resources(&mut commands);

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

    let mut attack_cooldown = AttackCooldown(Timer::new(PLAYER_ATTACK_COOLDOWN, TimerMode::Once));
    attack_cooldown.0.set_elapsed(PLAYER_ATTACK_COOLDOWN);

    let sword_swing_params = SwordAnimationParams {
        start_delay: Duration::from_nanos(1),
        start_scale: SWORD_START_SCALE,
        end_scale: SWORD_END_SCALE,
        swing_time: SWORD_ANIMATION_TIME,
        start_rotation: SWORD_START_ROTATION,
        end_rotation: SWORD_END_ROTATION,
        start_translation: SWORD_START_TRANSLATION,
        end_translation: SWORD_END_TRANSLATION,
        send_swing_complete_event: false,
        swing_end_delay: SWORD_ANIMATION_END_DELAY,
        put_away_time: SWORD_PUT_AWAY_TIME,
        send_attack_done_event: false,
    };

    let mut sword_shadow_swing_params = SWORD_SHADOW_DELAYS_AND_ALPHAS
        .iter()
        .map(|(delay, alpha)| {
            (
                SwordAnimationParams {
                    start_delay: *delay,
                    start_scale: SWORD_START_SCALE,
                    end_scale: SWORD_END_SCALE,
                    swing_time: SWORD_ANIMATION_TIME,
                    start_rotation: SWORD_START_ROTATION,
                    end_rotation: SWORD_END_ROTATION,
                    start_translation: SWORD_START_TRANSLATION,
                    end_translation: SWORD_END_TRANSLATION,
                    send_swing_complete_event: false,
                    swing_end_delay: SWORD_ANIMATION_END_DELAY - *delay,
                    put_away_time: SWORD_PUT_AWAY_TIME,
                    send_attack_done_event: false,
                },
                *alpha,
            )
        })
        .collect::<Vec<(SwordAnimationParams, f32)>>();

    if let Some(last_params) = sword_shadow_swing_params.last_mut() {
        last_params.0.send_swing_complete_event = true;
        last_params.0.send_attack_done_event = true;
    }

    // player
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
        .insert(MaxSpeed(PLAYER_MAX_SPEED))
        .insert(Perks(Vec::new()))
        .insert(attack_cooldown)
        .with_children(|parent| {
            spawn_sword_pivot(parent, &mut meshes, &mut materials, sword_swing_params, 1.0);

            for (params, alpha) in sword_shadow_swing_params {
                spawn_sword_pivot(parent, &mut meshes, &mut materials, params, alpha);
            }
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

            // enemy count display
            parent
                .spawn(
                    TextBundle::from_section(
                        "Enemies: 0",
                        TextStyle {
                            font: asset_server.load(MONO_FONT),
                            font_size: 20.0,
                            color: Color::WHITE,
                        },
                    )
                    .with_text_alignment(TextAlignment::Center)
                    .with_style(Style {
                        margin: UiRect {
                            bottom: Val::Px(5.0),
                            ..default()
                        },
                        position_type: PositionType::Absolute,
                        bottom: Val::Px(5.0),
                        ..default()
                    }),
                )
                .insert(EnemyCountText);
        });
}

#[derive(Component)]
struct SwordAnimationParams {
    start_delay: Duration,
    start_scale: Vec3,
    end_scale: Vec3,
    swing_time: Duration,
    start_rotation: f32,
    end_rotation: f32,
    start_translation: Vec3,
    end_translation: Vec3,
    send_swing_complete_event: bool,
    swing_end_delay: Duration,
    put_away_time: Duration,
    send_attack_done_event: bool,
}

/// Builds the animation for a sword swing
fn build_sword_animation(params: &SwordAnimationParams) -> Sequence<Transform> {
    let mut rotate_tween = Tween::new(
        EaseFunction::QuadraticOut,
        params.swing_time,
        TransformRotateZLens {
            start: params.start_rotation,
            end: params.end_rotation,
        },
    );
    if params.send_swing_complete_event {
        rotate_tween = rotate_tween.with_completed_event(SWORD_SWING_COMPLETE_EVENT_ID);
    }

    let mut put_away_tween = Tween::new(
        EaseFunction::QuadraticIn,
        params.put_away_time,
        TransformScaleLens {
            start: params.end_scale,
            end: params.start_scale,
        },
    );
    if params.send_attack_done_event {
        put_away_tween = put_away_tween.with_completed_event(ATTACK_DONE_EVENT_ID);
    }

    Delay::new(params.start_delay)
        .then(Tween::new(
            EaseMethod::Discrete(0.0),
            Duration::from_nanos(1),
            TransformScaleLens {
                start: params.start_scale,
                end: params.end_scale,
            },
        ))
        .then(Tracks::new(vec![
            rotate_tween,
            Tween::new(
                EaseFunction::QuadraticOut,
                params.swing_time,
                TransformPositionLens {
                    start: params.start_translation,
                    end: params.end_translation,
                },
            ),
        ]))
        .then(Delay::new(params.swing_end_delay))
        .then(put_away_tween)
}

fn spawn_sword_pivot(
    parent: &mut ChildBuilder,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    animation_params: SwordAnimationParams,
    alpha: f32,
) {
    // pivot
    parent
        .spawn(SpatialBundle::from_transform(
            Transform::from_translation(SWORD_START_TRANSLATION)
                .with_scale(SWORD_START_SCALE)
                .with_rotation(Quat::from_rotation_z(SWORD_START_ROTATION)),
        ))
        .insert(SwordPivot)
        .insert(
            Animator::new(build_sword_animation(&animation_params))
                .with_state(AnimatorState::Paused),
        )
        .insert(animation_params)
        .with_children(|pivot| {
            // sword
            pivot
                .spawn(MaterialMesh2dBundle {
                    mesh: meshes
                        .add(shape::Quad::new(Vec2::new(SWORD_WIDTH, SWORD_LENGTH)).into())
                        .into(),
                    material: materials.add(ColorMaterial::from(Color::GRAY.with_a(alpha))),
                    transform: Transform::from_translation(Vec3::new(0., SWORD_LENGTH / 2.0, 0.)),
                    ..default()
                })
                .insert(Collider::cuboid(SWORD_WIDTH, SWORD_LENGTH / 2.0))
                .insert(Sensor)
                .insert(Sword { active: false });
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
            &MaxSpeed,
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

    for (mut force, mut velocity, mut transform, attacking, max_speed) in &mut player_query {
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
        velocity.linvel = velocity.linvel.clamp_length_max(max_speed.0);
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
    mut spawn_interval_change_timer: ResMut<SpawnIntervalChangeTimer>,
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

    spawn_interval_change_timer.0.tick(time.delta());
    if spawn_interval_change_timer.0.just_finished() {
        let new_duration = MIN_SPAWN_INTERVAL.max(
            spawn_timer
                .0
                .duration()
                .mul_f32(SPAWN_INTERVAL_CHANGE_MULTIPLIER),
        );
        spawn_timer.0.set_duration(new_duration)
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
    mut slow_mo_timer: ResMut<SlowMoTimer>,
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

                        slow_mo_timer.target_time_scale = HIT_SLOW_MO_TIME_SCALE;
                        slow_mo_timer.timer.set_duration(HIT_SLOW_MO_TIME);
                        slow_mo_timer.timer.reset();
                        slow_mo_timer.timer.unpause();
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
    mut level_up_events: EventWriter<LevelUp>,
    mut level_text_query: Query<&mut Text, (With<LevelText>, Without<XpText>)>,
    mut xp_text_query: Query<&mut Text, (With<XpText>, Without<LevelText>)>,
) {
    while level.current_xp >= level.xp_needed {
        level.advance();
        level_up_events.send(LevelUp {
            new_level: level.current_level,
        });
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

/// Keeps the enemy count display up to date
fn update_enemy_count_display(
    enemy_query: Query<&Enemy>,
    mut enemy_count_text_query: Query<&mut Text, With<EnemyCountText>>,
) {
    let enemy_count = enemy_query.iter().count();
    for mut text in enemy_count_text_query.iter_mut() {
        text.sections[0].value = format!("Enemies: {enemy_count}");
    }
}

/// Handles making the game go in slow motion temporarily
fn slow_mo(mut timer: ResMut<SlowMoTimer>, mut time: ResMut<Time<Virtual>>) {
    timer.timer.tick(time.delta());
    if timer.timer.paused() || timer.timer.finished() {
        time.set_relative_speed(1.0);
    } else {
        time.set_relative_speed(timer.target_time_scale);
    }
}

/// Handles doing things when the player levels up
fn level_up(
    mut level_up_events: EventReader<LevelUp>,
    mut zoom: ResMut<ZoomLevel>,
    mut sword_pivot_query: Query<
        (&mut SwordAnimationParams, &mut Animator<Transform>),
        With<SwordPivot>,
    >,
    mut player_query: Query<(&mut AttackCooldown, &mut MaxSpeed, &Perks), With<Player>>,
) {
    for event in level_up_events.read() {
        // zoom out a bit
        let new_zoom = MAX_ZOOM_LEVEL.min(zoom.0 * ZOOM_LEVEL_MULTIPLIER);
        zoom.0 = new_zoom;

        // make sword longer and swing wider
        // TODO remove
        for (mut swing_params, mut animator) in sword_pivot_query.iter_mut() {
            swing_params.end_scale.y *= 1.1;
            swing_params.start_rotation *= 1.1;
            swing_params.end_rotation *= 1.1;
            *animator =
                Animator::new(build_sword_animation(&swing_params)).with_state(animator.state);
        }

        // reduce attack cooldown and increase max speed
        // TODO remove
        /*
        for (mut cooldown, mut max_speed, perks) in player_query.iter_mut() {
            let new_duration = cooldown.0.duration().mul_f32(0.8);
            cooldown.0.set_duration(new_duration);

            let new_max_speed = max_speed.0 * 1.1;
            max_speed.0 = new_max_speed;
        }
        */

        // display perk chooser
        for (mut cooldown, mut max_speed, perks) in player_query.iter_mut() {
            let num_perk_choices = perks.get_num_perk_choices();
            let available_perks = PerkType::choose_random_perk_types(num_perk_choices, &perks.0);
            //TODO
        }
    }
}

/// Checks if the player is dead, and ends the game if they are
fn check_for_death(mut next_state: ResMut<NextState<GameState>>, health: Res<Health>) {
    if health.current_health == 0 {
        next_state.set(GameState::GameOver);
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
