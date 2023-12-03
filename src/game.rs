use std::time::Duration;

use bevy::{
    audio::{Volume, VolumeLevel},
    input::common_conditions::input_just_pressed,
    sprite::MaterialMesh2dBundle,
};
use bevy_asset_loader::{
    asset_collection::AssetCollection,
    loading_state::{LoadingState, LoadingStateAppExt},
};
use bevy_rapier2d::dynamics::{
    AdditionalMassProperties, Damping, ExternalForce, ExternalImpulse, GravityScale,
    MassProperties, RigidBody, Velocity,
};
use bevy_tweening::{
    lens::{TransformPositionLens, TransformRotateZLens},
    Animator, AnimatorState, Delay, EaseFunction, Sequence, Tracks, Tween, TweenCompleted,
};
use iyes_progress::{ProgressCounter, ProgressPlugin};

use crate::*;

const LOADING_FONT: &str = "fonts/MajorMonoDisplay-Regular.ttf";

const PLAYER_SIZE: f32 = 5.0;
const PLAYER_MAX_SPEED: f32 = 70.0;
const PLAYER_MOVE_FORCE: f32 = 100000.0;
const PLAYER_DAMPING: f32 = 12.0;
const PLAYER_MASS: f32 = 100.0;
const PLAYER_INERTIA: f32 = 16000.0;

const SWORD_WIDTH: f32 = 1.0;
const SWORD_LENGTH: f32 = 12.0;

const PLAYER_ATTACK_COOLDOWN: Duration = Duration::from_millis(1000);
const SWORD_SWING_ROTATION_DEGREES: f32 = 60.0;
const SWORD_SWING_TRANSLATION: f32 = 2.0;

const SWORD_ANIMATION_TIME: Duration = Duration::from_millis(150);
const SWORD_ANIMATION_END_DELAY: Duration = Duration::from_millis(150);

const SWORD_SWING_COMPLETE_EVENT_ID: u64 = 1;

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

        app.add_systems(
            Update,
            (
                update_attack_cooldown.before(player_attack),
                player_movement,
                player_attack.run_if(input_just_pressed(ATTACK_INPUT)),
                tween_completed,
            ),
        );
    }
}

#[derive(AssetCollection, Resource)]
pub struct ImageAssets {
    /* TODO
    #[asset(path = "images/player.png")]
    player: Handle<Image>,
    */
}

#[derive(AssetCollection, Resource)]
pub struct AudioAssets {
    /* TODO
    #[asset(path = "sounds/background_music.ogg")]
    background_music: Handle<AudioSource>,
    */
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
    asset_server: Res<AssetServer>,
) {
    // player
    let mut attack_cooldown = AttackCooldown(Timer::new(PLAYER_ATTACK_COOLDOWN, TimerMode::Once));
    attack_cooldown.0.set_elapsed(PLAYER_ATTACK_COOLDOWN);

    let sword_swing_tween = Sequence::new(vec![Tracks::new(vec![
        Tween::new(
            EaseFunction::QuadraticOut,
            SWORD_ANIMATION_TIME,
            TransformRotateZLens {
                start: f32::to_radians(SWORD_SWING_ROTATION_DEGREES / 2.0),
                end: -f32::to_radians(SWORD_SWING_ROTATION_DEGREES / 2.0),
            },
        ),
        Tween::new(
            EaseFunction::QuadraticOut,
            SWORD_ANIMATION_TIME,
            TransformPositionLens {
                start: Vec3::new(-SWORD_SWING_TRANSLATION / 2.0, 0.0, 0.0),
                end: Vec3::new(SWORD_SWING_TRANSLATION / 2.0, 0.0, 0.0),
            },
        ),
    ])])
    .then(
        Delay::new(SWORD_ANIMATION_END_DELAY).with_completed_event(SWORD_SWING_COMPLETE_EVENT_ID),
    );

    commands
        .spawn(MaterialMesh2dBundle {
            mesh: meshes.add(shape::Circle::new(PLAYER_SIZE).into()).into(),
            material: materials.add(ColorMaterial::from(Color::PURPLE)),
            transform: Transform::from_translation(Vec3::new(0., 0., 0.)),
            ..default()
        })
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
        .insert(attack_cooldown)
        .with_children(|parent| {
            // sword pivot
            parent
                .spawn(SpatialBundle::from_transform(Transform::from_translation(
                    Vec3::new(0.0, PLAYER_SIZE * 0.5, 0.0),
                )))
                .insert(SwordPivot)
                .insert(Animator::new(sword_swing_tween).with_state(AnimatorState::Paused))
                .with_children(|pivot| {
                    // sword
                    pivot.spawn(MaterialMesh2dBundle {
                        mesh: meshes
                            .add(shape::Quad::new(Vec2::new(SWORD_WIDTH, SWORD_LENGTH)).into())
                            .into(),
                        material: materials.add(ColorMaterial::from(Color::PURPLE)),
                        transform: Transform::from_translation(Vec3::new(
                            0.,
                            SWORD_LENGTH / 2.0,
                            0.,
                        )),
                        ..default()
                    });
                });
        });
}

/// Handles events for completed tweens
fn tween_completed(
    mut reader: EventReader<TweenCompleted>,
    mut sword_pivot_query: Query<&mut Visibility, With<SwordPivot>>,
) {
    for ev in reader.read() {
        if ev.user_data == SWORD_SWING_COMPLETE_EVENT_ID {
            for mut visibility in sword_pivot_query.iter_mut() {
                *visibility = Visibility::Hidden;
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
    mut player_query: Query<(&mut ExternalForce, &mut Velocity, &mut Transform), With<Player>>,
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

    for (mut force, mut velocity, mut transform) in &mut player_query {
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

        // rotation
        let to_cursor = (cursor_world_position - transform.translation.xy()).normalize();
        let rotate_to_cursor = Quat::from_rotation_arc(Vec3::Y, to_cursor.extend(0.));
        transform.rotation = rotate_to_cursor;

        // clamp speed
        velocity.linvel = velocity.linvel.clamp_length_max(PLAYER_MAX_SPEED);
    }
}

/// Makes the player attack
fn player_attack(
    mut player_query: Query<&mut AttackCooldown, With<Player>>,
    mut sword_query: Query<(&mut Animator<Transform>, &mut Visibility), With<SwordPivot>>,
) {
    for mut cooldown in player_query.iter_mut() {
        if !cooldown.0.finished() {
            continue;
        }

        //TODO only animate the sword for this particular player somehow
        for (mut animator, mut visibility) in sword_query.iter_mut() {
            animator.stop();
            animator.state = AnimatorState::Playing;

            *visibility = Visibility::Inherited;
        }

        cooldown.0.reset();
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
