use std::path::PathBuf;
use zayavka::{fmt, model, money_words, pdf};
use model::{Document, LineItem, Unit};

fn main() {
    let doc = Document {
        number: "DZHOД123456".to_string(),
        date: fmt::today_string(),
        lines: vec![
            // Пример из запроса пользователя — с символами |, /, эмодзи для проверки фильтрации.
            LineItem {
                name: "Подшипник полуоси| зад | [Mitsubishi] (MB664611) 🚀".to_string(),
                qty: "1".to_string(),
                unit: Unit::Sht,
                price: "3847.11".to_string(),
            },
            LineItem {
                name: "Амортизатор - Excel-G | зад прав/лев | [KYB] (343251)".to_string(),
                qty: "2".to_string(),
                unit: Unit::Sht,
                price: "2325.01".to_string(),
            },
            LineItem {
                name: "Сальник полуоси 🇯🇵 你好 [Mitsubishi] (MN110724)".to_string(),
                qty: "1".to_string(),
                unit: Unit::Sht,
                price: "324.91".to_string(),
            },
        ],
    };

    println!("Итого: {}", fmt::format_money(doc.total()));
    println!("Прописью: {}", money_words::amount_to_words(doc.total()));

    // Проверим фильтрацию символов.
    println!("\n--- Проверка фильтрации символов ---");
    let test_strings = [
        "Подшипник | зад | 🚀",
        "Амортизатор зад прав/лев",
        "Сальник 🇯🇵 你好",
        "№ 123 ±0.5 °C × ÷ «кавычки»",
    ];
    for s in &test_strings {
        let filtered = pdf::filter_unsupported(s);
        println!("  '{}' → '{}'", s, filtered);
    }

    let path: PathBuf = PathBuf::from("sample_output.pdf");
    match pdf::generate_pdf(&doc, &path) {
        Ok(()) => println!("\nPDF saved: {}", path.display()),
        Err(e) => {
            println!("ERROR: {}", e);
            std::process::exit(1);
        }
    }

    let bytes = std::fs::read(&path).unwrap();
    println!("Size: {} bytes", bytes.len());
}
