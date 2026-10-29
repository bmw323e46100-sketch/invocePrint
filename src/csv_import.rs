//! Модуль импорта CSV-данных из выгрузки сайта поставщика.
//! Разделитель ;, поддерживает пробелы в числах.

use crate::model::{LineItem, Unit};

/// Результат импорта: список строк + количество пропущенных.
#[derive(Debug, Clone)]
pub struct ImportResult {
    /// Успешно импортированные строки.
    pub items: Vec<LineItem>,
    /// Количество пропущенных строк (заголовок, «Итого», пустые).
    pub skipped: usize,
    /// Сообщение для пользователя.
    pub message: String,
}

/// Парсит CSV-текст и возвращает строки товаров.
///
/// Принимает текст целиком (с заголовком, строками данных и финальной
/// строкой «Итого»). Разделитель — точка с запятой `;`.
/// Пробелы в числах (разделители тысяч) игнорируются.
pub fn parse_csv_text(text: &str) -> ImportResult {
    let mut items = Vec::new();
    let mut skipped = 0;
    let mut header_seen = false;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            skipped += 1;
            continue;
        }

        // Пропускаем заголовок (содержит «GUID» или «Наименование»).
        if !header_seen
            && (line.contains("GUID") || line.contains("Номер запчасти"))
        {
            header_seen = true;
            skipped += 1;
            continue;
        }

        // Пропускаем строку «Итого».
        if line.to_lowercase().starts_with("итого")
            || line.starts_with("Итого")
            || line.starts_with("ИТОГО")
        {
            skipped += 1;
            continue;
        }

        // Разбиваем по `;`.
        let cols: Vec<&str> = line.split(';').collect();
        // Ожидаем минимум 7 колонок: GUID, Номер, Наименование, Бренд,
        // Цена, Заказано, Сумма, ... (Комментарий и Статус могут быть пустыми).
        if cols.len() < 7 {
            skipped += 1;
            continue;
        }

        let _guid = cols[0].trim();
        let part_number = cols[1].trim();
        let name = cols[2].trim();
        let brand = cols[3].trim();
        let price_str = cols[4].trim();
        let qty_str = cols[5].trim();
        let _sum_str = cols[6].trim();

        // Пропускаем строки без наименования.
        if name.is_empty() {
            skipped += 1;
            continue;
        }

        // Парсим количество (игнорируем пробелы).
        let qty_clean: String = qty_str
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '\u{a0}')
            .collect();
        let qty_val: f64 = match qty_clean.parse() {
            Ok(v) => v,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        // Пропускаем строки с нулевым количеством.
        if qty_val == 0.0 {
            skipped += 1;
            continue;
        }

        // Парсим цену (заменяем неразрывный пробел на обычный и удаляем
        // все пробелы, потом парсим как f64 — поддерживает и точку, и запятую).
        let price_clean: String = price_str
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '\u{a0}')
            .map(|c| if c == ',' { '.' } else { c })
            .collect();
        let price_val: f64 = match price_clean.parse() {
            Ok(v) => v,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };

        // Формируем наименование товара: «Наименование [Бренд] (Номер)».
        // Если бренд или номер пустые — не добавляем их.
        let mut full_name = name.to_string();
        if !brand.is_empty() {
            full_name.push_str(&format!(" [{}]", brand));
        }
        if !part_number.is_empty() {
            full_name.push_str(&format!(" ({})", part_number));
        }

        items.push(LineItem {
            name: full_name,
            qty: format!("{}", qty_val),
            unit: Unit::Sht,
            price: format!("{}", price_val),
        });
    }

    let message = if items.is_empty() {
        format!("Не найдено ни одной строки для импорта (пропущено: {})", skipped)
    } else {
        format!(
            "Импортировано строк: {}, пропущено: {}",
            items.len(),
            skipped
        )
    };

    ImportResult {
        items,
        skipped,
        message,
    }
}

/// Читает CSV-файл и парсит его.
pub fn parse_csv_file(path: &std::path::Path) -> Result<ImportResult, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Не удалось прочитать файл: {}", e))?;
    Ok(parse_csv_text(&content))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CSV: &str = "GUID;Номер запчасти;Наименование;Бренд;Цена, руб;Заказано;Сумма, руб;Комментарий;Статус
