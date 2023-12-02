use bevy::audio::{Volume, VolumeLevel};
use bevy_asset_loader::{
    asset_collection::AssetCollection,
    loading_state::{LoadingState, LoadingStateAppExt},
};
use iyes_progress::{ProgressCounter, ProgressPlugin};

use crate::*;

const LOADING_FONT: &str = "fonts/MajorMonoDisplay-Regular.ttf";

const MOVE_LEFT_KEY: KeyCode = KeyCode::A;
const MOVE_RIGHT_KEY: KeyCode = KeyCode::D;
const MOVE_UP_KEY: KeyCode = KeyCode::W;
const MOVE_DOWN_KEY: KeyCode = KeyCode::S;

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
    }
}

#[derive(AssetCollection, Resource)]
pub struct ImageAssets {
    #[asset(path = "images/player.png")]
    player: Handle<Image>,
}

#[derive(AssetCollection, Resource)]
pub struct AudioAssets {
    #[asset(path = "sounds/background_music.ogg")]
    background_music: Handle<AudioSource>,
}

#[derive(Component)]
struct LoadingComponent;

#[derive(Component)]
struct LoadingText;

#[derive(Component)]
struct GameComponent;

#[derive(Component)]
struct BackgroundMusic;

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
fn game_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    todo!()
}

/// Starts playing the background music
fn start_background_music(mut commands: Commands, audio_assets: Res<AudioAssets>) {
    commands.spawn((
        AudioBundle {
            source: audio_assets.background_music.clone(),
            settings: PlaybackSettings::LOOP
                .with_volume(Volume::Relative(VolumeLevel::new(BG_MUSIC_VOLUME))),
        },
        BackgroundMusic,
    ));
}

/// Stops playing the background music
fn stop_background_music(music_controller: Query<&AudioSink, With<BackgroundMusic>>) {
    if let Ok(sink) = music_controller.get_single() {
        sink.stop();
    }
}
