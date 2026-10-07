//! Библиотечная часть приложения «Заявка на отгрузку».
//!
//! Весь код собран в один файл, чтобы пользователю не приходилось
//! создавать множество отдельных файлов модулей.
//!
//! Содержит:
//! - [`fmt`] — форматирование чисел и дат.
//! - [`model`] — структуры данных и расчёт итогов.
//! - [`money_words`] — сумма прописью.
//! - [`pdf`] — генерация PDF-документа.
//! - [`gui`] — графический интерфейс на egui.

/// Имя приложения для интерфейса.
pub const APP_NAME: &str = "Заявка на отгрузку";

// ============================================================================
// МОДУЛЬ fmt — форматирование чисел и дат
// ============================================================================

/// Модуль форматирования чисел и дат по русским правилам.
///
/// - Разделитель тысяч — неразрывный пробел (U+00A0).
/// - Десятичный разделитель — запятая, два знака после запятой.
/// - В полях ввода принимаются и точка, и запятая как десятичный разделитель.
pub mod fmt {
    use chrono::{Datelike, Local, NaiveDate};

    /// Неразрывный пробел — разделитель разрядов в русской типографике.
    pub const NBSP: char = '\u{a0}';

    /// Месяцы в родительном падеже, строчными буквами.
    const MONTHS_RU_GENITIVE: [&str; 12] = [
        "января", "февраля", "марта", "апреля", "мая", "июня",
        "июля", "августа", "сентября", "октября", "ноября", "декабря",
    ];

    /// Форматирует денежную сумму в русской нотации.
    /// Пример: `47100.0` → `"47 100,00"` (с неразрывным пробелом).
    pub fn format_money(value: f64) -> String {
        let neg = value < 0.0;
        let abs = value.abs();
        let total_kopecks = (abs * 100.0).round() as i64;
        let rubles = total_kopecks / 100;
        let kopecks = total_kopecks % 100;

        let mut s = format_int_groups(rubles);
        s.push(',');
        s.push_str(&format!("{:02}", kopecks));
        if neg { format!("-{}", s) } else { s }
    }

    /// Форматирует целое число с разделителем разрядов (неразрывный пробел).
    pub fn format_int_groups(n: i64) -> String {
        let neg = n < 0;
        let mut n = n.unsigned_abs() as u64;
        if n == 0 { return "0".to_string(); }
        let mut groups: Vec<u64> = Vec::new();
        while n > 0 {
            groups.push(n % 1000);
            n /= 1000;
        }
        groups.reverse();
        let mut s = String::new();
        for (i, g) in groups.iter().enumerate() {
            if i > 0 {
                s.push(NBSP);
                s.push_str(&format!("{:03}", g));
            } else {
                s.push_str(&format!("{}", g));
            }
        }
        if neg { s = format!("-{}", s); }
        s
    }

    /// Округляет до целого рубля и форматирует с разделителем разрядов.
    pub fn format_rubles_rounded(value: f64) -> String {
        let rounded = value.round() as i64;
        format_int_groups(rounded)
    }

    /// Разбирает пользовательский ввод: принимает и точку, и запятую,
    /// игнорирует пробелы и неразрывные пробелы.
    pub fn parse_number(s: &str) -> Option<f64> {
        let cleaned: String = s
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '\u{a0}')
            .map(|c| if c == ',' { '.' } else { c })
            .collect();
        if cleaned.is_empty() { return None; }
        cleaned.parse::<f64>().ok()
    }

    /// Форматирует дату: «25 сентября 2026 г.».
    pub fn format_date_ru(date: NaiveDate) -> String {
        let day = date.day();
        let month_idx = date.month() as usize;
        let year = date.year();
        format!("{} {} {} г.", day, MONTHS_RU_GENITIVE[month_idx - 1], year)
    }

    /// Текущая системная дата в формате `format_date_ru`.
    pub fn today_string() -> String {
        format_date_ru(Local::now().date_naive())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn money_basic() {
            assert_eq!(format_money(47100.0), "47\u{a0}100,00");
            assert_eq!(format_money(0.0), "0,00");
            assert_eq!(format_money(1.5), "1,50");
            assert_eq!(format_money(1234.5), "1\u{a0}234,50");
        }

        #[test]
        fn money_negative() {
            assert_eq!(format_money(-1234.5), "-1\u{a0}234,50");
        }

        #[test]
        fn money_rounding() {
            assert_eq!(format_money(0.005), "0,01");
            assert_eq!(format_money(0.004), "0,00");
            assert_eq!(format_money(12400.0), "12\u{a0}400,00");
        }

        #[test]
        fn money_test_case_from_spec() {
            assert_eq!(format_money(9100.0), "9\u{a0}100,00");
            assert_eq!(format_money(1780.0), "1\u{a0}780,00");
            assert_eq!(format_money(1520.0), "1\u{a0}520,00");
            assert_eq!(format_money(12400.0), "12\u{a0}400,00");
        }

        #[test]
        fn money_million() {
            assert_eq!(format_money(1_000_000.0), "1\u{a0}000\u{a0}000,00");
        }

        #[test]
        fn parse_accepts_comma_and_dot() {
            assert_eq!(parse_number("9100"), Some(9100.0));
            assert_eq!(parse_number("9100,00"), Some(9100.0));
            assert_eq!(parse_number("9100.00"), Some(9100.0));
            assert_eq!(parse_number("9 100,00"), Some(9100.0));
            assert_eq!(parse_number("9\u{a0}100,00"), Some(9100.0));
            assert_eq!(parse_number(" 12 400 "), Some(12400.0));
        }

        #[test]
        fn parse_invalid_returns_none() {
            assert_eq!(parse_number(""), None);
            assert_eq!(parse_number("   "), None);
            assert_eq!(parse_number("abc"), None);
        }

        #[test]
        fn rubles_rounded_format() {
            assert_eq!(format_rubles_rounded(12400.0), "12\u{a0}400");
            assert_eq!(format_rubles_rounded(12400.40), "12\u{a0}400");
            assert_eq!(format_rubles_rounded(12400.50), "12\u{a0}401");
            assert_eq!(format_rubles_rounded(47100.0), "47\u{a0}100");
        }

        #[test]
        fn date_format_basic() {
            let d = NaiveDate::from_ymd_opt(2026, 9, 25).unwrap();
            assert_eq!(format_date_ru(d), "25 сентября 2026 г.");
        }

        #[test]
        fn date_all_months() {
            let months = [
                "января", "февраля", "марта", "апреля", "мая", "июня",
                "июля", "августа", "сентября", "октября", "ноября", "декабря",
            ];
            for (i, m) in months.iter().enumerate() {
                let d = NaiveDate::from_ymd_opt(2026, (i + 1) as u32, 15).unwrap();
                assert!(format_date_ru(d).contains(m), "month {} failed", i + 1);
            }
        }

        #[test]
        fn date_no_leading_zero() {
            let d = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
            assert_eq!(format_date_ru(d), "5 января 2026 г.");
        }
    }
}

// ============================================================================
// МОДУЛЬ money_words — сумма прописью
// ============================================================================

/// Модуль преобразования суммы в слова на русском языке.
///
/// Эталон: `47100.00` → «Сорок семь тысяч сто рублей 00 копеек».
pub mod money_words {
    /// Возвращает сумму прописью для переданного значения в рублях.
    pub fn amount_to_words(value: f64) -> String {
        let total_kopecks = (value * 100.0).round() as i64;
        let rubles = total_kopecks.div_euclid(100);
        let kopecks = total_kopecks.rem_euclid(100);

        let mut s = rubles_to_words(rubles);
        if let Some(first) = s.chars().next() {
            let upper: String = first.to_uppercase().collect();
            s = format!("{}{}", upper, &s[first.len_utf8()..]);
        }
        s.push(' ');
        s.push_str(&kopecks_word(kopecks));
        s
    }

    fn ruble_word(n: i64) -> &'static str { declension_ru(n, "рубль", "рубля", "рублей") }
    fn thousand_word(n: i64) -> &'static str { declension_ru(n, "тысяча", "тысячи", "тысяч") }
    fn million_word(n: i64) -> &'static str { declension_ru(n, "миллион", "миллиона", "миллионов") }

    fn kopecks_word(kopecks: i64) -> String {
        let word = declension_ru(kopecks, "копейка", "копейки", "копеек");
        format!("{:02} {}", kopecks, word)
    }

