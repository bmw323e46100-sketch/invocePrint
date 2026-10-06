//! Интеграционный тест: генерация PDF из тест-кейса спецификации.

use std::path::PathBuf;
use zayavka::{fmt, model, money_words, pdf};

fn build_spec_test_document() -> model::Document {
    use model::{Document, LineItem, Unit};
    Document {
        number: "DZHOД123456".to_string(),
        date: fmt::today_string(),
        lines: vec![
            LineItem {
                name: "Поршнекомплект с кольцами G4FA +0.5".to_string(),
                qty: "1".to_string(),
                unit: Unit::Kkt,
                price: "9100".to_string(),
            },
            LineItem {
                name: "Антифриз, Korea-Standard зелёный -37°C, 5кг".to_string(),
                qty: "1".to_string(),
                unit: Unit::Sht,
                price: "1780".to_string(),
            },
            LineItem {
                name: "HYUNDAI/KIA/MOBIS Клапан выпускной".to_string(),
                qty: "8".to_string(),
                unit: Unit::Sht,
                price: "190".to_string(),
            },
        ],
    }
}

#[test]
fn generates_pdf_with_test_case() {
    let doc = build_spec_test_document();
    assert_eq!(doc.filled_count(), 3);
    assert_eq!(doc.total(), 12400.0);
    assert_eq!(
        money_words::amount_to_words(doc.total()),
        "Двенадцать тысяч четыреста рублей 00 копеек"
    );

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

    let s = String::from_utf8_lossy(&bytes);
    assert!(
        s.contains("PTSerif") || s.contains("PT_Serif") || s.contains("PT Serif")
            || s.contains("PTSerif-Regular") || s.contains("ParaType"),
        "В PDF должно быть упоминание шрифта PT Serif"
    );

    assert!(
        s.contains("DZHOД"),
        "В PDF должен быть номер заявки (DZHOД...)"
    );
    assert!(s.contains("Заявка"), "В PDF должен быть заголовок «Заявка»");

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
