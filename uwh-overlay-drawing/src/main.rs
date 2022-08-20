use std::{str::FromStr, sync::mpsc::channel};
//use uwh_common::game_snapshot::GamePeriod;
use network::StatePacket;
use std::net::IpAddr;

use macroquad::prelude::*;
use uwh_common::game_snapshot::{GamePeriod, GameSnapshot};
mod load_images;
mod network;
mod pages;

const APP_CONFIG_NAME: &str = "uwh-overlay-drawing";

fn window_conf() -> Conf {
    Conf {
        window_title: "UWH Overlay".to_owned(),
        window_width: 1920,
        window_height: 1080,
        window_resizable: false,
        ..Default::default()
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    refbox_ip: IpAddr,
    refbox_port: u64,
    uwhscores_url: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            refbox_ip: IpAddr::from_str("127.0.0.1").unwrap(),
            refbox_port: 8000,
            uwhscores_url: String::from("uwhscores.com"),
        }
    }
}

pub struct State {
    snapshot: GameSnapshot,
    black: String,
    white: String,
    w_flag: Option<Texture2D>,
    b_flag: Option<Texture2D>,
}

#[macroquad::main(window_conf)]
async fn main() {
    let (tx, rx) = channel::<StatePacket>();

    let config: AppConfig = match confy::load(APP_CONFIG_NAME, None) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to read config file, overwriting with default. Error: {e}");
            let config = AppConfig::default();
            confy::store(APP_CONFIG_NAME, None, &config).unwrap();
            config
        }
    };

    std::thread::spawn(|| {
        network::networking_thread(tx, config)
            .expect("Networking error. Does the supplied URL exist and is it live?")
    });

    let args: Vec<String> = std::env::args().collect();
    assert!(
        args.len() == 2,
        "Got {} args instead of one. Pass one argument, --color or --alpha to get the color or alpha feed respectively",
        args.len() - 1
    );

    let (textures, is_alpha_mode) = if args[1] == *"--color" {
        (load_images::Textures::init_color(), false)
    } else if args[1] == *"--alpha" {
        (load_images::Textures::init_alpha(), true)
    } else {
        panic!("Expected --color or --alpha arg!")
    };

    let mut game_state: Option<State> = None;

    // Should the goal graphic be displayed?
    // let mut show_goal = false;
    //keeps track of old whit and black scores in order to detect a change and show the goal graphic
    // let (mut b_score, mut w_score) = (0, 0);
    let mut renderer = pages::PageRenderer {
        animation_counter: 0f32,
        textures,
        is_alpha_mode,
    };

    loop {
        clear_background(RED);
        if let Ok(state) = rx.try_recv() {
            // Update state parameters like team names and flags if they are present.
            if let Some(game_state) = &mut game_state {
                game_state.w_flag = if state.w_flag.is_some() {
                    Some(Texture2D::from_file_with_format(
                        state.w_flag.unwrap(),
                        None,
                    ))
                } else {
                    game_state.w_flag
                };
                if let Some(team_name) = state.black {
                    game_state.black = team_name;
                }
                if let Some(team_name) = state.white {
                    game_state.white = team_name;
                }
                game_state.snapshot = state.snapshot;
            } else {
                // If `game_state` hasn't been init'd, just copy all the values over.
                game_state = Some(State {
                    white: state.white.unwrap(),
                    black: state.black.unwrap(),
                    w_flag: if let Some(flag) = state.w_flag {
                        Some(Texture2D::from_file_with_format(flag, None))
                    } else {
                        None
                    },
                    b_flag: if let Some(flag) = state.b_flag {
                        Some(Texture2D::from_file_with_format(flag, None))
                    } else {
                        None
                    },
                    snapshot: state.snapshot,
                })
            }
        }
        // if show_goal {
        //     if !is_alpha_mode {
        //         pages_color::show_goal_graphic(
        //             &textures,
        //             &mut secondary_animation_counter,
        //             &mut show_goal,
        //         );
        //     } else {
        //         pages_alpha::show_goal_graphic(
        //             &textures,
        //             &mut secondary_animation_counter,
        //             &mut show_goal,
        //         );
        //     }
        // }

        if let Some(state) = &game_state {
            // if state.snapshot.b_score != b_score || state.snapshot.w_score != w_score {
            //     w_score = state.snapshot.w_score;
            //     b_score = state.snapshot.b_score;
            //     show_goal = true;
            // }
            match state.snapshot.current_period {
                GamePeriod::BetweenGames => match state.snapshot.secs_in_period {
                    151..=u16::MAX => {
                        // If an old game just finished, display its scores for a minute
                        if state.snapshot.is_old_game {
                            renderer.final_scores(state);
                        } else {
                            renderer.next_game(state);
                        }
                    }
                    30..=150 => {
                        renderer.roster(state);
                    }
                    _ => {
                        renderer.pre_game_display(state);
                    }
                },
                GamePeriod::FirstHalf | GamePeriod::SecondHalf | GamePeriod::HalfTime => {
                    renderer.in_game_display(state);
                }
                GamePeriod::OvertimeFirstHalf
                | GamePeriod::OvertimeHalfTime
                | GamePeriod::OvertimeSecondHalf
                | GamePeriod::PreOvertime => {
                    renderer.overtime_display();
                }
                _ => {}
            }
        }
        next_frame().await;
    }
}
