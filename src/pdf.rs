//! Модуль генерации PDF-документа через genpdf.
//! Шрифт DejaVu Serif (3447 глифов) вшит через include_bytes!.

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

// Глобальный bitmap всех codepoint-ов, поддерживаемых шрифтом DejaVu Serif.
//
// Раньше: thread_local<RefCell<Option<Face>>> + glyph_index(c) на каждый
// символ — O(log n) cmap lookup + BorrowMut на каждый чар.
// Теперь: один раз парсим Face, обходим cmap subtables через codepoints()
// callback, заполняем HashSet. После инициализации contains() — O(1).
//
// Используем std::sync::OnceLock (стабилен с Rust 1.70). Управляющие
// символы \n \r \t в cmap не входят, но мы их пропускаем явно.
static SUPPORTED_CODEPOINTS: std::sync::OnceLock<std::collections::HashSet<u32>> =
    std::sync::OnceLock::new();

/// Возвращает ссылку на глобальный bitmap поддерживаемых символов.
/// Парсит шрифт один раз при первом обращении (через `OnceLock`).
fn supported_codepoints() -> &'static std::collections::HashSet<u32> {
    SUPPORTED_CODEPOINTS.get_or_init(|| {
        let mut set = std::collections::HashSet::with_capacity(4096);
        match ttf_parser::Face::parse(FONT_REGULAR, 0) {
            Ok(face) => {
                if let Some(cmap) = face.tables().cmap {
                    for subtable in cmap.subtables {
                        subtable.codepoints(|cp| {
                            set.insert(cp);
                        });
                    }
                }
                set.insert('\n' as u32);
                set.insert('\r' as u32);
                set.insert('\t' as u32);
                set
            }
            Err(_) => set,
        }
    })
}

/// Проверяет, поддерживает ли шрифт данный символ.
fn is_char_supported(c: char) -> bool {
    if c == '\n' || c == '\r' || c == '\t' {
        return true;
    }
    supported_codepoints().contains(&(c as u32))
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
    // Кэш FontData через OnceLock. Раньше каждый generate_pdf парсил
    // TTF заново (2 × ~700 КБ аллокаций). Теперь парсим один раз
    // за весь процесс и клонируем только FontData (содержит Arc для raw_data).
    //
    // get_or_try_init требует unstable once_cell_try, поэтому get_or_init
    // с .expect() — шрифт вшит в бинарник через include_bytes!, так что
    // panic внутри означает compile-time проблему с вшитыми данными.
    static REGULAR: std::sync::OnceLock<genpdf::fonts::FontData> =
        std::sync::OnceLock::new();
    static BOLD: std::sync::OnceLock<genpdf::fonts::FontData> =
        std::sync::OnceLock::new();

    let regular = REGULAR.get_or_init(|| {
        genpdf::fonts::FontData::new(FONT_REGULAR.to_vec(), None)
            .expect("вшитый DejaVu Serif Regular должен парситься")
    });
    let bold = BOLD.get_or_init(|| {
        genpdf::fonts::FontData::new(FONT_BOLD.to_vec(), None)
            .expect("вшитый DejaVu Serif Bold должен парситься")
    });

    let regular_clone = regular.clone();
    let bold_clone = bold.clone();
    Ok(genpdf::fonts::FontFamily {
        regular: regular_clone,
        bold: bold_clone.clone(),
        italic: regular.clone(),
        bold_italic: bold_clone,
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

    for (idx, line) in (1u32..).zip(doc.filled_lines()) {
        let sum = line.sum_value().unwrap_or(0.0);
        let qty_str = line
            .qty_value()
            .map(fmt::format_qty)
            .unwrap_or_default();
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
        Box::new(styled_paragraph("Итого:", bold, Alignment::Right)),
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
                .styled(line_style),
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
        Box::new(el::Paragraph::new("              (подпись)").styled(small_style)),
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