    fn declension_ru(n: i64, one: &'static str, few: &'static str, many: &'static str) -> &'static str {
        let n = n.unsigned_abs();
        let last_two = n % 100;
        let last = n % 10;
        if last_two >= 11 && last_two <= 14 { return many; }
        match last {
            1 => one,
            2 | 3 | 4 => few,
            _ => many,
        }
    }

    fn rubles_to_words(rubles: i64) -> String {
        if rubles == 0 { return "0 рублей".to_string(); }
        let mut parts: Vec<String> = Vec::new();
        let mut n = rubles;
        let mut millions = 0i64;
        let mut thousands = 0i64;
        let units;
        if n >= 1_000_000 { millions = n / 1_000_000; n %= 1_000_000; }
        if n >= 1_000 { thousands = n / 1_000; n %= 1_000; }
        units = n;
        if millions > 0 {
            parts.push(format!("{} {}", three_digit_words(millions, Gender::Masculine), million_word(millions)));
        }
        if thousands > 0 {
            parts.push(format!("{} {}", three_digit_words(thousands, Gender::Feminine), thousand_word(thousands)));
        }
        if units > 0 {
            parts.push(format!("{} {}", three_digit_words(units, Gender::Masculine), ruble_word(units)));
        } else if !parts.is_empty() {
            parts.push("рублей".to_string());
        }
        parts.join(" ")
    }

    #[derive(Clone, Copy, PartialEq)]
    enum Gender { Masculine, Feminine }

    fn three_digit_words(n: i64, gender: Gender) -> String {
        let n = n as u32;
        let mut out: Vec<&'static str> = Vec::new();
        let hundreds = n / 100;
        let tens = (n % 100) / 10;
        let ones = n % 10;
        if hundreds > 0 { out.push(HUNDREDS[hundreds as usize]); }
        if tens == 1 {
            out.push(TENS_TEENS[ones as usize]);
        } else {
            if tens > 0 { out.push(TENS[tens as usize]); }
            if ones > 0 {
                if gender == Gender::Feminine {
                    out.push(ONES_FEM[ones as usize]);
                } else {
                    out.push(ONES_MASC[ones as usize]);
                }
            }
        }
        out.join(" ")
    }

    const HUNDREDS: [&str; 10] = [
        "", "сто", "двести", "триста", "четыреста", "пятьсот", "шестьсот",
        "семьсот", "восемьсот", "девятьсот",
    ];
    const TENS: [&str; 10] = [
        "", "", "двадцать", "тридцать", "сорок", "пятьдесят", "шестьдесят",
        "семьдесят", "восемьдесят", "девяносто",
    ];
    const TENS_TEENS: [&str; 10] = [
        "десять", "одиннадцать", "двенадцать", "тринадцать", "четырнадцать",
        "пятнадцать", "шестнадцать", "семнадцать", "восемнадцать", "девятнадцать",
    ];
    const ONES_MASC: [&str; 10] = [
        "", "один", "два", "три", "четыре", "пять", "шесть", "семь", "восемь", "девять",
    ];
    const ONES_FEM: [&str; 10] = [
        "", "одна", "две", "три", "четыре", "пять", "шесть", "семь", "восемь", "девять",
    ];

    #[cfg(test)]
    mod tests {
        use super::*;

        fn words(v: f64) -> String { amount_to_words(v) }

        #[test]
        fn zero() { assert_eq!(words(0.0), "0 рублей 00 копеек"); }
        #[test]
        fn one_ruble() { assert_eq!(words(1.0), "Один рубль 00 копеек"); }
        #[test]
        fn two_rubles() { assert_eq!(words(2.0), "Два рубля 00 копеек"); }
        #[test]
        fn five_rubles() { assert_eq!(words(5.0), "Пять рублей 00 копеек"); }
        #[test]
        fn eleven_rubles() { assert_eq!(words(11.0), "Одиннадцать рублей 00 копеек"); }
        #[test]
        fn twenty_one_ruble() { assert_eq!(words(21.0), "Двадцать один рубль 00 копеек"); }
        #[test]
        fn twenty_two_rubles() { assert_eq!(words(22.0), "Двадцать два рубля 00 копеек"); }
        #[test]
        fn one_hundred() { assert_eq!(words(100.0), "Сто рублей 00 копеек"); }
        #[test]
        fn one_hundred_one() { assert_eq!(words(101.0), "Сто один рубль 00 копеек"); }
        #[test]
        fn one_thousand() { assert_eq!(words(1000.0), "Одна тысяча рублей 00 копеек"); }
        #[test]
        fn two_thousand() { assert_eq!(words(2000.0), "Две тысячи рублей 00 копеек"); }
        #[test]
        fn five_thousand() { assert_eq!(words(5000.0), "Пять тысяч рублей 00 копеек"); }
        #[test]
        fn twenty_one_thousand() { assert_eq!(words(21000.0), "Двадцать одна тысяча рублей 00 копеек"); }
        #[test]
        fn spec_reference_47100() {
            assert_eq!(words(47100.0), "Сорок семь тысяч сто рублей 00 копеек");
        }
        #[test]
        fn one_million() { assert_eq!(words(1_000_000.0), "Один миллион рублей 00 копеек"); }
        #[test]
        fn two_million() { assert_eq!(words(2_000_000.0), "Два миллиона рублей 00 копеек"); }
        #[test]
        fn test_case_12400() {
            assert_eq!(words(12400.0), "Двенадцать тысяч четыреста рублей 00 копеек");
        }
        #[test]
        fn kopecks_variants() {
            assert_eq!(words(1.01), "Один рубль 01 копейка");
            assert_eq!(words(1.02), "Один рубль 02 копейки");
            assert_eq!(words(1.05), "Один рубль 05 копеек");
            assert_eq!(words(1.11), "Один рубль 11 копеек");
            assert_eq!(words(1.21), "Один рубль 21 копейка");
        }
        #[test]
        fn capitalization_first_letter() {
            let s = words(12400.0);
            assert!(s.starts_with('Д'));
        }
        #[test]
        fn complex_with_kopecks() {
            assert_eq!(words(12345.67), "Двенадцать тысяч триста сорок пять рублей 67 копеек");
        }
        #[test]
        fn declension_edge_12_13_14() {
            assert!(words(12.0).contains("рублей"));
            assert!(words(13.0).contains("рублей"));
            assert!(words(14.0).contains("рублей"));
            assert!(words(112.0).contains("рублей"));
            assert!(words(1012.0).contains("рублей"));
        }
        #[test]
        fn declension_edge_thousands_12_14() {
            assert!(words(12000.0).contains("тысяч"));
            assert!(words(13000.0).contains("тысяч"));
            assert!(words(14000.0).contains("тысяч"));
            assert!(words(21000.0).contains("тысяча"));
            assert!(words(22000.0).contains("тысячи"));
            assert!(words(25000.0).contains("тысяч"));
        }
    }
}

// ============================================================================
// МОДУЛЬ model — структуры данных
// ============================================================================

/// Модель данных заявки: структуры, расчёт итогов.
pub mod model {
    use crate::fmt;
    use serde::{Deserialize, Serialize};
    use std::hash::Hash;

    /// Единицы измерения.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub enum Unit {
        /// «шт».
        Sht,
        /// «к-кт».
        Kkt,
    }

