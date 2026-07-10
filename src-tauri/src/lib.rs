mod api;
mod commands;

use commands::AppState;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::test_connection,
            commands::get_info,
            commands::get_metrics,
            commands::get_players,
            commands::get_settings,
            commands::get_game_data,
            commands::announce,
            commands::kick,
            commands::ban,
            commands::unban,
            commands::save_world,
            commands::shutdown,
            commands::force_stop,
            commands::save_connection,
            commands::load_connection
        ])
        .run(tauri::generate_context!())
        .expect("error while running Palworld Server Manager");
}
