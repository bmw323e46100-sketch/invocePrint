//! Точка входа приложения «Заявка на отгрузку».
//!
//! На Windows в release-сборке скрывает консольное окно.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use std::path::PathBuf;
use zayavka::gui;
use zayavka::APP_NAME;

fn main() -> eframe::Result<()> {
    // Пути: черновик, PDF и история сохраняются рядом с исполняемым файлом,
    // если это возможно; иначе — в каталоге данных пользователя.
    let (draft_path, pdf_dir, history_dir) = resolve_paths();

    let app = gui::App::new(draft_path, pdf_dir, history_dir);

    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([gui::WINDOW_WIDTH, gui::WINDOW_HEIGHT])
        .with_min_inner_size([640.0, 480.0])
        .with_title(format!("Заявка на отгрузку — {}", APP_NAME));

    // Используем glow-рендер (OpenGL) вместо wgpu по умолчанию.
    // Это избегает конфликта версий крейта `windows` (0.58 vs 0.62) в
    // транзитивных зависимостях wgpu-hal на Windows, который приводит
    // к ошибкам компиляции. Glow-рендер легче, стабильнее и совместим
    // со всеми ОС (Windows 10/11, Linux, macOS).
    let renderer = eframe::Renderer::Glow;

    let native_options = eframe::NativeOptions {
        viewport,
        renderer,
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        native_options,
        Box::new(move |_cc| Ok(Box::new(app))),
    )
}

/// Определяет пути для черновика, PDF и истории:
/// - предпочтительно: рядом с исполняемым файлом (current_exe).
/// - запас: каталог данных приложения через крейт `dirs`
///   (Linux: ~/.local/share/zayavka, Windows: %APPDATA%/zayavka,
///    macOS: ~/Library/Application Support/zayavka).
fn resolve_paths() -> (PathBuf, PathBuf, PathBuf) {
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
            let history_dir = dir.join("История");
            return (draft, pdf_dir, history_dir);
        }
    }

    // Запасной вариант: каталог данных пользователя через крейт `dirs`.
    // Раньше здесь была ручная реализация dirs_or_appdata() с тремя
    // ветками (Linux/Windows/macOS) — теперь используем проверенный крейт.
    // Если по какой-то причине dirs::data_dir() вернул None (нет HOME
    // и нет %APPDATA%) — откатываемся к ./zayavka_data в текущей директории.
    let base = dirs::data_dir()
        .map(|d| d.join("zayavka"))
        .unwrap_or_else(|| PathBuf::from("./zayavka_data"));
    let _ = std::fs::create_dir_all(&base);
    let draft = base.join("zayavka_draft.json");
    let pdf_dir = base.join("Заявки");
    let history_dir = base.join("История");
    (draft, pdf_dir, history_dir)
}