    impl Unit {
        pub fn as_str(self) -> &'static str {
            match self {
                Unit::Sht => "шт",
                Unit::Kkt => "к-кт",
            }
        }
        pub fn all() -> &'static [Unit] {
            &[Unit::Sht, Unit::Kkt]
        }
    }

    /// Одна строка таблицы товаров.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct LineItem {
        pub name: String,
        pub qty: String,
        pub unit: Unit,
        pub price: String,
    }

    impl LineItem {
        pub fn empty() -> Self {
            Self {
                name: String::new(),
                qty: String::new(),
                unit: Unit::Sht,
                price: String::new(),
            }
        }
        pub fn qty_value(&self) -> Option<f64> { fmt::parse_number(&self.qty) }
        pub fn price_value(&self) -> Option<f64> { fmt::parse_number(&self.price) }
        pub fn sum_value(&self) -> Option<f64> {
            Some(self.qty_value()? * self.price_value()?)
        }
        pub fn is_filled(&self) -> bool {
            let name_ok = !self.name.trim().is_empty();
            let qty_ok = self.qty_value().map(|v| v != 0.0).unwrap_or(false);
            let price_ok = self.price_value().is_some();
            name_ok && qty_ok && price_ok
        }
    }

    /// Документ заявки.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Document {
        pub number: String,
        pub date: String,
        pub lines: Vec<LineItem>,
    }

    impl Document {
        pub fn filled_lines(&self) -> Vec<&LineItem> {
            self.lines.iter().filter(|l| l.is_filled()).collect()
        }
        pub fn filled_count(&self) -> usize {
            self.lines.iter().filter(|l| l.is_filled()).count()
        }
        pub fn total(&self) -> f64 {
            self.lines
                .iter()
                .filter_map(|l| if l.is_filled() { l.sum_value() } else { None })
                .sum()
        }
    }

    pub const SUPPLIER_NAME: &str = "ООО «Автоконтракты»";
    pub const NUMBER_PREFIX: &str = "DZHOД";

    /// Генерирует номер: `DZHOД` + 6 случайных цифр.
    pub fn generate_number() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let n: u32 = rng.gen_range(0..1_000_000);
        format!("{}{:06}", NUMBER_PREFIX, n)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn line(name: &str, qty: &str, unit: Unit, price: &str) -> LineItem {
            LineItem {
                name: name.to_string(),
                qty: qty.to_string(),
                unit,
                price: price.to_string(),
            }
        }

        #[test]
        fn line_sum_basic() {
            let l = line("Товар", "2", Unit::Sht, "100");
            assert_eq!(l.sum_value(), Some(200.0));
        }
        #[test]
        fn line_sum_with_comma() {
            let l = line("Товар", "1,5", Unit::Sht, "100");
            assert_eq!(l.sum_value(), Some(150.0));
        }
        #[test]
        fn line_filled_detection() {
            assert!(line("A", "1", Unit::Sht, "10").is_filled());
            assert!(!line("", "1", Unit::Sht, "10").is_filled());
            assert!(!line("A", "", Unit::Sht, "10").is_filled());
            assert!(!line("A", "1", Unit::Sht, "").is_filled());
            assert!(!line("A", "0", Unit::Sht, "10").is_filled());
        }
        #[test]
        fn total_skips_empty_lines() {
            let doc = Document {
                number: "DZHOД000001".to_string(),
                date: "1 января 2026 г.".to_string(),
                lines: vec![
                    line("A", "1", Unit::Sht, "100"),
                    LineItem::empty(),
                    line("B", "2", Unit::Kkt, "50"),
                    line("", "5", Unit::Sht, "10"),
                ],
            };
            assert_eq!(doc.total(), 200.0);
            assert_eq!(doc.filled_count(), 2);
        }
        #[test]
        fn test_case_from_spec() {
            let doc = Document {
                number: "DZHOД000001".to_string(),
                date: "1 января 2026 г.".to_string(),
                lines: vec![
                    line("Поршнекомплект с кольцами G4FA +0.5", "1", Unit::Kkt, "9100"),
                    line("Антифриз, Korea-Standard зелёный -37°C, 5кг", "1", Unit::Sht, "1780"),
                    line("HYUNDAI/KIA/MOBIS Клапан выпускной", "8", Unit::Sht, "190"),
                ],
            };
            assert_eq!(doc.filled_count(), 3);
            assert_eq!(doc.total(), 12400.0);
        }
        #[test]
        fn number_format() {
            let n = generate_number();
            assert!(n.starts_with("DZHOД"));
            assert_eq!(n.chars().count(), 11);
            let digits: String = n.chars().skip_while(|c| !c.is_ascii_digit()).collect();
            assert_eq!(digits.len(), 6);
            assert!(digits.chars().all(|c| c.is_ascii_digit()));
        }
        #[test]
        fn unit_strings() {
            assert_eq!(Unit::Sht.as_str(), "шт");
            assert_eq!(Unit::Kkt.as_str(), "к-кт");
        }
    }
}

// ============================================================================
// МОДУЛЬ history — база данных распечатанных заявок в RON-формате
// ============================================================================

/// Модуль истории распечаток.
///
/// Каждая распечатанная заявка сохраняется в отдельный `.ron`-файл в
/// специальной папке (по умолчанию — `История/` рядом с exe). Файлы
/// имеют читаемый RON-формат (Rusty Object Notation), который можно
/// открыть в любом текстовом редакторе.
///
/// Внутри программы доступен просмотр истории: список всех распечатанных
/// заявок с номером, датой, итогом и количеством строк.
pub mod history {
    use crate::model::Document;
    use chrono::Local;
    use serde::{Deserialize, Serialize};
    use std::path::{Path, PathBuf};

