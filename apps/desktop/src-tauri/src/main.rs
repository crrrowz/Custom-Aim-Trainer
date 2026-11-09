#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use aim_analysis::SessionDetail;
use aim_coach::drills::CustomTargets;
use aimforge_desktop::core::{CoachConfig, Engine, ProcessRunner, Settings, View};
use std::sync::Mutex;
use tauri::Manager;

struct Shared(Mutex<Engine<ProcessRunner>>);

/// Runs engine work off the UI thread, because a trainer session blocks until the player finishes.
async fn with_engine<T: Send + 'static>(
    app: tauri::AppHandle,
    work: impl FnOnce(&mut Engine<ProcessRunner>) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Shared>();
        let mut engine = state
            .0
            .lock()
            .map_err(|_| "engine lock poisoned".to_string())?;
        work(&mut engine)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_view(app: tauri::AppHandle) -> Result<View, String> {
    with_engine(app, |e| e.view()).await
}

#[tauri::command]
async fn save_settings(app: tauri::AppHandle, settings: Settings) -> Result<View, String> {
    with_engine(app, move |e| {
        e.save_settings(&settings)?;
        e.view()
    })
    .await
}

#[tauri::command]
async fn run_assessment(app: tauri::AppHandle) -> Result<View, String> {
    with_engine(app, |e| {
        e.run_assessment()?;
        e.view()
    })
    .await
}

#[tauri::command]
async fn accept_plan(app: tauri::AppHandle) -> Result<View, String> {
    with_engine(app, |e| {
        e.accept_plan()?;
        e.view()
    })
    .await
}

#[tauri::command]
async fn run_next_item(app: tauri::AppHandle) -> Result<View, String> {
    with_engine(app, |e| {
        e.run_next_item()?;
        e.view()
    })
    .await
}

#[tauri::command]
async fn run_retest(app: tauri::AppHandle) -> Result<View, String> {
    with_engine(app, |e| {
        e.run_retest()?;
        e.view()
    })
    .await
}

#[tauri::command]
async fn next_block(app: tauri::AppHandle) -> Result<View, String> {
    with_engine(app, |e| {
        e.next_block()?;
        e.view()
    })
    .await
}

#[tauri::command]
async fn run_custom(app: tauri::AppHandle, targets: CustomTargets) -> Result<View, String> {
    with_engine(app, move |e| {
        e.run_custom(&targets)?;
        e.view()
    })
    .await
}

#[tauri::command]
async fn run_scenario(app: tauri::AppHandle, scenario: String) -> Result<View, String> {
    with_engine(app, move |e| {
        e.run_scenario(&scenario)?;
        e.view()
    })
    .await
}

#[tauri::command]
async fn session_detail(app: tauri::AppHandle, path: String) -> Result<SessionDetail, String> {
    with_engine(app, move |e| e.session_detail(&path)).await
}

#[tauri::command]
async fn generate_report(app: tauri::AppHandle, scope: String) -> Result<View, String> {
    with_engine(app, move |e| {
        e.generate_report(&scope)?;
        e.view()
    })
    .await
}

#[tauri::command]
async fn delete_session(app: tauri::AppHandle, path: String) -> Result<View, String> {
    with_engine(app, move |e| {
        e.delete_session(&path)?;
        e.view()
    })
    .await
}

#[tauri::command]
async fn delete_report(app: tauri::AppHandle, at_ms: u64) -> Result<View, String> {
    with_engine(app, move |e| {
        e.delete_report(at_ms)?;
        e.view()
    })
    .await
}

#[tauri::command]
async fn set_ai_key(app: tauri::AppHandle, provider: String, key: String) -> Result<View, String> {
    with_engine(app, move |e| {
        e.set_ai_key(&provider, &key)?;
        e.view()
    })
    .await
}

#[tauri::command]
async fn test_ai(app: tauri::AppHandle) -> Result<String, String> {
    with_engine(app, |e| e.test_ai()).await
}

#[tauri::command]
async fn get_coach_config(app: tauri::AppHandle) -> Result<CoachConfig, String> {
    with_engine(app, |e| e.coach_config()).await
}

#[tauri::command]
async fn save_coach_config(app: tauri::AppHandle, config: CoachConfig) -> Result<(), String> {
    with_engine(app, move |e| e.save_coach_config(&config)).await
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let engine = Engine::open(&data_dir, ProcessRunner::locate())
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            app.manage(Shared(Mutex::new(engine)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_view,
            save_settings,
            run_assessment,
            accept_plan,
            run_next_item,
            run_retest,
            next_block,
            run_custom,
            run_scenario,
            session_detail,
            generate_report,
            delete_session,
            delete_report,
            set_ai_key,
            test_ai,
            get_coach_config,
            save_coach_config
        ])
        .run(tauri::generate_context!())
        .expect("failed to start AimForge");
}
