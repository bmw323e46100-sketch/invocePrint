// build.rs
//
// Автоматически скачивает шрифты DejaVu Serif (Regular + Bold) при первой
// сборке. Запускается cargo до компиляции.
//
// Источник: npm-пакет dejavu-fonts-ttf через CDN jsDelivr.
// Лицензия: свободная (DejaVu Fonts License, совместима с GPL/BSD).
//
// DejaVu Serif выбран за самое широкое покрытие Unicode среди свободных
// шрифтов с засечками: 3447 глифов (против ~1000 у PT Serif и ~2000 у Noto
// Serif). Поддерживает латиницу, кириллицу (включая расширенную), греческий,
// математику, знаки препинания, технические символы — практически всё,
// что пользователь может ввести в наименовании товара.
//
// Шрифты сохраняются в OUT_DIR (официальное место для выходных данных
// build script) — это обходит sandbox Cargo, который не позволяет
// build script записывать файлы в произвольные места. Пути передаются
// в lib.rs через cargo:rustc-env.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const FONTS: &[(&str, &str, &str)] = &[
    (
        "dejavu-serif-regular.ttf",
        "https://cdn.jsdelivr.net/npm/dejavu-fonts-ttf@2.37.3/ttf/DejaVuSerif.ttf",
        "FONT_REGULAR_PATH",
    ),
    (
        "dejavu-serif-bold.ttf",
        "https://cdn.jsdelivr.net/npm/dejavu-fonts-ttf@2.37.3/ttf/DejaVuSerif-Bold.ttf",
        "FONT_BOLD_PATH",
    ),
];

fn main() {
    // OUT_DIR — официальный каталог для выходных данных build script.
    // Cargo гарантированно позволяет сюда писать.
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let fonts_dir = out_dir.join("fonts");
    std::fs::create_dir_all(&fonts_dir).expect("не удалось создать fonts dir");

    for (name, url, env_var) in FONTS {
        let path = fonts_dir.join(name);
        let already_ok = path.exists()
            && std::fs::metadata(&path).map(|m| m.len() > 100_000).unwrap_or(false);
        if !already_ok {
            println!("cargo:warning=Скачиваю шрифт {} ...", name);
            let ok = download(url, &path);
            if !ok {
                panic!(
                    "Не удалось скачать шрифт {} с {}.\n\
                     Скачайте вручную и положите в: {}",
                    name, url, path.display()
                );
            }
            println!("cargo:warning=Шрифт {} скачан ({} байт).",
                name,
                std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0));
        }
        // Передаём абсолютный путь к шрифту в lib.rs через окружение.
        // include_bytes!(env!("...")) использует этот путь при компиляции.
        println!("cargo:rustc-env={}={}", env_var, path.display());
    }

    println!("cargo:rerun-if-changed=build.rs");
}

fn download(url: &str, dest: &Path) -> bool {
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