    /// Одна запись в истории — упрощённая копия распечатанной заявки.
    ///
    /// Хранится в RON-файле в папке истории. Содержит всё, что нужно
    /// для просмотра: номер, дату, итог, строки товаров, путь к PDF.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct HistoryEntry {
        /// Номер заявки (например, «DZHOД123456»).
        pub number: String,
        /// Дата создания (например, «6 октября 2026 г.»).
        pub date: String,
        /// ISO-временная метка создания (для сортировки).
        pub timestamp: String,
        /// Количество наименований.
        pub items_count: usize,
        /// Итоговая сумма в рублях.
        pub total: f64,
        /// Сумма прописью (например, «Двенадцать тысяч четыреста рублей 00 копеек»).
        pub total_words: String,
        /// Строки товаров (упрощённая структура — без строковых полей).
        pub items: Vec<HistoryItem>,
        /// Путь к PDF-файлу (если сохранён).
        pub pdf_path: Option<String>,
    }

    /// Одна строка товара в записи истории.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct HistoryItem {
        pub index: u32,
        pub name: String,
        pub qty: f64,
        pub unit: String,
        pub price: f64,
        pub sum: f64,
    }

    impl HistoryEntry {
        /// Создаёт запись истории из документа.
        pub fn from_document(doc: &Document, pdf_path: Option<&Path>) -> Self {
            let total = doc.total();
            let total_words = crate::money_words::amount_to_words(total);
            let items: Vec<HistoryItem> = doc
                .filled_lines()
                .into_iter()
                .enumerate()
                .map(|(i, line)| HistoryItem {
                    index: (i + 1) as u32,
                    name: line.name.clone(),
                    qty: line.qty_value().unwrap_or(0.0),
                    unit: line.unit.as_str().to_string(),
                    price: line.price_value().unwrap_or(0.0),
                    sum: line.sum_value().unwrap_or(0.0),
                })
                .collect();

            Self {
                number: doc.number.clone(),
                date: doc.date.clone(),
                timestamp: Local::now().to_rfc3339(),
                items_count: doc.filled_count(),
                total,
                total_words,
                items,
                pdf_path: pdf_path.map(|p| p.display().to_string()),
            }
        }

        /// Возвращает имя RON-файла для этой записи.
        /// Формат: «Заявка DZHOД123456 2026-10-06 15-30-45.ron»
        /// (с временной меткой, чтобы избежать коллизий имён).
        pub fn ron_filename(&self) -> String {
            // Извлекаем дату-время из timestamp для имени файла.
            let safe_ts = self
                .timestamp
                .chars()
                .map(|c| match c {
                    ':' | ' ' | 'T' => '-',
                    '+' => 'p',
                    '.' => '-',
                    _ => c,
                })
                .collect::<String>();
            format!("Заявка {} {}.ron", self.number, safe_ts)
        }
    }

    /// Менеджер истории: загрузка, сохранение, список.
    pub struct HistoryStore {
        /// Папка, где хранятся RON-файлы истории.
        pub dir: PathBuf,
    }

    impl HistoryStore {
        /// Создаёт менеджер истории и гарантирует, что папка существует.
        pub fn new(dir: PathBuf) -> Self {
            let _ = std::fs::create_dir_all(&dir);
            Self { dir }
        }

        /// Сохраняет запись истории в RON-файл.
        /// Возвращает путь к созданному файлу.
        pub fn save(&self, entry: &HistoryEntry) -> Result<PathBuf, String> {
            let _ = std::fs::create_dir_all(&self.dir);
            let filename = entry.ron_filename();
            let path = self.dir.join(&filename);
            // Красивое форматирование RON с переносами строк.
            let pretty = ron::ser::PrettyConfig::default()
                .depth_limit(4)
                .separate_tuple_members(true)
                .enumerate_arrays(true);
            let ron_str = ron::ser::to_string_pretty(entry, pretty)
                .map_err(|e| format!("Ошибка сериализации RON: {}", e))?;
            std::fs::write(&path, ron_str)
                .map_err(|e| format!("Не удалось записать {}: {}", path.display(), e))?;
            Ok(path)
        }

        /// Загружает все записи истории, отсортированные по убыванию даты
        /// (самые свежие — первыми).
        pub fn list(&self) -> Vec<HistoryEntry> {
            let mut entries = Vec::new();
            if let Ok(rd) = std::fs::read_dir(&self.dir) {
                for entry in rd.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("ron") {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(hist) = ron::from_str::<HistoryEntry>(&content) {
                                entries.push(hist);
                            }
                        }
                    }
                }
            }
            // Сортировка: свежие первыми (по timestamp, по убыванию).
            entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            entries
        }

        /// Удаляет запись истории по её номеру и timestamp.
        pub fn delete(&self, number: &str, timestamp: &str) -> Result<(), String> {
            if let Ok(rd) = std::fs::read_dir(&self.dir) {
                for entry in rd.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                        continue;
                    }
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(hist) = ron::from_str::<HistoryEntry>(&content) {
                            if hist.number == number && hist.timestamp == timestamp {
                                std::fs::remove_file(&path)
                                    .map_err(|e| format!("Не удалось удалить: {}", e))?;
                                return Ok(());
                            }
                        }
                    }
                }
            }
            Err("Запись не найдена".to_string())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::model::{Document, LineItem, Unit};

        fn make_doc() -> Document {
            Document {
                number: "DZHOД123456".to_string(),
                date: "6 октября 2026 г.".to_string(),
                lines: vec![
                    LineItem {
                        name: "Товар A".to_string(),
                        qty: "2".to_string(),
                        unit: Unit::Sht,
                        price: "100".to_string(),
                    },
                    LineItem {
                        name: "Товар B".to_string(),
                        qty: "1".to_string(),
                        unit: Unit::Kkt,
                        price: "500".to_string(),
                    },
                ],
            }
        }

        #[test]
        fn history_entry_from_document() {
            let doc = make_doc();
            let entry = HistoryEntry::from_document(&doc, None);
            assert_eq!(entry.number, "DZHOД123456");
            assert_eq!(entry.date, "6 октября 2026 г.");
            assert_eq!(entry.items_count, 2);
            assert_eq!(entry.total, 700.0);
            assert_eq!(entry.items.len(), 2);
            assert_eq!(entry.items[0].name, "Товар A");
            assert_eq!(entry.items[0].sum, 200.0);
            assert!(entry.total_words.contains("рублей"));
        }

        #[test]
        fn ron_filename_is_unique() {
            let doc = make_doc();
            let mut entry = HistoryEntry::from_document(&doc, None);
            let name1 = entry.ron_filename();
            // Меняем timestamp — имя должно измениться.
            entry.timestamp = "2026-10-06T15:30:45+00:00".to_string();
            let name2 = entry.ron_filename();
            assert_ne!(name1, name2);
            assert!(name1.starts_with("Заявка DZHOД123456"));
            assert!(name1.ends_with(".ron"));
        }

        #[test]
        fn save_and_list_history() {
            let tmp = std::env::temp_dir().join("zayavka_history_test");
            let _ = std::fs::remove_dir_all(&tmp);
            let store = HistoryStore::new(tmp.clone());

            let doc = make_doc();
            let entry = HistoryEntry::from_document(&doc, None);
            let path = store.save(&entry).expect("save failed");
            assert!(path.exists());

            let list = store.list();
            assert_eq!(list.len(), 1);
            assert_eq!(list[0].number, "DZHOД123456");
            assert_eq!(list[0].total, 700.0);

            // Проверим, что RON-файл читаемый.
            let content = std::fs::read_to_string(&path).unwrap();
            assert!(content.contains("DZHOД123456"));
            assert!(content.contains("Товар A"));

            let _ = std::fs::remove_dir_all(&tmp);
        }

        #[test]
        fn delete_history_entry() {
            let tmp = std::env::temp_dir().join("zayavka_history_test_del");
            let _ = std::fs::remove_dir_all(&tmp);
            let store = HistoryStore::new(tmp.clone());

            let doc = make_doc();
            let entry = HistoryEntry::from_document(&doc, None);
            let _ = store.save(&entry).expect("save failed");
            assert_eq!(store.list().len(), 1);

            store
                .delete(&entry.number, &entry.timestamp)
                .expect("delete failed");
            assert_eq!(store.list().len(), 0);

            let _ = std::fs::remove_dir_all(&tmp);
        }

        #[test]
        fn list_sorted_by_date_desc() {
            let tmp = std::env::temp_dir().join("zayavka_history_test_sort");
            let _ = std::fs::remove_dir_all(&tmp);
            let store = HistoryStore::new(tmp.clone());

            // Три записи с разными timestamp.
            let mut e1 = HistoryEntry::from_document(&make_doc(), None);
            e1.timestamp = "2026-10-01T10:00:00+00:00".to_string();
            e1.number = "DZHOД000001".to_string();
            let _ = store.save(&e1);

            let mut e2 = HistoryEntry::from_document(&make_doc(), None);
            e2.timestamp = "2026-10-06T15:00:00+00:00".to_string();
            e2.number = "DZHOД000002".to_string();
            let _ = store.save(&e2);

            let mut e3 = HistoryEntry::from_document(&make_doc(), None);
            e3.timestamp = "2026-10-03T12:00:00+00:00".to_string();
            e3.number = "DZHOД000003".to_string();
            let _ = store.save(&e3);

            let list = store.list();
            assert_eq!(list.len(), 3);
            // Свежие первыми: e2 (6 окт) → e3 (3 окт) → e1 (1 окт).
            assert_eq!(list[0].number, "DZHOД000002");
            assert_eq!(list[1].number, "DZHOД000003");
            assert_eq!(list[2].number, "DZHOД000001");

            let _ = std::fs::remove_dir_all(&tmp);
        }
    }
}

// ============================================================================
// МОДУЛЬ csv_import — импорт заявок из CSV (формат поставщика)
// ============================================================================

/// Модуль импорта CSV-данных из выгрузки сайта поставщика.
///
/// Формат CSV (разделитель `;`):
/// ```text
/// GUID;Номер запчасти;Наименование;Бренд;Цена, руб;Заказано;Сумма, руб;Комментарий;Статус
/// NSIN0003495651;MN110724;Сальник полуоси;Mitsubishi;324.91;1;324.91;;
/// NSIN0005391098;MB664611;Подшипник полуоси| зад |;Mitsubishi;3 847.11;1;3 847.11;;
/// ...
/// Итого;;;;;6;12 191.76;;
/// ```
///
/// При импорте:
/// - Наименование товара формируется как «Наименование [Бренд] (Номер)» —
///   так пользователь видит полную информацию.
/// - Количество берётся из колонки «Заказано».
/// - Цена — из «Цена, руб» (пробелы-разделители тысяч удаляются).
/// - Служебная строка «Итого» пропускается.
/// - Пустые строки (с нулевым количеством или без наименования) пропускаются.
pub mod csv_import {
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
}

// ============================================================================
// МОДУЛЬ pdf — генерация PDF через genpdf
// ============================================================================

/// Модуль генерации PDF-документа.
///
/// Использует крейт `genpdf` со встроенным шрифтом DejaVu Serif
/// (3447 глифов — самое широкое покрытие Unicode среди свободных шрифтов
/// с засечками) через `include_bytes!`. Файлы шрифта автоматически
/// скачиваются при первой сборке через `build.rs`.
pub mod pdf {
    use crate::fmt;
    use crate::model::{self, Document};
    use crate::money_words;
    use genpdf::style::Style;
    use genpdf::Element;
    use genpdf::{elements as el, Alignment, Document as GenDocument, Margins};
    use std::path::Path;

    // Шрифты вшиты в бинарник на этапе компиляции.
    // DejaVu Serif — самое широкое покрытие Unicode среди свободных шрифтов
    // с засечками: 3447 глифов. Поддерживает латиницу, кириллицу (включая
    // расширенную), греческий, математику, технические символы, и т.д.
    //
    // Пути к шрифтам передаются через cargo:rustc-env из build.rs,
    // который скачивает их в OUT_DIR (обходит sandbox Cargo).
    const FONT_REGULAR: &[u8] = include_bytes!(env!("FONT_REGULAR_PATH"));
    const FONT_BOLD: &[u8] = include_bytes!(env!("FONT_BOLD_PATH"));

    const FONT_SIZE_BODY: u8 = 10;
    const FONT_SIZE_TITLE: u8 = 13;
    const FONT_SIZE_SMALL: u8 = 7;

    // ============================================================================
    // Фильтрация неподдерживаемых символов
    // ============================================================================

    /// Thread-local кэш распарсенного шрифта. Face парсится один раз на поток,
    /// потом используется для проверки отдельных символов через `glyph_index`.
    /// `glyph_index` очень быстрый (O(log n) в cmap), поэтому фильтрация
    /// строк не создаёт накладных расходов.
    thread_local! {
        static FONT_FACE: std::cell::RefCell<Option<ttf_parser::Face<'static>>> =
            std::cell::RefCell::new(None);
    }

    /// Проверяет, поддерживает ли шрифт данный символ.
    fn is_char_supported(c: char) -> bool {
        // Управляющие символы всегда пропускаем.
        if c == '\n' || c == '\r' || c == '\t' {
            return true;
        }
        FONT_FACE.with(|cell| {
            let mut borrowed = cell.borrow_mut();
            if borrowed.is_none() {
                *borrowed = ttf_parser::Face::parse(FONT_REGULAR, 0).ok();
            }
            if let Some(face) = borrowed.as_ref() {
                face.glyph_index(c).is_some()
            } else {
                false
            }
        })
    }

