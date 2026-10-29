//! Snapshot-тест генерации PDF.
//!
//! Защищает от случайных изменений выходного PDF при рефакторинге.
//!
//! **Нормализация таймстампов.** `printpdf` встраивает в PDF временную метку
//! создания: поля `CreationDate`, `ModDate` и XMP-метаданные
//! `xmp:CreateDate` / `xmp:ModifyDate`. Байт-в-байт сравнение падало бы при
//! каждом запуске. Поэтому перед сравнением все таймстампы заменяются на
//! плейсхолдер `<TS>`. После этого сравнение стабильное.
//!
//! Принцип:
//! - При первом запуске (или если эталон отсутствует) — сохраняем
//!   нормализованный PDF как эталон и просим перезапустить тест.
//! - При последующих — нормализуем текущий вывод и сравниваем с эталоном.
//! - Если поведение PDF **намеренно** изменилось:
//!     UPDATE_SNAPSHOTS=1 cargo test --test snapshot_pdf

use std::fs;
use std::path::PathBuf;
use zayavka::{model, pdf};

fn build_spec_document() -> model::Document {
    use model::{Document, LineItem, Unit};
    Document {
        number: "DZHOД123456".to_string(),
        date: "6 октября 2026 г.".to_string(),
        lines: vec![
            LineItem {
                name: "Подшипник ступицы колеса | зад прав/лев | LADA 2108-099/2110-12/2113-15/GRANTA/KALINA/PRIORA".to_string(),
                qty: "1".to_string(),
                unit: Unit::Sht,
                price: "2500".to_string(),
            },
            LineItem {
                name: "Свеча зажигания Bosch 0242235666".to_string(),
                qty: "4".to_string(),
                unit: Unit::Sht,
                price: "350".to_string(),
            },
            LineItem {
                name: "Поршнекомплект с кольцами G4FA +0.5".to_string(),
                qty: "1".to_string(),
                unit: Unit::Kkt,
                price: "9100".to_string(),
            },
        ],
    }
}

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
}

fn snapshot_path() -> PathBuf {
    snapshot_dir().join("spec_v1.pdf")
}

#[test]
fn pdf_matches_snapshot() {
    let doc = build_spec_document();
    assert_eq!(doc.filled_count(), 3);
    assert_eq!(doc.total(), 13000.0);

    let dir = snapshot_dir();
    fs::create_dir_all(&dir).expect("не удалось создать tests/snapshots");

    let actual_path = dir.join("spec_v1_actual.pdf");
    let _ = fs::remove_file(&actual_path);

    pdf::generate_pdf(&doc, &actual_path).expect("PDF generation should succeed");

    let actual_bytes =
        fs::read(&actual_path).expect("не удалось прочитать только что созданный PDF");
    assert!(actual_bytes.starts_with(b"%PDF-"), "PDF должен начинаться с %PDF-");
    assert!(
        actual_bytes.len() > 50_000,
        "PDF подозрительно мал: {} байт — возможно, шрифт не встроен",
        actual_bytes.len()
    );

    let normalized_bytes = normalize_pdf_timestamps(&actual_bytes);
    let snapshot_path = snapshot_path();

    if std::env::var("UPDATE_SNAPSHOTS").as_deref() == Ok("1") {
        fs::write(&snapshot_path, &normalized_bytes).expect("запись эталона");
        eprintln!(
            " эталон обновлён: {} ({} байт, нормализованных)",
            snapshot_path.display(),
            normalized_bytes.len()
        );
        let _ = fs::remove_file(&actual_path);
        return;
    }

    if !snapshot_path.exists() {
        fs::write(&snapshot_path, &normalized_bytes).expect("создание эталона");
        panic!(
            " Эталон отсутствовал — создан: {} ({} байт, нормализованных).\n\
             Перезапустите тест без UPDATE_SNAPSHOTS, чтобы проверить совпадение.",
            snapshot_path.display(),
            normalized_bytes.len()
        );
    }

    let snapshot_bytes =
        fs::read(&snapshot_path).expect("не удалось прочитать эталонный PDF");

    if normalized_bytes != snapshot_bytes {
        let actual_hash = sha256_hex(&normalized_bytes);
        let snapshot_hash = sha256_hex(&snapshot_bytes);

        let diff_path = dir.join("spec_v1_actual_diff.pdf");
        let _ = fs::remove_file(&diff_path);
        fs::copy(&actual_path, &diff_path).ok();

        panic!(
            "PDF отличается от эталона (после нормализации таймстампов)!\n\
             \n  Эталон:  {} байт  sha256: {}\n\
             \n  Текущий: {} байт  sha256: {}\n\
             \n  Копия текущего сохранена: {}\n\
             \n  Если изменение намеренное:\n\
             \n    UPDATE_SNAPSHOTS=1 cargo test --test snapshot_pdf\n\
             \n  и закоммитьте новый эталон `tests/snapshots/spec_v1.pdf`.",
            snapshot_bytes.len(),
            snapshot_hash,
            normalized_bytes.len(),
            actual_hash,
            diff_path.display(),
        );
    } else {
        let _ = fs::remove_file(&actual_path);
    }
}

/// Заменяет все таймстампы в PDF на плейсхолдер `<TS>`.
///
/// `printpdf` пишет таймстампы в трёх местах:
/// 1. XMP-метаданные: `2026-10-29T10:25:27+00'00'` (ISO 8601).
/// 2. Инфо-словарь: `D:20261029102527+00'00'` (PDF-формат даты).
///
/// Оба варианта содержат подстроку `+00'00'`. Ищем её и захватываем назад
/// до начала даты (первая нецифра после отступа).
fn normalize_pdf_timestamps(input: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(input.len());
    let mut i = 0;
    let mut last_emit_end: usize = 0;
    while i < input.len() {
        if input[i] == b'+'
            && i + 6 < input.len()
            && input[i + 1] == b'0'
            && input[i + 2] == b'0'
            && input[i + 3] == b'\''
            && input[i + 4] == b'0'
            && input[i + 5] == b'0'
            && input[i + 6] == b'\''
        {
            let mut j = i;
            while j > last_emit_end {
                let prev = input[j - 1];
                if prev.is_ascii_digit()
                    || prev == b'-'
                    || prev == b'T'
                    || prev == b':'
                    || prev == b'D'
                    || prev == b'+'
                {
                    j -= 1;
                    if i - j > 60 {
                        break;
                    }
                } else {
                    break;
                }
            }
            while j < i && !input[j].is_ascii_digit() {
                j += 1;
            }
            out.extend_from_slice(&input[last_emit_end..j]);
            out.extend_from_slice(b"<TS>");
            last_emit_end = i + 7;
            i = last_emit_end;
        } else {
            i += 1;
        }
    }
    out.extend_from_slice(&input[last_emit_end..]);
    out
}

/// SHA-256 без внешних зависимостей (только для отчёта о расхождении).
fn sha256_hex(data: &[u8]) -> String {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    let k: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(k[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let mj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(mj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut out = String::with_capacity(64);
    for w in h.iter() {
        out.push_str(&format!("{:08x}", w));
    }
    out
}
