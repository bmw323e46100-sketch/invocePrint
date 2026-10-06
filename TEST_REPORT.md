# Отчёт о прохождении тестов

## Окружение

- **ОС**: Linux x86_64
- **Rust**: rustc 1.99.0 (b940084d7 2026-09-28)
- **Cargo**: cargo 1.99.0 (5f94df478 2026-08-27)
- **Profile**: release (с оптимизацией)
- **Команда**: `cargo test --release`

## Итог

| Категория | Пройдено | Упало | Игнорировано |
|---|---|---|---|
| Unit-тесты (lib.rs) | 40 | 0 | 0 |
| Unit-тесты (main.rs) | 0 | 0 | 0 |
| Интеграционные тесты (tests/pdf_integration.rs) | 3 | 0 | 0 |
| Doc-тесты | 0 | 0 | 0 |
| **Всего** | **43** | **0** | **0** |

**Результат: ✅ ВСЕ ТЕСТЫ ПРОЙДЕНЫ**

## Покрытие тестами по модулям

### `fmt` — форматирование чисел и дат (11 тестов)

| Тест | Что проверяет |
|---|---|
| `money_basic` | Базовое форматирование: `47100.0` → `47 100,00`, `0.0` → `0,00`, `1234.5` → `1 234,50` |
| `money_negative` | Отрицательные числа: `-1234.5` → `-1 234,50` |
| `money_rounding` | Округление до копеек: `0.005` → `0,01`, `0.004` → `0,00` |
| `money_test_case_from_spec` | Тест-кейс спецификации: 9100, 1780, 1520, 12400 |
| `money_million` | Миллионы: `1 000 000,00` |
| `parse_accepts_comma_and_dot` | Парсинг: «9100», «9100,00», «9100.00», «9 100,00», «9 100,00» (NBSP) — все → 9100.0 |
| `parse_invalid_returns_none` | Некорректный ввод: «», «   », «abc» → None |
| `rubles_rounded_format` | Округлённые рубли для строки «Всего наименований N»: `12400.40` → `12 400`, `12400.50` → `12 401` |
| `date_format_basic` | Формат даты: 25.09.2026 → «25 сентября 2026 г.» |
| `date_all_months` | Все 12 месяцев в родительном падеже |
| `date_no_leading_zero` | Без ведущего нуля: 5.01.2026 → «5 января 2026 г.» |

### `money_words` — сумма прописью (21 тест)

| Тест | Вход | Ожидаемый выход |
|---|---|---|
| `zero` | 0.0 | «0 рублей 00 копеек» |
| `one_ruble` | 1.0 | «Один рубль 00 копеек» |
| `two_rubles` | 2.0 | «Два рубля 00 копеек» |
| `five_rubles` | 5.0 | «Пять рублей 00 копеек» |
| `eleven_rubles` | 11.0 | «Одиннадцать рублей 00 копеек» |
| `twenty_one_ruble` | 21.0 | «Двадцать один рубль 00 копеек» |
| `twenty_two_rubles` | 22.0 | «Двадцать два рубля 00 копеек» |
| `one_hundred` | 100.0 | «Сто рублей 00 копеек» |
| `one_hundred_one` | 101.0 | «Сто один рубль 00 копеек» |
| `one_thousand` | 1000.0 | «Одна тысяча рублей 00 копеек» |
| `two_thousand` | 2000.0 | «Две тысячи рублей 00 копеек» |
| `five_thousand` | 5000.0 | «Пять тысяч рублей 00 копеек» |
| `twenty_one_thousand` | 21000.0 | «Двадцать одна тысяча рублей 00 копеек» |
| `spec_reference_47100` | 47100.0 | **«Сорок семь тысяч сто рублей 00 копеек»** (эталон из ТЗ) |
| `test_case_12400` | 12400.0 | **«Двенадцать тысяч четыреста рублей 00 копеек»** (тест-кейс приёмки) |
| `one_million` | 1 000 000.0 | «Один миллион рублей 00 копеек» |
| `two_million` | 2 000 000.0 | «Два миллиона рублей 00 копеек» |
| `complex_with_kopecks` | 12345.67 | «Двенадцать тысяч триста сорок пять рублей 67 копеек» |
| `kopecks_variants` | 1.01 / 1.02 / 1.05 / 1.11 / 1.21 | «01 копейка / 02 копейки / 05 копеек / 11 копеек / 21 копейка» |
| `capitalization_first_letter` | любая сумма | первая буква заглавная |
| `declension_edge_12_13_14` | 12, 13, 14, 112, 1012 | всегда «рублей» (особый случай русского языка) |
| `declension_edge_thousands_12_14` | 12 000 / 13 000 / 14 000 / 21 000 / 22 000 / 25 000 | «тысяч» / «тысяч» / «тысяч» / «тысяча» / «тысячи» / «тысяч» |