    /// Фильтрует строку, удаляя символы, которых нет в шрифте.
    /// Заменяет их пустой строкой (не вопросительным знаком — пользователь
    /// явно просил исключить «пустые квадраты и знаки вопроса»).
    pub fn filter_unsupported(text: &str) -> String {
        text.chars()
            .filter(|c| is_char_supported(*c))
            .collect()
    }

    /// Применяет `filter_unsupported` к документу — возвращает новую копию
    /// документа, в которой все строки очищены от неподдерживаемых символов.
    fn filter_document(doc: &Document) -> Document {
        use crate::model::LineItem;
        let filtered_lines = doc
            .lines
            .iter()
            .map(|line| LineItem {
                name: filter_unsupported(&line.name),
                qty: filter_unsupported(&line.qty),
                unit: line.unit,
                price: filter_unsupported(&line.price),
            })
            .collect();
        Document {
            number: filter_unsupported(&doc.number),
            date: filter_unsupported(&doc.date),
            lines: filtered_lines,
        }
    }

    /// Генерирует PDF и сохраняет его по указанному пути.
    ///
    /// Перед рендерингом **все строки документа фильтруются** — символы,
    /// которых нет в шрифте DejaVu Serif, удаляются. Это исключает появление
    /// пустых квадратов (tofu) и знаков вопроса в PDF.
    pub fn generate_pdf(doc: &Document, path: &Path) -> Result<(), String> {
        // Фильтруем неподдерживаемые символы.
        let doc = filter_document(doc);

        let font_family = load_font_family()
            .map_err(|e| format!("Не удалось загрузить шрифт: {}", e))?;

        let mut pdf = GenDocument::new(font_family);
        pdf.set_title(format!("Заявка на отгрузку № {}", doc.number));
        pdf.set_font_size(FONT_SIZE_BODY);
        // Воздушный межстрочный интервал — текст выглядит легче.
        pdf.set_line_spacing(1.35);
        pdf.set_paper_size(genpdf::PaperSize::A4);

        let mut decorator = genpdf::SimplePageDecorator::new();
        decorator.set_margins(Margins::trbl(
            13.0_f32, 15.0_f32, 12.0_f32, 15.0_f32,
        ));
        pdf.set_page_decorator(decorator);

        pdf.push(render_title(&doc.number, &doc.date));
        pdf.push(render_supplier_block());
        pdf.push(render_table(&doc));
        pdf.push(render_total_row(&doc));
        pdf.push(render_total_count(&doc));
        pdf.push(render_amount_in_words(&doc));
        pdf.push(render_payment_block(&doc));
        pdf.push(render_signatures_block());

        pdf.render_to_file(path).map_err(|e| format!("Ошибка записи PDF: {}", e))?;
        Ok(())
    }

    fn load_font_family() -> Result<genpdf::fonts::FontFamily<genpdf::fonts::FontData>, genpdf::error::Error> {
        let regular = genpdf::fonts::FontData::new(FONT_REGULAR.to_vec(), None)?;
        let bold = genpdf::fonts::FontData::new(FONT_BOLD.to_vec(), None)?;
        let regular_italic = regular.clone();
        let bold_italic = bold.clone();
        Ok(genpdf::fonts::FontFamily {
            regular, bold,
            italic: regular_italic,
            bold_italic,
        })
    }

    fn render_title(number: &str, date: &str) -> el::LinearLayout {
        let mut layout = el::LinearLayout::vertical();
        let title = format!("Заявка на отгрузку № {} от {}", number, date);
        // Заголовок — обычный шрифт увеличенного размера, без bold.
        // Так выглядит изящнее и «дороже», чем жирный.
        layout.push(
            el::Paragraph::new(title)
                .aligned(Alignment::Center)
                .styled(Style::new().with_font_size(FONT_SIZE_TITLE)),
        );
        layout.push(el::Break::new(0.3));
        layout.push(thin_line());
        layout.push(el::Break::new(0.4));
        layout
    }

    fn render_supplier_block() -> el::LinearLayout {
        let mut layout = el::LinearLayout::vertical();
        // Поставщик — без ИНН (по новой спецификации). ИНН полностью убран.
        // «Поставщик:» — обычный шрифт, значение — жирным для акцента.
        let supplier_para = el::Paragraph::default()
            .string("Поставщик:  ")
            .styled_string(model::SUPPLIER_NAME, Style::new().bold());
        layout.push(supplier_para);
        layout.push(el::Break::new(0.3));
        layout
    }

    fn render_table(doc: &Document) -> el::LinearLayout {
        let mut layout = el::LinearLayout::vertical();
        let frame = el::FrameCellDecorator::new(true, true, true);
        let mut table = el::TableLayout::new(vec![1, 5, 1, 1, 1, 1]);
        table.set_cell_decorator(frame);

        let header_style = Style::new().bold();
        let header: Vec<Box<dyn Element>> = vec![
            Box::new(styled_paragraph("#", header_style, Alignment::Center)),
            Box::new(styled_paragraph("Товар", header_style, Alignment::Left)),
            Box::new(styled_paragraph("кол-во", header_style, Alignment::Center)),
            Box::new(styled_paragraph("ед.", header_style, Alignment::Center)),
            Box::new(styled_paragraph("цена", header_style, Alignment::Center)),
            Box::new(styled_paragraph("сумма", header_style, Alignment::Center)),
        ];
        let _ = table.push_row(header);

        let mut idx = 1u32;
        for line in doc.filled_lines() {
            let sum = line.sum_value().unwrap_or(0.0);
            let qty_str = format_qty(line.qty_value());
            let price_str = format_price(line.price_value());
            let sum_str = fmt::format_money(sum);

            // Все числовые колонки — по центру (по запросу пользователя).
            let row: Vec<Box<dyn Element>> = vec![
                Box::new(styled_paragraph(&idx.to_string(), Style::new(), Alignment::Center)),
                Box::new(styled_paragraph(&line.name, Style::new(), Alignment::Left)),
                Box::new(styled_paragraph(&qty_str, Style::new(), Alignment::Center)),
                Box::new(styled_paragraph(line.unit.as_str(), Style::new(), Alignment::Center)),
                Box::new(styled_paragraph(&price_str, Style::new(), Alignment::Center)),
                Box::new(styled_paragraph(&sum_str, Style::new(), Alignment::Center)),
            ];
            let _ = table.push_row(row);
            idx += 1;
        }

        // Итоговая строка УБРАНА из таблицы — теперь «Итого:» отдельной
        // строкой под таблицей (по запросу пользователя).
        layout.push(table);
        layout.push(el::Break::new(0.2));
        layout
    }

    /// Отдельная строка «Итого: <сумма>» под таблицей (за её пределами).
    fn render_total_row(doc: &Document) -> el::LinearLayout {
        let mut layout = el::LinearLayout::vertical();
        let total = doc.total();
        let total_str = fmt::format_money(total);
        let bold = Style::new().bold();
        // Таблица 1×2: «Итого:» слева, сумма справа — выравнивается по правому краю.
        let frame = el::FrameCellDecorator::new(false, false, false);
        let mut table = el::TableLayout::new(vec![5, 2]);
        table.set_cell_decorator(frame);
        let row: Vec<Box<dyn Element>> = vec![
            Box::new(styled_paragraph("Итого:", bold.clone(), Alignment::Right)),
            Box::new(styled_paragraph(&total_str, bold, Alignment::Center)),
        ];
        let _ = table.push_row(row);
        layout.push(table);
        layout.push(el::Break::new(0.3));
        layout
    }

    fn render_total_count(doc: &Document) -> el::LinearLayout {
        let mut layout = el::LinearLayout::vertical();
        let n = doc.filled_count();
        let x = fmt::format_rubles_rounded(doc.total());
        let text = format!("Всего наименований {}, на сумму {} руб.", n, x);
        layout.push(el::Paragraph::new(text));
        layout.push(el::Break::new(0.3));
        layout
    }

    fn render_amount_in_words(doc: &Document) -> el::LinearLayout {
        let mut layout = el::LinearLayout::vertical();
        let words = money_words::amount_to_words(doc.total());
        layout.push(el::Paragraph::new(words).styled(Style::new().bold()));
        layout.push(el::Break::new(0.4));
        layout
    }

    fn render_payment_block(doc: &Document) -> el::LinearLayout {
        let mut layout = el::LinearLayout::vertical();
        layout.push(thin_line());
        layout.push(el::Break::new(0.3));
        // «Оплата» — обычный шрифт, не bold. Раньше жирный перегружал вид.
        layout.push(el::Paragraph::new("Оплата"));
        let cash_str = fmt::format_money(doc.total());
        layout.push(el::Paragraph::new(format!("    Наличные {}", cash_str)));
        layout.push(el::Paragraph::new("    Сдача  0,00"));
        layout.push(el::Break::new(0.2));
        layout.push(thin_line());
        layout
    }

