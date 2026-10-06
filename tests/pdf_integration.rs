//! Интеграционный тест: генерация PDF из тест-кейса спецификации.

use std::path::PathBuf;
use zayavka::{fmt, model, money_words, pdf};

fn build_spec_test_document() -> model::Document {
    use model::{Document, LineItem, Unit};
    Document {
        number: "DZHOД123456".to_string(),
        date: fmt::today_string(),
        lines: vec![
            // Специальные символы (|, /, -, латиница, кириллица) —
            // проверяем, что Noto Serif их корректно отображает.
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

#[test]
fn generates_pdf_with_test_case() {
    let doc = build_spec_test_document();
    assert_eq!(doc.filled_count(), 3);
    // 2500*1 + 350*4 + 9100*1 = 2500 + 1400 + 9100 = 13000
    assert_eq!(doc.total(), 13000.0);

    let tmp_dir = std::env::temp_dir();
    let path: PathBuf = tmp_dir.join("zayavka_test_spec.pdf");
    let _ = std::fs::remove_file(&path);

    pdf::generate_pdf(&doc, &path).expect("PDF generation should succeed");

    assert!(path.exists(), "PDF file should exist at {}", path.display());

    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    assert!(
        size > 50_000,
        "PDF file too small ({} bytes) — шрифт не вшит?",
        size
    );

    let bytes = std::fs::read(&path).expect("read pdf");
    assert!(bytes.starts_with(b"%PDF-"), "PDF должен начинаться с %PDF-");

    // Проверяем, что PDF содержит встроенные шрифты (Type0/CIDFont —
    // стандартный способ встраивания TTF в PDF через printpdf).
    let s = String::from_utf8_lossy(&bytes);
    assert!(
        s.contains("Type0") || s.contains("CIDFont") || s.contains("FontFile"),
        "В PDF должны быть встроенные шрифты (Type0/CIDFont/FontFile)"
    );

    // Извлекаем текст через pdftotext (если доступен) — это надёжнее,
    // чем прямой поиск по байтам PDF, потому что printpdf использует
    // CID-кодировку и текст не виден как ASCII/UTF-8.
    let extracted = std::process::Command::new("pdftotext")
        .args(["-layout", &path.display().to_string(), "-"])
        .output()
        .ok()
        .and_then(|o| if o.status.success() { Some(o.stdout) } else { None })
        .map(|b| String::from_utf8_lossy(&b).to_string())
        .unwrap_or_default();

    // Если pdftotext недоступен (например, на Windows без poppler),
    // проверяем только базовые вещи через s.
    let check_text = |needle: &str, msg: &str| {
        if !extracted.is_empty() {
            assert!(extracted.contains(needle), "{} (текст PDF: {})", msg, extracted);
        } else {
            // Fallback: ищем по байтам (может не сработать для русского текста).
            // Пропускаем проверку, если pdftotext недоступен.
        }
    };

    assert!(
        s.contains("DZHOД"),
        "В PDF должен быть номер заявки (DZHOД...)"
    );
    assert!(s.contains("Заявка"), "В PDF должен быть заголовок «Заявка»");
    // Проверяем нового поставщика.
    check_text("Автоконтракты", "В PDF должен быть поставщик ООО «Автоконтракты»");
    // Проверяем отсутствие ИНН.
    if !extracted.is_empty() && extracted.contains("ИНН") {
        panic!("В PDF не должно быть поля ИНН, но оно найдено: {}", extracted);
    }
    // Проверяем графы подписей вместо кассира.
    check_text("Отпустил", "В PDF должна быть графа «Отпустил»");
    check_text("Получил", "В PDF должна быть графа «Получил»");
    if !extracted.is_empty() && extracted.contains("Кассир") {
        panic!("В PDF не должно быть поля «Кассир», но оно найдено: {}", extracted);
    }
    // Проверяем, что специальные символы из наименований товаров
    // корректно отображаются в PDF (это было причиной перехода на Noto Serif).
    check_text("|", "В PDF должен отображаться символ |");
    check_text("/", "В PDF должен отображаться символ /");
    check_text("LADA", "В PDF должно быть наименование LADA");
    check_text("Bosch", "В PDF должно быть наименование Bosch");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn empty_document_still_generates_pdf() {
    use model::{Document, LineItem};
    let doc = Document {
        number: "DZHOД000000".to_string(),
        date: "1 января 2026 г.".to_string(),
        lines: vec![LineItem::empty()],
    };
    assert_eq!(doc.filled_count(), 0);
    assert_eq!(doc.total(), 0.0);

    let path: PathBuf = std::env::temp_dir().join("zayavka_test_empty.pdf");
    let _ = std::fs::remove_file(&path);

    pdf::generate_pdf(&doc, &path).expect("PDF generation should succeed even for empty doc");
    assert!(path.exists());

    let _ = std::fs::remove_file(&path);
}

#[test]
fn spec_amount_words_and_format() {
    assert_eq!(fmt::format_money(9100.0), "9\u{a0}100,00");
    assert_eq!(fmt::format_money(1780.0), "1\u{a0}780,00");
    assert_eq!(fmt::format_money(1520.0), "1\u{a0}520,00");
    assert_eq!(fmt::format_money(12400.0), "12\u{a0}400,00");
    assert_eq!(fmt::format_rubles_rounded(12400.0), "12\u{a0}400");
    assert_eq!(
        money_words::amount_to_words(12400.0),
        "Двенадцать тысяч четыреста рублей 00 копеек"
    );
}
