use std::path::PathBuf;
use zayavka::{fmt, model, money_words, pdf};
use model::{Document, LineItem, Unit};

fn main() {
    let doc = Document {
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
    };

    println!("Итого: {}", fmt::format_money(doc.total()));
    println!("Прописью: {}", money_words::amount_to_words(doc.total()));

    let path: PathBuf = PathBuf::from("sample_output.pdf");
    match pdf::generate_pdf(&doc, &path) {
        Ok(()) => println!("PDF saved: {}", path.display()),
        Err(e) => {
            println!("ERROR: {}", e);
            std::process::exit(1);
        }
    }

    let bytes = std::fs::read(&path).unwrap();
    println!("Size: {} bytes", bytes.len());
}