    fn render_signatures_block() -> el::LinearLayout {
        // Две графы для подписей: «Отпустил» и «Получил».
        // Вместо прежнего блока «Кассир» — по новой спецификации.
        let mut layout = el::LinearLayout::vertical();
        layout.push(el::Break::new(0.6));
        // Две колонки 50/50 с линиями для подписи.
        let frame = el::FrameCellDecorator::new(false, false, false);
        let mut table = el::TableLayout::new(vec![1, 1]);
        table.set_cell_decorator(frame);
        let line_style = Style::new();
        let row: Vec<Box<dyn Element>> = vec![
            Box::new(
                el::Paragraph::new("Отпустил: ______________________")
                    .styled(line_style.clone()),
            ),
            Box::new(
                el::Paragraph::new("Получил: ______________________")
                    .aligned(Alignment::Right)
                    .styled(line_style),
            ),
        ];
        let _ = table.push_row(row);
        layout.push(table);
        // Подписи «(подпись)» под линиями.
        let mut sub_table = el::TableLayout::new(vec![1, 1]);
        sub_table.set_cell_decorator(el::FrameCellDecorator::new(false, false, false));
        let small_style = Style::new().with_font_size(FONT_SIZE_SMALL);
        let sub_row: Vec<Box<dyn Element>> = vec![
            Box::new(el::Paragraph::new("              (подпись)").styled(small_style.clone())),
            Box::new(
                el::Paragraph::new("              (подпись)")
                    .aligned(Alignment::Right)
                    .styled(small_style),
            ),
        ];
        let _ = sub_table.push_row(sub_row);
        layout.push(sub_table);
        layout
    }

    /// Тонкая горизонтальная линия на всю ширину.
    ///
    /// Реализация через таблицу 1×1 с FrameCellDecorator — рамка рисует
    /// верхнюю и нижнюю линии. Без bold-стиля линия получается тонкой
    /// и элегантной, не перегружает документ.
    fn thin_line() -> impl Element {
        let frame = el::FrameCellDecorator::new(true, true, true);
        let mut table = el::TableLayout::new(vec![1]);
        table.set_cell_decorator(frame);
        let cell: Vec<Box<dyn Element>> = vec![Box::new(el::Break::new(0.02))];
        let _ = table.push_row(cell);
        el::StyledElement::new(table, Style::new())
    }

    fn styled_paragraph(text: &str, style: Style, align: Alignment) -> el::StyledElement<el::Paragraph> {
        el::Paragraph::new(text).aligned(align).styled(style)
    }

    fn format_qty(value: Option<f64>) -> String {
        match value {
            Some(v) => {
                if v.fract().abs() < 1e-9 {
                    format!("{}", v as i64)
                } else {
                    format!("{:.3}", v).replace('.', ",")
                }
            }
            None => String::new(),
        }
    }

    fn format_price(value: Option<f64>) -> String {
        match value {
            Some(v) => fmt::format_money(v),
            None => String::new(),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn filter_keeps_basic_cyrillic() {
            let s = filter_unsupported("Привет, мир!");
            assert_eq!(s, "Привет, мир!");
        }

        #[test]
        fn filter_keeps_pipe_and_slash() {
            // Символы |, /, -, _, кавычки — должны поддерживаться DejaVu Serif.
            let s = filter_unsupported("Подшипник | зад прав/лев | LADA");
            assert_eq!(s, "Подшипник | зад прав/лев | LADA");
        }

        #[test]
        fn filter_removes_emoji() {
            // Эмодзи нет в DejaVu Serif — должны быть удалены.
            let s = filter_unsupported("Товар 🚀!");
            assert_eq!(s, "Товар !");
        }

        #[test]
        fn filter_removes_chinese_chars() {
            // Китайские иероглифы не поддерживаются DejaVu Serif.
            let s = filter_unsupported("Товар 你好!");
            assert_eq!(s, "Товар !");
        }

        #[test]
        fn filter_keeps_special_symbols() {
            // Спецсимволы: №, ±, °, ×, ÷, «» — DejaVu Serif их поддерживает.
            let s = filter_unsupported("№ 123 ±0.5 °C × ÷ «кавычки»");
            // Проверим, что ничего не удалено.
            assert_eq!(s, "№ 123 ±0.5 °C × ÷ «кавычки»");
        }

        #[test]
        fn filter_preserves_newlines() {
            let s = filter_unsupported("Строка 1\nСтрока 2\tTab");
            assert_eq!(s, "Строка 1\nСтрока 2\tTab");
        }

        #[test]
        fn filter_empty_string() {
            assert_eq!(filter_unsupported(""), "");
        }
    }
}

// ============================================================================
// МОДУЛЬ gui — графический интерфейс на egui
// ============================================================================

/// Модуль графического интерфейса на egui.
pub mod gui {
    use crate::fmt;
    use crate::history::{HistoryEntry, HistoryStore};
    use crate::model::{self, Document, LineItem, Unit};
    use crate::pdf;
    use crate::APP_NAME;
    use eframe::egui;
    use std::path::PathBuf;

    /// Размер окна по умолчанию.
    pub const WINDOW_WIDTH: f32 = 950.0;
    pub const WINDOW_HEIGHT: f32 = 720.0;

    /// Состояние приложения.
    pub struct App {
        pub doc: Document,
        pub draft_path: PathBuf,
        pub pdf_dir: PathBuf,
        pub history_dir: PathBuf,
        pub status_message: String,
        pub last_filled_signature: u64,
        pub show_new_doc_dialog: bool,
        /// Окно истории распечаток.
        pub show_history_window: bool,
        /// Кэш списка истории (обновляется при открытии окна).
        pub history_entries: Vec<HistoryEntry>,
        /// Индекс выбранной записи в истории (для просмотра деталей).
        pub history_selected: Option<usize>,
    }

    impl App {
        pub fn new(draft_path: PathBuf, pdf_dir: PathBuf, history_dir: PathBuf) -> Self {
            let mut doc = load_draft(&draft_path).unwrap_or_else(|| Document {
                number: model::generate_number(),
                date: fmt::today_string(),
                lines: vec![LineItem::empty()],
            });
            if doc.lines.is_empty() {
                doc.lines.push(LineItem::empty());
            }
            doc.number = model::generate_number();
            doc.date = fmt::today_string();

            let last_filled_signature = filled_signature(&doc);
            Self {
                doc,
                draft_path,
                pdf_dir,
                history_dir,
                status_message: String::new(),
                last_filled_signature,
                show_new_doc_dialog: false,
                show_history_window: false,
                history_entries: Vec::new(),
                history_selected: None,
            }
        }

        pub fn save_draft(&self) {
            if let Some(parent) = self.draft_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let json = serde_json::to_string_pretty(&self.doc).unwrap_or_default();
            if let Err(e) = std::fs::write(&self.draft_path, json) {
                eprintln!("Не удалось сохранить черновик: {}", e);
            }
        }

        fn new_document(&mut self) {
            self.doc = Document {
                number: model::generate_number(),
                date: fmt::today_string(),
                lines: vec![LineItem::empty()],
            };
            self.last_filled_signature = 0;
            self.status_message = "Создана новая заявка".to_string();
            self.save_draft();
        }

        fn generate_and_open_pdf(&mut self) {
            if self.doc.filled_count() == 0 {
                self.status_message = "Нет заполненных строк для формирования PDF".to_string();
                return;
            }
            let _ = std::fs::create_dir_all(&self.pdf_dir);
            let filename = format!("Заявка {}.pdf", self.doc.number);
            let path = self.pdf_dir.join(&filename);

            match pdf::generate_pdf(&self.doc, &path) {
                Ok(()) => {
                    self.status_message = format!("PDF сохранён: {}", path.display());

                    // Сохраняем запись в историю (RON-формат).
                    let entry = HistoryEntry::from_document(&self.doc, Some(&path));
                    let store = HistoryStore::new(self.history_dir.clone());
                    match store.save(&entry) {
                        Ok(ron_path) => {
                            self.status_message.push_str(&format!(
                                "\nИстория: {}", ron_path.display()
                            ));
                        }
                        Err(e) => {
                            self.status_message.push_str(&format!(
                                "\nНе удалось сохранить историю: {}", e
                            ));
                        }
                    }

                    if let Err(e) = open::that(&path) {
                        self.status_message = format!(
                            "PDF создан, но не удалось открыть просмотрщик: {}", e
                        );
                    }
                }
                Err(e) => {
                    self.status_message = format!("Ошибка генерации PDF: {}", e);
                }
            }
        }

