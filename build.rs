// build.rs
//
// Автоматически скачивает шрифты PT Serif (Regular + Bold) при первой
// сборке, если их нет в assets/fonts/. Запускается cargo до компиляции.
//
// Источник: официальный репозиторий google/fonts на GitHub через CDN jsDelivr.
// Лицензия: SIL OFL 1.1 (разрешает встраивание и распространение).
// PT Serif специально разработан ParaType для кириллицы — отлично подходит
// для русскоязычных документов.

use std::env;
use std::path::PathBuf;
use std::process::Command;

const FONTS: &[(&str, &str)] = &[
    (
        "pt-serif-regular.ttf",
        "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/ptserif/PT_Serif-Web-Regular.ttf",
    ),
    (
        "pt-serif-bold.ttf",
        "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/ptserif/PT_Serif-Web-Bold.ttf",
    ),
];

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let fonts_dir = manifest_dir.join("assets").join("fonts");
    std::fs::create_dir_all(&fonts_dir).expect("не удалось создать assets/fonts");

    for (name, url) in FONTS {
        let path = fonts_dir.join(name);
        // Считаем файл валидным, если он больше 100 КБ.
        let already_ok = path.exists()
            && std::fs::metadata(&path).map(|m| m.len() > 100_000).unwrap_or(false);
        if already_ok {
            continue;
        }
        println!("cargo:warning=Скачиваю шрифт {} ...", name);
        let ok = download(url, &path);
        if !ok {
            panic!(
                "Не удалось скачать шрифт {} с {}.\n\
                 Скачайте вручную и положите в assets/fonts/{}",
                name, url, name
            );
        }
        println!("cargo:warning=Шрифт {} скачан ({} байт).",
            name,
            std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0));
    }

    println!("cargo:rerun-if-changed=build.rs");
}

fn download(url: &str, dest: &PathBuf) -> bool {
    // Способ 1: PowerShell (Windows). Invoke-WebRequest есть начиная с PowerShell 3.0.
    if cfg!(target_os = "windows") {
        let script = format!(
            "try {{ \
                [Net.ServicePointManager]::SecurityProtocol = \
                    [Net.SecurityProtocolType]::Tls12; \
                Invoke-WebRequest -Uri '{}' -OutFile '{}' -UseBasicParsing \
            }} catch {{ exit 1 }}",
            url,
            dest.display()
        );
        let status = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .status();
        if let Ok(s) = status {
            if s.success() && dest.exists() {
                return true;
            }
        }
        // Способ 2: curl.exe (встроен в Windows 10 1803+ и Windows 11).
        let status = Command::new("curl")
            .args(["-fSL", "--tlsv1.2", "-o", &dest.display().to_string(), url])
            .status();
        if let Ok(s) = status {
            if s.success() && dest.exists() {
                return true;
            }
        }
        return false;
    }

    // На Unix: сначала curl, затем wget.
    let status = Command::new("curl")
        .args(["-fSL", "-o", &dest.display().to_string(), url])
        .status();
    if let Ok(s) = status {
        if s.success() && dest.exists() {
            return true;
        }
    }
    let status = Command::new("wget")
        .args(["-q", "-O", &dest.display().to_string(), url])
        .status();
    if let Ok(s) = status {
        if s.success() && dest.exists() {
            return true;
        }
    }
    false
}
