use crate::*;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Menu), menu_setup)
            .add_systems(
                OnExit(GameState::Menu),
                despawn_components_system::<MenuComponent>,
            )
            .add_systems(Update, start_button_system);
    }
}

#[derive(Component)]
struct MenuComponent;

#[derive(Component)]
struct StartButton;

fn menu_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // title text
    commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(50.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ..default()
        })
        .insert(MenuComponent)
        .with_children(|parent| {
            parent.spawn(
                TextBundle::from_section(
                    "LAST STAND",
                    TextStyle {
                        font: asset_server.load(TITLE_FONT),
                        font_size: 90.0,
                        color: Color::rgb(0.9, 0.2, 0.2),
                    },
                )
                .with_text_alignment(TextAlignment::Center)
                .with_style(Style {
                    margin: UiRect::all(Val::Auto),
                    ..default()
                }),
            );
        });

    // start button
    commands
        .spawn(NodeBundle {
            style: Style {
                // center button
                width: Val::Percent(100.00),
                position_type: PositionType::Absolute,
                top: Val::Percent(50.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ..default()
        })
        .insert(MenuComponent)
        .with_children(|parent| {
            parent
                .spawn(ButtonBundle {
                    style: Style {
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        padding: UiRect::all(Val::Px(10.0)),
                        ..default()
                    },
                    background_color: NORMAL_BUTTON.into(),
                    ..default()
                })
                .insert(StartButton)
                .with_children(|parent| {
                    parent.spawn(TextBundle::from_section(
                        "Begin",
                        TextStyle {
                            font: asset_server.load(MAIN_FONT),
                            font_size: 40.0,
                            color: NORMAL_BUTTON_TEXT_COLOR,
                        },
                    ));
                });
        });
}

type InteractedStartButtonTuple = (Changed<Interaction>, With<StartButton>);

/// Handles interactions with the start button.
fn start_button_system(
    mut next_state: ResMut<NextState<GameState>>,
    interaction_query: Query<&Interaction, InteractedStartButtonTuple>,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            next_state.set(GameState::GameLoading);
        }
    }
}
