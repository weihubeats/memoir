pub mod commands;

use my_context_core as core;
use tauri::Manager;

pub struct AppState {
    pub ctx: core::Ctx,
    pub db_path: String,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let db_path = core::db::default_db_path()?;
            let ctx =
                tauri::async_runtime::block_on(core::Ctx::open(&db_path, core::embed::from_env()))?;
            app.manage(AppState {
                ctx,
                db_path: db_path.display().to_string(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::db_info,
            commands::list_entities,
            commands::list_facts,
            commands::list_entries,
            commands::list_audit,
            commands::stale_facts,
            commands::search_entries,
            commands::upsert_fact,
            commands::add_entry,
            commands::add_entity,
            commands::monthly_report,
            commands::export_data,
            commands::get_context_preview,
            commands::list_trash,
            commands::delete_entry,
            commands::restore_entry,
            commands::purge_entry,
            commands::delete_fact,
            commands::restore_fact,
            commands::purge_fact,
            commands::delete_entity,
            commands::restore_entity,
            commands::purge_entity,
            commands::merge_entities,
        ])
        .run(tauri::generate_context!())
        .expect("error while running my-context desktop app");
}