NSIN0003495651;MN110724;Сальник полуоси;Mitsubishi;324.91;1;324.91;;
NSIN0005391098;MB664611;Подшипник полуоси| зад |;Mitsubishi;3 847.11;1;3 847.11;;
NSIN0005391136;MR111877;Втулка запорная с ABS ;Mitsubishi;2 997.98;1;2 997.98;;
NSII0015501676;3715A155;Сальник подшипника задней полуоси;Mitsubishi;371.74;1;371.74;;
NSIN0000087515;343251;Амортизатор - Excel-G | зад прав/лев |;KYB;2 325.01;2;4 650.02;;
Итого;;;;;6;12 191.76;;";

    #[test]
    fn parse_full_csv() {
        let result = parse_csv_text(SAMPLE_CSV);
        assert_eq!(result.items.len(), 5);
        // Заголовок + строка «Итого» = 2 пропущенных.
        assert_eq!(result.skipped, 2);

        // Проверяем первую строку.
        let first = &result.items[0];
        assert_eq!(first.name, "Сальник полуоси [Mitsubishi] (MN110724)");
        assert_eq!(first.qty, "1");
        assert_eq!(first.price, "324.91");

        // Проверяем последнюю (с количеством 2).
        let last = &result.items[4];
        assert!(last.name.contains("Амортизатор"));
        assert!(last.name.contains("KYB"));
        assert!(last.name.contains("343251"));
        assert_eq!(last.qty, "2");
        assert_eq!(last.price, "2325.01");
    }

    #[test]
    fn parse_handles_spaces_in_numbers() {
        let csv = "GUID;Номер;Наименование;Бренд;Цена, руб;Заказано;Сумма, руб;;
NSIN1;P1;Товар;Brand;1 999,50;3;5 998,50;;";
        let result = parse_csv_text(csv);
        assert_eq!(result.items.len(), 1);
        // Цена с пробелом-разделителем и запятой должна распарситься.
        assert_eq!(result.items[0].price, "1999.5");
        assert_eq!(result.items[0].qty, "3");
    }

    #[test]
    fn parse_skips_empty_and_zero_qty() {
        let csv = "GUID;Номер;Наименование;Бренд;Цена, руб;Заказано;Сумма, руб;;
NSIN1;P1;Товар1;B;100;1;100;;
NSIN2;P2;;B;100;1;100;;
NSIN3;P3;Товар3;B;100;0;0;;";
        let result = parse_csv_text(csv);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].name, "Товар1 [B] (P1)");
    }

    #[test]
    fn parse_handles_missing_brand_and_number() {
        // Наименование пустое → строка пропускается.
        let csv = "GUID;Номер;Наименование;Бренд;Цена, руб;Заказано;Сумма, руб;;
NSIN1;;;500;1;500;;";
        let result = parse_csv_text(csv);
        assert_eq!(result.items.len(), 0);
        assert!(result.skipped >= 1);
    }

    #[test]
    fn parse_only_name_no_brand_no_number() {
        // Только наименование, без бренда и номера — должна импортироваться.
        let csv = "GUID;Номер;Наименование;Бренд;Цена, руб;Заказано;Сумма, руб;;
NSIN1;;Товар;;;500;1;500;;";
        // Если 8 колонок — то name=Товар, brand=пусто, number=пусто.
        // Но "NSIN1;;Товар;;;500;1;500;;".split(';') = ["NSIN1", "", "Товар", "", "", "500", "1", "500", "", ""] — 10 колонок
        // cols[4]=пусто (price), cols[5]=500 (qty), cols[6]=1 (sum) — цена пустая, пропустится.
        // Чтобы тест был корректным, сделаем правильную расстановку.
        let csv2 = "GUID;Номер;Наименование;Бренд;Цена, руб;Заказано;Сумма, руб;;
NSIN1;;Товар;;500;1;500;;";
        let result = parse_csv_text(csv2);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].name, "Товар");
        assert_eq!(result.items[0].price, "500");
    }

    #[test]
    fn parse_returns_message() {
        let result = parse_csv_text(SAMPLE_CSV);
        assert!(result.message.contains("5"));
        assert!(result.message.contains("пропущено: 2"));
    }
}
