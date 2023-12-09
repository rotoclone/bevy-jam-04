use crate::*;

pub struct GameOverPlugin;

impl Plugin for GameOverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::GameOver), game_over_setup)
            .add_systems(
                OnExit(GameState::GameOver),
                despawn_components_system::<GameOverComponent>,
            )
            .add_systems(Update, restart_button_system);
    }
}

#[derive(Component)]
struct GameOverComponent;

#[derive(Component)]
struct RestartButton;

fn game_over_setup(mut commands: Commands, level: Res<Level>, asset_server: Res<AssetServer>) {
    let legacy_message = if level.current_level < 4 {
        "You will be forgotten."
    } else if level.current_level < 10 {
        "Your efforts were not in vain, but you will be forgotten."
    } else if level.current_level < 15 {
        "You will be remembered."
    } else {
        "Your heroic feats will be remembered for all time."
    };

    commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ..default()
        })
        .insert(GameOverComponent)
        .with_children(|parent| {
            // text
            parent.spawn(
                TextBundle::from_section(
                    format!(
                        "You perished at level {} with {} XP.\n{}",
                        level.current_level, level.current_xp, legacy_message
                    ),
                    TextStyle {
                        font: asset_server.load(MAIN_FONT),
                        font_size: 50.0,
                        color: Color::WHITE,
                    },
                )
                .with_text_alignment(TextAlignment::Center)
                .with_style(Style {
                    margin: UiRect::bottom(Val::Px(5.0)),
                    ..default()
                }),
            );

            // restart button
            parent
                .spawn(NodeBundle {
                    style: Style {
                        // center button
                        width: Val::Percent(100.00),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    ..default()
                })
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
                        .insert(RestartButton)
                        .with_children(|parent| {
                            parent.spawn(TextBundle::from_section(
                                "Restart",
                                TextStyle {
                                    font: asset_server.load(MAIN_FONT),
                                    font_size: 40.0,
                                    color: NORMAL_BUTTON_TEXT_COLOR,
                                },
                            ));
                        });
                });
        });
}

type InteractedRestartButtonTuple = (Changed<Interaction>, With<RestartButton>);

/// Handles interactions with the restart button.
fn restart_button_system(
    mut next_state: ResMut<NextState<GameState>>,
    interaction_query: Query<&Interaction, InteractedRestartButtonTuple>,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            next_state.set(GameState::GameLoading);
        }
    }
}
