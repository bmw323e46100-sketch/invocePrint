//! Модель данных заявки: структуры, расчёт итогов, генерация номера.

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
            // По умолчанию qty=1 — большинство товаров заказывают по 1 шт.
            // Раньше было пусто, что заставляло пользователя вводить «1» в каждой строке.
            qty: "1".to_string(),
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
    use rand::RngExt;
    let mut rng = rand::rng();
    let n: u32 = rng.random_range(0..1_000_000);
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
