//! Модуль форматирования чисел и дат по русским правилам.
//! Разделитель тысяч — неразрывный пробел, десятичный — запятая.

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
    let mut n = n.unsigned_abs();
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

/// Форматирует количество товара: целое — без дробной части,
/// дробное — с 3 знаками после запятой (запятая как разделитель).
///
/// Раньше эта логика была дублирована в `pdf::format_qty(Option<f64>)` и
/// `gui::format_qty_short(f64)` — теперь одна реализация здесь.
///
/// Примеры: `1.0` → `"1"`, `1.5` → `"1,500"`, `8.0` → `"8"`.
pub fn format_qty(v: f64) -> String {
    if v.fract().abs() < 1e-9 {
        format!("{}", v as i64)
    } else {
        format!("{:.3}", v).replace('.', ",")
    }
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
