// build.rs
//
// Автоматически скачивает шрифты Noto Serif (Regular + Bold) при первой
// сборке. Запускается cargo до компиляции.
//
// Источник: официальный репозиторий notofonts на GitHub через CDN jsDelivr.
// Лицензия: SIL OFL 1.1 (разрешает встраивание и распространение).
//
// Noto Serif выбран за самое широкое покрытие Unicode среди свободных
// шрифтов с засечками: поддерживает латиницу, кириллицу (включая
// расширенную), греческий, все знаки препинания, цифры, специальные
// символы (|, /, -, кавычки «», №, и т.д.) — гарантированно отображает
// любые символы, которые пользователь может ввести в наименовании товара.
//
// Шрифты сохраняются в OUT_DIR (официальное место для выходных данных
// build script) — это обходит sandbox Cargo, который не позволяет
// build script записывать файлы в произвольные места. Пути передаются
// в lib.rs через cargo:rustc-env.

use std::env;
use std::path::PathBuf;
use std::process::Command;

const FONTS: &[(&str, &str, &str)] = &[
    (
        "noto-serif-regular.ttf",
        "https://cdn.jsdelivr.net/gh/notofonts/notofonts.github.io@main/fonts/NotoSerif/hinted/ttf/NotoSerif-Regular.ttf",
        "FONT_REGULAR_PATH",
    ),
    (
        "noto-serif-bold.ttf",
        "https://cdn.jsdelivr.net/gh/notofonts/notofonts.github.io@main/fonts/NotoSerif/hinted/ttf/NotoSerif-Bold.ttf",
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
