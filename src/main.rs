mod prelude;
use crate::prelude::*;
mod board;
mod config;
mod devtools;
mod loading;
#[cfg(test)]
mod performance;
mod states;
mod tiles;
mod word_bank;

use bevy::app::AppExit;
use bevy::window::ExitCondition;

fn main() -> AppExit {
    let options = match devtools::RunOptions::from_env() {
        Ok(options) => options,
        Err(error) => {
            eprintln!("Invalid startup options: {error}");
            return AppExit::error();
        }
    };
    let headless = options.headless();
    let mut plugins = DefaultPlugins.set(WindowPlugin {
        primary_window: (!headless).then(|| options.window()),
        exit_condition: if headless {
            ExitCondition::DontExit
        } else {
            ExitCondition::OnAllClosed
        },
        ..default()
    });
    if headless {
        plugins = plugins.disable::<bevy::winit::WinitPlugin>();
    }
    let mut app = App::new();
    app.add_plugins(plugins)
        .init_state::<AppState>()
        .add_plugins((loading::LoadingPlugin, tiles::TilesPlugin))
        .add_systems(Startup, board::spawn_board);
    options.configure(&mut app);
    app.run()
}
