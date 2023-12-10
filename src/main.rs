use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    input::common_conditions::input_toggle_active,
    prelude::*,
    window::{WindowResized, WindowResolution},
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_rapier2d::plugin::{NoUserData, RapierConfiguration, RapierPhysicsPlugin};
use bevy_tweening::TweeningPlugin;
use bevy_wasm_window_resize::WindowResizePlugin;
use smooth_bevy_cameras::{LookTransform, LookTransformBundle, LookTransformPlugin, Smoother};

mod menu;
use menu::*;

mod game;
use game::*;

mod game_over;
use game_over::*;

const DEV_MODE: bool = true;

const WINDOW_WIDTH: f32 = 1280.0;
const WINDOW_HEIGHT: f32 = 720.0;

const NORMAL_BUTTON_TEXT_COLOR: Color = Color::rgb(0.9, 0.9, 0.9);

const NORMAL_BUTTON: Color = Color::rgb(0.25, 0.25, 0.25);
const HOVERED_BUTTON: Color = Color::rgb(0.35, 0.35, 0.35);
const PRESSED_BUTTON: Color = Color::rgb(0.35, 0.75, 0.35);

const TITLE_FONT: &str = "fonts/SofiaSans-Light.ttf";
const MAIN_FONT: &str = "fonts/SofiaSans-Light.ttf";
const MONO_FONT: &str = "fonts/MajorMonoDisplay-Regular.ttf";

const MASTER_VOLUME: f32 = 0.5;
const STARTING_ZOOM_LEVEL: f32 = 0.33;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum GameState {
    #[default]
    Menu,
    GameLoading,
    Game,
    GameOver,
}

#[derive(Component)]
pub struct MainCamera;

#[derive(Resource)]
pub struct ZoomLevel(pub f32);

#[derive(Component)]
pub struct DisabledButton;

fn main() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::BLACK))
        .insert_resource(Msaa::Sample4)
        .insert_resource(GlobalVolume::new(MASTER_VOLUME))
        .insert_resource(ZoomLevel(STARTING_ZOOM_LEVEL))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "There's Too Many Of Them".into(),
                resolution: WindowResolution::new(WINDOW_WIDTH, WINDOW_HEIGHT),
                // Tells wasm to resize the window according to the available canvas
                fit_canvas_to_parent: true,
                // Tells wasm not to override default event handling, like F5, Ctrl+R etc.
                prevent_default_event_handling: false,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0))
        .insert_resource(RapierConfiguration {
            gravity: Vec2::ZERO,
            ..default()
        })
        .add_plugins(WindowResizePlugin)
        .add_plugins(TweeningPlugin)
        .add_plugins(LookTransformPlugin)
        .add_state::<GameState>()
        .add_systems(Startup, setup)
        .add_plugins((MenuPlugin, GamePlugin, GameOverPlugin))
        .add_systems(Update, (zoom_based_on_window_size, button_color_system));

    if DEV_MODE {
        app.add_systems(Update, bevy::window::close_on_esc)
            .add_plugins(LogDiagnosticsPlugin::default())
            .add_plugins(FrameTimeDiagnosticsPlugin)
            .add_plugins(
                WorldInspectorPlugin::new().run_if(input_toggle_active(false, KeyCode::Equals)),
            );
    }

    app.run();
}

fn setup(mut commands: Commands) {
    commands
        .spawn(LookTransformBundle {
            transform: LookTransform::new(Vec3::new(0.0, 0.0, 100.0), Vec3::ZERO, Vec3::Y),
            smoother: Smoother::new(0.9),
        })
        .insert(Camera2dBundle::default())
        .insert(MainCamera);
}

/// Adjusts the camera zoom when the window is resized
fn zoom_based_on_window_size(
    mut camera_query: Query<&mut OrthographicProjection, With<MainCamera>>,
    window_query: Query<&Window>,
    zoom_level: Res<ZoomLevel>,
    mut resize_reader: EventReader<WindowResized>,
) {
    let mut projection = camera_query.single_mut();

    let mut base_scale = (WINDOW_WIDTH / window_query.single().width())
        .min(WINDOW_HEIGHT / window_query.single().height());
    /*
    for event in resize_reader.read() {
        base_scale = (WINDOW_WIDTH / event.width).max(WINDOW_HEIGHT / event.height);
    }
    */

    projection.scale = base_scale * zoom_level.0;
}

type InteractedButtonTuple = (Changed<Interaction>, With<Button>, Without<DisabledButton>);

/// Handles changing button colors when they're interacted with.
fn button_color_system(
    mut interaction_query: Query<(&Interaction, &mut BackgroundColor), InteractedButtonTuple>,
) {
    for (interaction, mut color) in interaction_query.iter_mut() {
        *color = match *interaction {
            Interaction::Pressed => PRESSED_BUTTON.into(),
            Interaction::Hovered => HOVERED_BUTTON.into(),
            Interaction::None => NORMAL_BUTTON.into(),
        }
    }
}

/// Generic system that takes a component as a parameter, and will despawn all entities with that component
fn despawn_components_system<T: Component>(
    to_despawn: Query<Entity, With<T>>,
    mut commands: Commands,
) {
    despawn_components(to_despawn, &mut commands);
}

fn despawn_components<T: Component>(to_despawn: Query<Entity, With<T>>, commands: &mut Commands) {
    for entity in to_despawn.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
