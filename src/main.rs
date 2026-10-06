//! Точка входа приложения «Заявка на отгрузку».
//!
//! На Windows в release-сборке скрывает консольное окно.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use std::path::PathBuf;
use zayavka::gui;
use zayavka::APP_NAME;

fn main() -> eframe::Result<()> {
    // Пути: черновик и PDF сохраняются рядом с исполняемым файлом,
    // если это возможно; иначе — в каталоге данных пользователя.
    let (draft_path, pdf_dir) = resolve_paths();

    let app = gui::App::new(draft_path, pdf_dir);

    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([gui::WINDOW_WIDTH, gui::WINDOW_HEIGHT])
        .with_min_inner_size([640.0, 480.0])
        .with_title(format!("Заявка на отгрузку — {}", APP_NAME));

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        native_options,
        Box::new(move |_cc| Ok(Box::new(app))),
    )
}

/// Определяет пути для черновика и PDF:
/// - предпочтительно: рядом с исполняемым файлом (current_exe).
/// - запас: каталог данных приложения (home/.local/share/zayavka или %APPDATA%/zayavka).
fn resolve_paths() -> (PathBuf, PathBuf) {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));

    if let Some(dir) = exe_dir {
        // Если каталог exe доступен для записи — используем его.
        let probe = dir.join(".write_test_zayavka");
        if std::fs::write(&probe, b"").is_ok() {
            let _ = std::fs::remove_file(&probe);
            let draft = dir.join("zayavka_draft.json");
            let pdf_dir = dir.join("Заявки");
            return (draft, pdf_dir);
        }
    }

    // Запасной вариант: каталог данных пользователя.
    let base = dirs_or_appdata();
    let _ = std::fs::create_dir_all(&base);
    let draft = base.join("zayavka_draft.json");
    let pdf_dir = base.join("Заявки");
    (draft, pdf_dir)
}

/// Возвращает каталог данных приложения в зависимости от платформы.
fn dirs_or_appdata() -> PathBuf {
    // Linux: ~/.local/share/zayavka
    // Windows: %APPDATA%/zayavka
    // macOS: ~/Library/Application Support/zayavka
    if let Some(home) = std::env::var_os("HOME") {
        let p = PathBuf::from(home).join(".local/share/zayavka");
        if std::fs::create_dir_all(&p).is_ok() {
            return p;
        }
    }
    if cfg!(target_os = "windows") {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let p = PathBuf::from(appdata).join("zayavka");
            return p;
        }
    }
    PathBuf::from("./zayavka_data")
}
