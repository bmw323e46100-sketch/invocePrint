//! Модуль преобразования суммы в слова на русском языке.
//! Эталон: 47100.00 → «Сорок семь тысяч сто рублей 00 копеек».

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
    if (11..=14).contains(&last_two) { return many; }
    match last {
        1 => one,
        2..=4 => few,
        _ => many,
    }
}

fn rubles_to_words(rubles: i64) -> String {
    if rubles == 0 { return "0 рублей".to_string(); }
    let mut parts: Vec<String> = Vec::new();
    let mut n = rubles;
    let mut millions = 0i64;
    let mut thousands = 0i64;
    if n >= 1_000_000 { millions = n / 1_000_000; n %= 1_000_000; }
    if n >= 1_000 { thousands = n / 1_000; n %= 1_000; }
    let units = n;
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