        fn add_empty_line(&mut self) {
            self.doc.lines.push(LineItem::empty());
        }

        fn remove_line(&mut self, idx: usize) {
            if idx < self.doc.lines.len() {
                self.doc.lines.remove(idx);
                if self.doc.lines.is_empty() {
                    self.doc.lines.push(LineItem::empty());
                }
            }
        }

        /// Открывает файловый диалог и импортирует CSV из выбранного файла.
        /// Импортированные строки ДОБАВЛЯЮТСЯ к текущим (не заменяют).
        fn import_csv_from_file(&mut self) {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("CSV файлы", &["csv", "txt"])
                .add_filter("Все файлы", &["*"])
                .set_title("Выберите CSV-файл от поставщика")
                .pick_file()
            {
                match crate::csv_import::parse_csv_file(&path) {
                    Ok(result) => {
                        // Добавляем импортированные строки к существующим.
                        // Сначала удаляем пустые строки в конце.
                        while let Some(last) = self.doc.lines.last() {
                            if !last.is_filled() {
                                self.doc.lines.pop();
                            } else {
                                break;
                            }
                        }
                        self.doc.lines.extend(result.items);
                        // Гарантируем хотя бы одну пустую строку в конце.
                        if self.doc.lines.is_empty() {
                            self.doc.lines.push(LineItem::empty());
                        }
                        self.status_message = result.message;
                    }
                    Err(e) => {
                        self.status_message = format!("Ошибка импорта: {}", e);
                    }
                }
            }
        }

        /// Читает CSV из буфера обмена и импортирует строки.
        fn import_csv_from_clipboard(&mut self, _ctx: &egui::Context) {
            // Используем arboard для чтения буфера обмена — egui 0.29
            // не предоставляет API для чтения, только для записи (copy_text).
            let clipboard_text: Option<String> = arboard::Clipboard::new()
                .ok()
                .and_then(|mut cb| cb.get_text().ok());
            match clipboard_text {
                Some(text) => {
                    let result = crate::csv_import::parse_csv_text(&text);
                    if result.items.is_empty() {
                        self.status_message = format!(
                            "В буфере не найдено строк для импорта. {}",
                            result.message
                        );
                        return;
                    }
                    // Добавляем импортированные строки к существующим.
                    while let Some(last) = self.doc.lines.last() {
                        if !last.is_filled() {
                            self.doc.lines.pop();
                        } else {
                            break;
                        }
                    }
                    self.doc.lines.extend(result.items);
                    if self.doc.lines.is_empty() {
                        self.doc.lines.push(LineItem::empty());
                    }
                    self.status_message = format!(
                        "Импорт из буфера: {}",
                        result.message
                    );
                }
                None => {
                    self.status_message =
                        "Буфер обмена пуст или не содержит текста".to_string();
                }
            }
        }

        /// Копирует текст заявки в буфер обмена — для отправки в Telegram,
        /// мессенджеры или email. Формат: читаемый текст с переносами строк.
        fn copy_to_clipboard(&mut self, ctx: &egui::Context) {
            let total = self.doc.total();
            let total_str = fmt::format_money(total);
            let total_words = crate::money_words::amount_to_words(total);
            let n = self.doc.filled_count();
            let x = fmt::format_rubles_rounded(total);

            let mut text = String::new();
            text.push_str(&format!(
                "Заявка на отгрузку № {} от {}\n",
                self.doc.number, self.doc.date
            ));
            text.push('\n');
            text.push_str(&format!("Поставщик: {}\n", crate::model::SUPPLIER_NAME));
            text.push('\n');

            // Таблица в текстовом виде — просто пронумерованный список.
            let mut idx = 1u32;
            for line in self.doc.filled_lines() {
                let qty = line.qty_value().unwrap_or(0.0);
                let price = line.price_value().unwrap_or(0.0);
                let sum = line.sum_value().unwrap_or(0.0);
                text.push_str(&format!(
                    "{}. {} — {} {} × {} = {} руб.\n",
                    idx,
                    line.name,
                    format_qty_short(qty),
                    line.unit.as_str(),
                    fmt::format_money(price),
                    fmt::format_money(sum),
                ));
                idx += 1;
            }

            text.push('\n');
            text.push_str(&format!("Итого: {} руб.\n", total_str));
            text.push_str(&format!(
                "Всего наименований {}, на сумму {} руб.\n",
                n, x
            ));
            text.push_str(&format!("{}\n", total_words));
            text.push('\n');
            text.push_str(&format!("Оплата / Наличные {}\n", total_str));
            text.push_str("Сдача 0,00\n");

            ctx.copy_text(text);
            self.status_message =
                "Заявка скопирована в буфер обмена — можно вставить в Telegram"
                    .to_string();
        }

        fn ensure_trailing_empty(&mut self) {
            if let Some(last) = self.doc.lines.last() {
                if last.is_filled() {
                    self.doc.lines.push(LineItem::empty());
                }
            } else {
                self.doc.lines.push(LineItem::empty());
            }
        }
    }

    impl eframe::App for App {
        fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
            self.save_draft();
        }

        fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            let title = format!("Заявка на отгрузку № {} — {}", self.doc.number, APP_NAME);
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));

            if self.show_new_doc_dialog {
                let mut open = true;
                egui::Window::new("Новая заявка")
                    .collapsible(false)
                    .resizable(false)
                    .open(&mut open)
                    .show(ctx, |ui| {
                        ui.label("Очистить таблицу и сгенерировать новый номер заявки?");
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui.button("Да, создать новую").clicked() {
                                self.new_document();
                                self.show_new_doc_dialog = false;
                            }
                            if ui.button("Отмена").clicked() {
                                self.show_new_doc_dialog = false;
                            }
                        });
                    });
                if !open {
                    self.show_new_doc_dialog = false;
                }
            }

            // Окно истории распечаток.
            if self.show_history_window {
                let mut open = true;
                egui::Window::new("📋 История распечаток")
                    .collapsible(false)
                    .resizable(true)
                    .default_width(900.0)
                    .default_height(600.0)
                    .open(&mut open)
                    .show(ctx, |ui| {
                        self.render_history(ui);
                    });
                if !open {
                    self.show_history_window = false;
                }
            }

            egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
                ui.add_space(2.0);
                ui.horizontal(|ui| { ui.label(&self.status_message); });
                ui.add_space(2.0);
            });

            egui::TopBottomPanel::bottom("totals_panel")
                .resizable(false)
                .show(ctx, |ui| { self.render_totals(ui); });

            egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button("Сформировать PDF").clicked() {
                        self.generate_and_open_pdf();
                    }
                    if ui.button("Новая заявка").clicked() {
                        self.show_new_doc_dialog = true;
                    }
                    ui.separator();
                    if ui.button("+ Добавить строку").clicked() {
                        self.add_empty_line();
                    }
                    ui.separator();
                    if ui.button("📥 Импорт CSV").clicked() {
                        self.import_csv_from_file();
                    }
                    if ui.button("📋 Вставить из буфера").clicked() {
                        self.import_csv_from_clipboard(ctx);
                    }
                    ui.separator();
                    if ui.button("📑 Копировать текст").clicked() {
                        self.copy_to_clipboard(ctx);
                    }
                    ui.separator();
                    if ui.button("🗂 История").clicked() {
                        // Обновляем кэш истории при открытии окна.
                        let store = HistoryStore::new(self.history_dir.clone());
                        self.history_entries = store.list();
                        self.history_selected = None;
                        self.show_history_window = true;
                    }
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.heading(format!(
                        "Заявка на отгрузку № {} от {}",
                        self.doc.number, self.doc.date
                    ));
                });
                ui.add_space(4.0);
            });

            egui::CentralPanel::default().show(ctx, |ui| {
                self.render_table(ui);
            });

            self.ensure_trailing_empty();

            let cur_sig = filled_signature(&self.doc);
            if cur_sig != self.last_filled_signature {
                self.last_filled_signature = cur_sig;
                self.save_draft();
            }
        }
    }

    impl App {
        fn render_table(&mut self, ui: &mut egui::Ui) {
            let avail = ui.available_width();
            let col_widths = [
                avail * 0.04, avail * 0.42, avail * 0.10, avail * 0.09,
                avail * 0.12, avail * 0.14, avail * 0.05,
            ];

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal(|ui| {
                    let headers = ["№", "Товар", "кол-во", "ед.", "цена", "сумма", ""];
                    let mut widths = col_widths.iter().copied();
                    for h in headers.iter() {
                        let w = widths.next().unwrap_or(60.0);
                        let rich = egui::RichText::new(*h).strong();
                        ui.add_sized([w, 18.0], egui::Label::new(rich));
                    }
                });
                ui.separator();

                let mut remove_idx: Option<usize> = None;
                let line_count = self.doc.lines.len();
                for i in 0..line_count {
                    let line = &mut self.doc.lines[i];
                    let mut widths = col_widths.iter().copied();
                    ui.horizontal(|ui| {
                        let w = widths.next().unwrap_or(40.0);
                        ui.add_sized([w, 20.0], egui::Label::new(format!("{}", i + 1)));

                        let w = widths.next().unwrap_or(200.0);
                        ui.add_sized(
                            [w, 20.0],
                            egui::TextEdit::singleline(&mut line.name)
                                .desired_width(w)
                                .hint_text("Наименование товара"),
                        );

                        let w = widths.next().unwrap_or(60.0);
                        ui.add_sized(
                            [w, 20.0],
                            egui::TextEdit::singleline(&mut line.qty)
                                .desired_width(w)
                                .hint_text("0"),
                        );

                        let w = widths.next().unwrap_or(60.0);
                        let mut unit = line.unit;
                        let combo_resp = egui::ComboBox::from_id_salt(format!("unit_{}", i))
                            .selected_text(unit.as_str())
                            .width(w)
                            .show_ui(ui, |ui| {
                                for u in Unit::all() {
                                    ui.selectable_value(&mut unit, *u, u.as_str());
                                }
                            });
                        line.unit = unit;
                        let _ = combo_resp;

                        let w = widths.next().unwrap_or(80.0);
                        ui.add_sized(
                            [w, 20.0],
                            egui::TextEdit::singleline(&mut line.price)
                                .desired_width(w)
                                .hint_text("0,00"),
                        );

                        let w = widths.next().unwrap_or(80.0);
                        let sum_str = line
                            .sum_value()
                            .map(|v| fmt::format_money(v))
                            .unwrap_or_default();
                        ui.add_sized([w, 20.0], egui::Label::new(sum_str).selectable(false));

                        let _w = widths.next().unwrap_or(40.0);
                        if ui.button("×").on_hover_text("Удалить строку").clicked() {
                            remove_idx = Some(i);
                        }
                    });
                    ui.separator();
                }
                if let Some(i) = remove_idx {
                    self.remove_line(i);
                }
            });
        }

        fn render_totals(&self, ui: &mut egui::Ui) {
            ui.add_space(4.0);
            let total = self.doc.total();
            let n = self.doc.filled_count();
            let x = fmt::format_rubles_rounded(total);
            let words = crate::money_words::amount_to_words(total);
            let cash = fmt::format_money(total);

            egui::Grid::new("totals_grid")
                .num_columns(2)
                .striped(false)
                .min_col_width(140.0)
                .show(ui, |ui| {
                    ui.strong("Итого:");
                    ui.label(fmt::format_money(total));
                    ui.end_row();

                    ui.strong("Всего наименований:");
                    ui.label(format!("{}, на сумму {} руб.", n, x));
                    ui.end_row();

                    ui.strong("Сумма прописью:");
                    ui.label(words);
                    ui.end_row();

                    ui.strong("Оплата / Наличные:");
                    ui.label(cash);
                    ui.end_row();

                    ui.strong("Сдача:");
                    ui.label("0,00");
                    ui.end_row();
                });
            ui.add_space(4.0);
        }

        /// Рисует окно истории распечаток.
        fn render_history(&mut self, ui: &mut egui::Ui) {
            ui.horizontal(|ui| {
                ui.heading(format!(
                    "История распечаток ({} записей)",
                    self.history_entries.len()
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🔄 Обновить").clicked() {
                        let store = HistoryStore::new(self.history_dir.clone());
                        self.history_entries = store.list();
                        self.history_selected = None;
                    }
                    if ui.button("📂 Открыть папку").clicked() {
                        let _ = open::that(&self.history_dir);
                    }
                });
            });
            ui.separator();

            if self.history_entries.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label("История пуста.");
                    ui.label(
                        "Сформируйте первую заявку кнопкой «Сформировать PDF» — \
                         и она появится здесь.",
                    );
                });
                return;
            }

            // Список записей слева, детали выбранной — справа.
            let selected_idx = self.history_selected;
            let mut new_selection = selected_idx;

            egui::SidePanel::left("history_list")
                .resizable(true)
                .default_width(380.0)
                .show_inside(ui, |ui| {
                    ui.heading("Заявки:");
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (i, entry) in self.history_entries.iter().enumerate() {
                            let is_selected = selected_idx == Some(i);
                            let total_str = fmt::format_money(entry.total);
                            let button_text = format!(
                                "{}\n{} от {} — {} ({} наим.)",
                                entry.number,
                                entry.date,
                                entry.timestamp.chars().take(10).collect::<String>(),
                                total_str,
                                entry.items_count,
                            );
                            let resp = ui.add_sized(
                                [ui.available_width(), 56.0],
                                egui::SelectableLabel::new(is_selected, &button_text),
                            );
                            if resp.clicked() {
                                new_selection = Some(i);
                            }
                        }
                    });
                });

            self.history_selected = new_selection;

            // Детали выбранной записи.
            egui::CentralPanel::default().show_inside(ui, |ui| {
                if let Some(idx) = self.history_selected {
                    if let Some(entry) = self.history_entries.get(idx) {
                        let entry_clone = entry.clone();
                        ui.heading(format!("Заявка № {}", entry.number));
                        ui.add_space(4.0);
                        egui::Grid::new("history_detail_grid")
                            .num_columns(2)
                            .striped(true)
                            .min_col_width(120.0)
                            .show(ui, |ui| {
                                ui.strong("Дата:");
                                ui.label(&entry.date);
                                ui.end_row();
                                ui.strong("Создано:");
                                ui.label(&entry.timestamp);
                                ui.end_row();
                                ui.strong("Наименований:");
                                ui.label(entry.items_count.to_string());
                                ui.end_row();
                                ui.strong("Итого:");
                                ui.label(fmt::format_money(entry.total));
                                ui.end_row();
                                ui.strong("Прописью:");
                                ui.label(&entry.total_words);
                                ui.end_row();
                                if let Some(pdf) = &entry.pdf_path {
                                    ui.strong("PDF:");
                                    ui.label(pdf);
                                    ui.end_row();
                                }
                            });

                        ui.add_space(8.0);
                        ui.heading("Строки товаров:");
                        ui.separator();

                        egui::ScrollArea::vertical().show(ui, |ui| {
                            egui::Grid::new("history_items_grid")
                                .num_columns(6)
                                .striped(true)
                                .min_col_width(50.0)
                                .show(ui, |ui| {
                                    ui.strong("№");
                                    ui.strong("Товар");
                                    ui.strong("кол-во");
                                    ui.strong("ед.");
                                    ui.strong("цена");
                                    ui.strong("сумма");
                                    ui.end_row();
                                    for item in &entry_clone.items {
                                        ui.label(item.index.to_string());
                                        ui.label(&item.name);
                                        ui.label(format_qty_short(item.qty));
                                        ui.label(&item.unit);
                                        ui.label(fmt::format_money(item.price));
                                        ui.label(fmt::format_money(item.sum));
                                        ui.end_row();
                                    }
                                });
                        });

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if let Some(pdf) = &entry_clone.pdf_path {
                                if ui.button("📄 Открыть PDF").clicked() {
                                    let _ = open::that(pdf);
                                }
                            }
                            if ui.button("🗑 Удалить из истории").clicked() {
                                let store = HistoryStore::new(self.history_dir.clone());
                                match store.delete(&entry_clone.number, &entry_clone.timestamp) {
                                    Ok(()) => {
                                        self.history_entries = store.list();
                                        self.history_selected = None;
                                    }
                                    Err(e) => {
                                        eprintln!("Не удалось удалить: {}", e);
                                    }
                                }
                            }
                        });
                    } else {
                        ui.label("Запись не найдена.");
                    }
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label("Выберите заявку слева для просмотра деталей.");
                    });
                }
            });
        }
    }

    /// Вспомогательная функция: форматирование количества в коротком виде
    /// (целое — без дробной части).
    fn format_qty_short(v: f64) -> String {
        if v.fract().abs() < 1e-9 {
            format!("{}", v as i64)
        } else {
            format!("{:.3}", v).replace('.', ",")
        }
    }

    fn filled_signature(doc: &Document) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        for line in &doc.lines {
            line.name.hash(&mut h);
            line.qty.hash(&mut h);
            line.unit.hash(&mut h);
            line.price.hash(&mut h);
        }
        h.finish()
    }

    fn load_draft(path: &std::path::Path) -> Option<Document> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str::<Document>(&content).ok()
    }
}