### `model` — структуры данных и расчёт итогов (7 тестов)

| Тест | Что проверяет |
|---|---|
| `line_sum_basic` | Произведение количества на цену: 2 × 100 = 200 |
| `line_sum_with_comma` | Десятичная запятая: 1,5 × 100 = 150 |
| `line_filled_detection` | Строка считается заполненной, только если есть имя, кол-во ≠ 0 и цена |
| `total_skips_empty_lines` | Пустые строки исключаются из итога и счётчика |
| `test_case_from_spec` | **Тест-кейс приёмки:** 9100 + 1780 + 1520 = 12400, 3 наименования |
| `number_format` | Формат номера: «DZHOД» + 6 цифр = 11 символов |
| `unit_strings` | Единицы измерения: Sht → «шт», Kkt → «к-кт» |

### Интеграционные тесты PDF (3 теста)

| Тест | Что проверяет |
|---|---|
| `generates_pdf_with_test_case` | Генерирует PDF из тест-кейса спецификации: файл существует, размер > 50 КБ (шрифт вшит), начинается с `%PDF-`, содержит упоминание шрифта PT Serif, содержит номер заявки DZHOД и заголовок «Заявка» |
| `empty_document_still_generates_pdf` | PDF генерируется даже для пустого документа (0 строк) |
| `spec_amount_words_and_format` | Эталонные значения из ТЗ: 9100,00 / 1 780,00 / 1 520,00 / 12 400,00 / 12 400 / «Двенадцать тысяч четыреста рублей 00 копеек» |

## Полный вывод `cargo test --release`

```
running 40 tests
test fmt::tests::date_format_basic ... ok
test fmt::tests::date_all_months ... ok
test fmt::tests::date_no_leading_zero ... ok
test fmt::tests::money_million ... ok
test fmt::tests::money_basic ... ok
test fmt::tests::money_negative ... ok
test fmt::tests::money_rounding ... ok
test fmt::tests::money_test_case_from_spec ... ok
test fmt::tests::parse_accepts_comma_and_dot ... ok
test fmt::tests::parse_invalid_returns_none ... ok
test fmt::tests::rubles_rounded_format ... ok
test model::tests::line_filled_detection ... ok
test model::tests::line_sum_basic ... ok
test model::tests::line_sum_with_comma ... ok
test model::tests::number_format ... ok
test model::tests::test_case_from_spec ... ok
test model::tests::total_skips_empty_lines ... ok
test model::tests::unit_strings ... ok
test money_words::tests::capitalization_first_letter ... ok
test money_words::tests::complex_with_kopecks ... ok
test money_words::tests::declension_edge_12_13_14 ... ok
test money_words::tests::declension_edge_thousands_12_14 ... ok
test money_words::tests::eleven_rubles ... ok
test money_words::tests::five_rubles ... ok
test money_words::tests::five_thousand ... ok
test money_words::tests::kopecks_variants ... ok
test money_words::tests::one_hundred ... ok
test money_words::tests::one_hundred_one ... ok
test money_words::tests::one_million ... ok
test money_words::tests::one_ruble ... ok
test money_words::tests::one_thousand ... ok
test money_words::tests::spec_reference_47100 ... ok
test money_words::tests::test_case_12400 ... ok
test money_words::tests::twenty_one_ruble ... ok
test money_words::tests::twenty_one_thousand ... ok
test money_words::tests::twenty_two_rubles ... ok
test money_words::tests::two_million ... ok
test money_words::tests::two_rubles ... ok
test money_words::tests::two_thousand ... ok
test money_words::tests::zero ... ok

test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 3 tests
test empty_document_still_generates_pdf ... ok
test spec_amount_words_and_format ... ok
test generates_pdf_with_test_case ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Сборка

- `cargo build --release` — собирает один бинарник.
- Размер бинарника: ~11 МБ (включает встроенный шрифт, GUI-рантайм egui,
  PDF-генератор genpdf/printpdf).
- На Windows бинарник собирается без консольного окна
  (`#![windows_subsystem = "windows"]` в release-режиме).

## Автономность

Приложение:

- ✅ Не требует установки (один .exe).
- ✅ Не требует внешних рантаймов (статическая линковка).
- ✅ Не требует внешних шрифтов (PT Serif вшит через `include_bytes!`).
- ✅ Не делает сетевых запросов (нет HTTP, нет телеметрии).
- ✅ Работает офлайн.
