//! Модуль графического интерфейса на egui/eframe.

use crate::fmt;
use crate::history::{HistoryEntry, HistoryItem, HistoryStore};
use crate::model::{self, Document, LineItem, Unit};
use crate::pdf;
use crate::APP_NAME;
use eframe::egui;
use std::path::PathBuf;

/// Размер окна по умолчанию.
pub const WINDOW_WIDTH: f32 = 950.0;
pub const WINDOW_HEIGHT: f32 = 720.0;

/// Кэш рассчитанных итогов. Пересчитывается только если изменился
/// `filled_signature` документа — это убирает ~5 аллокаций (format_money,
/// format_rubles_rounded, amount_to_words и т.д.) на каждый кадр egui.
struct TotalsCache {
    signature: u64,
    total_str: String,
    n_items: usize,
    rubles_rounded_str: String,
    total_words: String,
    cash_str: String,
}

/// Состояние приложения.
pub struct App {
    pub doc: Document,
    pub draft_path: PathBuf,
    pub pdf_dir: PathBuf,
    pub history_dir: PathBuf,
    pub status_message: String,
    pub last_filled_signature: u64,
    pub show_new_doc_dialog: bool,
    pub show_history_window: bool,
    pub history_entries: Vec<HistoryEntry>,
    pub history_selected: Option<usize>,
    pub dark_theme: bool,
    /// Кэш заголовка окна — не дёргаем ViewportCommand::Title каждый кадр.
    last_title: String,
    /// Кэш применённой темы — не вызываем set_visuals каждый кадр.
    last_dark_theme: Option<bool>,
    /// Кэш рассчитанных итогов. None = инвалидирован.
    totals_cache: Option<TotalsCache>,
    /// Флаг: черновик изменился и ждёт записи на диск (debounce 500мс).
    dirty: bool,
    /// Timestamp последнего изменения документа.
    last_change: Option<std::time::Instant>,
    /// Флаг: идёт генерация PDF в фоновом потоке.
    pdf_in_progress: bool,
    /// Стек предыдущих состояний документа для Undo (Ctrl+Z).
    /// Максимум 50 снимков — больше не нужно, экономит память.
    pub undo_stack: Vec<Vec<LineItem>>,
    /// Показывать окно ввода процента наценки.
    pub show_markup_dialog: bool,
    /// Текущее значение процента наценки в окне (строка, парсится в f64).
    pub markup_input: String,
    /// Показывать окно предпросмотра текста для копирования в мессенджер.
    /// В окне можно отредактировать текст перед копированием.
    pub show_copy_dialog: bool,
    /// Текущий текст в окне предпросмотра. Редактируется пользователем.
    pub copy_dialog_text: String,
    /// Канал для получения результата фоновой генерации PDF.
    pdf_result_rx: Option<std::sync::mpsc::Receiver<PdfResult>>,
}

/// Результат фоновой генерации PDF. Шлётся рабочим потоком в основной.
enum PdfResult {
    Ok { pdf_path: PathBuf, history_path: Option<PathBuf> },
    Err(String),
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
        let initial_title = format!("Заявка на отгрузку № {} — {}", doc.number, APP_NAME);
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
            dark_theme: false,
            last_title: initial_title,
            last_dark_theme: None,
            totals_cache: None,
            dirty: false,
            last_change: None,
            pdf_in_progress: false,
            undo_stack: Vec::new(),
            show_markup_dialog: false,
            markup_input: "10".to_string(),
            show_copy_dialog: false,
            copy_dialog_text: String::new(),
            pdf_result_rx: None,
        }
    }

    /// Переключает между тёмной и светлой темой.
    pub fn toggle_theme(&mut self, ctx: &egui::Context) {
        self.dark_theme = !self.dark_theme;
        self.apply_theme(ctx);
    }

    /// Сохраняет снимок строк в стек Undo перед изменяющим действием.
    /// Стек ограничен 50 снимками — этого достаточно для типичной сессии
    /// и не даёт памяти расти бесконечно.
    fn push_undo(&mut self) {
        const MAX_UNDO: usize = 50;
        if self.undo_stack.len() >= MAX_UNDO {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(self.doc.lines.clone());
    }

    /// Откат последнего действия. Достаёт снимок из стека и восстанавливает.
    /// Если стек пуст — сообщает «Нечего отменять».
    fn undo(&mut self) {
        if let Some(prev_lines) = self.undo_stack.pop() {
            self.doc.lines = prev_lines;
            // totals_cache инвалидируется через filled_signature
            // (он проверяется в totals() при следующем вызове).
            self.totals_cache = None;
            self.status_message = "Действие отменено (Ctrl+Z)".to_string();
            if self.doc.lines.is_empty() {
                self.doc.lines.push(LineItem::empty());
            }
            // Undo — это изменение, помечаем как dirty (debounce сработает).
            self.mark_dirty();
        } else {
            self.status_message = "Нечего отменять".to_string();
        }
    }

    /// Применяет процент наценки ко всем строкам с ценой.
    /// Округляет до копеек (×100, round, /100). Undo доступно.
    /// Например, percent=10 увеличивает все цены на 10%.
    fn apply_markup(&mut self, percent: f64) {
        self.push_undo();
        let multiplier = 1.0 + percent / 100.0;
        let mut affected = 0usize;
        for line in &mut self.doc.lines {
            if let Some(price) = line.price_value() {
                let new_price = (price * multiplier * 100.0).round() / 100.0;
                line.price = format!("{:.2}", new_price);
                affected += 1;
            }
        }
        self.totals_cache = None;  // суммы строк изменились
        self.mark_dirty();
        self.status_message = format!(
            "Наценка {}% применена к {} строкам",
            percent, affected
        );
    }

    /// Применяет текущую тему. ВАЖНО: вызывать только при переключении
    /// темы — не каждый кадр. На Windows повторный set_visuals
    /// пересоздаёт стиль и слегка дёргает интерфейс.
    pub fn apply_theme(&self, ctx: &egui::Context) {
        if self.dark_theme {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }
    }

    /// Возвращает кэш итогов документа. Пересчитывает только если
    /// изменился `filled_signature`. Раньше: каждый кадр egui (~60 FPS)
    /// вызывал doc.total() + format_money + amount_to_words (~8 аллокаций).
    fn totals(&mut self) -> &TotalsCache {
        let sig = filled_signature(&self.doc);
        let needs_rebuild = self
            .totals_cache
            .as_ref()
            .map_or(true, |c| c.signature != sig);
        if needs_rebuild {
            let total = self.doc.total();
            let n = self.doc.filled_count();
            self.totals_cache = Some(TotalsCache {
                signature: sig,
                total_str: fmt::format_money(total),
                n_items: n,
                rubles_rounded_str: fmt::format_rubles_rounded(total),
                total_words: crate::money_words::amount_to_words(total),
                cash_str: fmt::format_money(total),
            });
        }
        self.totals_cache.as_ref().unwrap()
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

    /// Помечает черновик как изменённый. Реальная запись через 500мс.
    fn mark_dirty(&mut self) {
        self.dirty = true;
        self.last_change = Some(std::time::Instant::now());
    }

    /// Debounce: пишет на диск если dirty=true и прошло > 500мс.
    fn maybe_flush_dirty(&mut self) {
        if !self.dirty {
            return;
        }
        let ready = self
            .last_change
            .map(|t| t.elapsed() >= std::time::Duration::from_millis(500))
            .unwrap_or(true);
        if ready {
            self.save_draft();
            self.dirty = false;
            self.last_change = None;
        }
    }

    /// Проверяет, не пришёл ли результат фоновой генерации PDF.
    fn poll_pdf_result(&mut self) {
        if let Some(rx) = &self.pdf_result_rx {
            match rx.try_recv() {
                Ok(PdfResult::Ok { pdf_path, history_path }) => {
                    let mut msg = format!("PDF сохранён: {}", pdf_path.display());
                    if let Some(ron_path) = history_path {
                        msg.push_str(&format!("\nИстория: {}", ron_path.display()));
                    }
                    if let Err(e) = open::that(&pdf_path) {
                        msg = format!(
                            "PDF создан, но не удалось открыть просмотрщик: {}",
                            e
                        );
                    }
                    self.status_message = msg;
                    self.pdf_in_progress = false;
                    self.pdf_result_rx = None;
                }
                Ok(PdfResult::Err(e)) => {
                    self.status_message = format!("Ошибка генерации PDF: {}", e);
                    self.pdf_in_progress = false;
                    self.pdf_result_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.status_message =
                        "Ошибка: рабочий поток генерации PDF завершился аварийно".to_string();
                    self.pdf_in_progress = false;
                    self.pdf_result_rx = None;
                }
            }
        }
    }

    fn new_document(&mut self) {
        self.push_undo();  // сохранить текущее состояние перед очисткой
        self.doc = Document {
            number: model::generate_number(),
            date: fmt::today_string(),
            lines: vec![LineItem::empty()],
        };
        self.last_filled_signature = 0;
        self.totals_cache = None;
        self.status_message = "Создана новая заявка".to_string();
        self.save_draft();
    }

    fn generate_and_open_pdf(&mut self) {
        // Защита от двойного нажатия.
        if self.pdf_in_progress {
            self.status_message =
                "Генерация PDF уже идёт, подождите…".to_string();
            return;
        }
        if self.doc.filled_count() == 0 {
            self.status_message = "Нет заполненных строк для формирования PDF".to_string();
            return;
        }
        let _ = std::fs::create_dir_all(&self.pdf_dir);
        let filename = format!("Заявка {}.pdf", self.doc.number);
        let path = self.pdf_dir.join(&filename);

        // Клонируем всё для рабочего потока.
        let doc = self.doc.clone();
        let pdf_path = path.clone();
        let history_dir = self.history_dir.clone();

        let (tx, rx) = std::sync::mpsc::channel::<PdfResult>();

        self.pdf_in_progress = true;
        self.pdf_result_rx = Some(rx);
        self.status_message = format!(
            "Генерация PDF в фоне… (→ {})", path.display()
        );

        std::thread::spawn(move || {
            let result = pdf::generate_pdf(&doc, &pdf_path);
            match result {
                Ok(()) => {
                    let entry = HistoryEntry::from_document(&doc, Some(&pdf_path));
                    let store = HistoryStore::new(history_dir);
                    let history_path = match store.save(&entry) {
                        Ok(p) => Some(p),
                        Err(e) => {
                            eprintln!("Не удалось сохранить историю: {}", e);
                            None
                        }
                    };
                    let _ = tx.send(PdfResult::Ok {
                        pdf_path,
                        history_path,
                    });
                }
                Err(e) => {
                    let _ = tx.send(PdfResult::Err(e));
                }
            }
        });
    }

    fn add_empty_line(&mut self) {
        self.push_undo();
        self.doc.lines.push(LineItem::empty());
    }

    fn remove_line(&mut self, idx: usize) {
        if idx < self.doc.lines.len() {
            self.push_undo();
            self.doc.lines.remove(idx);
            if self.doc.lines.is_empty() {
                self.doc.lines.push(LineItem::empty());
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
                self.push_undo();
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

    /// Открывает диалог предпросмотра текста для копирования в мессенджер.
    ///
    /// Текст генерируется в формате «вариант D» (emoji + бренд/артикул на
    /// отдельной строке) и помещается в редактируемое поле. Пользователь
    /// может поправить текст перед копированием.
    fn open_copy_dialog(&mut self) {
        let total = self.doc.total();
        let total_words = crate::money_words::amount_to_words(total);
        let n = self.doc.filled_count();

        let chat_lines: Vec<ChatLineInfo> = self
            .doc
            .filled_lines()
            .into_iter()
            .map(|line| ChatLineInfo {
                name: line.name.clone(),
                qty: line.qty_value().unwrap_or(0.0),
                unit: line.unit.as_str().to_string(),
                price: line.price_value().unwrap_or(0.0),
                sum: line.sum_value().unwrap_or(0.0),
            })
            .collect();

        self.copy_dialog_text = format_lines_for_messenger(
            &chat_lines,
            &self.doc.number,
            total,
            &total_words,
            n,
        );
        self.show_copy_dialog = true;
    }

    /// Открывает диалог предпросмотра для записи из истории.
    /// Тот же формат, что и open_copy_dialog, но данные из HistoryEntry.
    fn open_copy_dialog_for_history(
        &mut self,
        items: &[HistoryItem],
        doc_number: &str,
        total: f64,
        total_words: &str,
        n: usize,
    ) {
        let chat_lines: Vec<ChatLineInfo> = items
            .iter()
            .map(|item| ChatLineInfo {
                name: item.name.clone(),
                qty: item.qty,
                unit: item.unit.clone(),
                price: item.price,
                sum: item.sum,
            })
            .collect();
        self.copy_dialog_text = format_lines_for_messenger(
            &chat_lines,
            doc_number,
            total,
            total_words,
            n,
        );
        self.show_copy_dialog = true;
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
        // Принудительно сохраняем черновик при выходе — debounce может
        // оставить dirty=true, если пользователь закрыл окно быстрее
        // чем через 500мс после последнего нажатия клавиши.
        self.save_draft();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // 1. Сначала проверяем, не пришёл ли результат фоновой генерации PDF.
        self.poll_pdf_result();

        // Если генерация идёт — просим egui перерисовать.
        if self.pdf_in_progress {
            ctx.request_repaint();
        }

        // Тему применяем только при изменении флага dark_theme — не каждый кадр.
        // Тема не «слетает» сама, egui хранит её в ctx.
        if self.last_dark_theme != Some(self.dark_theme) {
            self.apply_theme(&ctx);
            self.last_dark_theme = Some(self.dark_theme);
        }

        // Заголовок окна обновляем только при изменении — частый
        // send_viewport_cmd(Title) перегружает оконную систему на Windows.
        let title = format!("Заявка на отгрузку № {} — {}", self.doc.number, APP_NAME);
        if self.last_title != title {
            self.last_title = title.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }

        // Горячие клавиши. egui даёт доступ к состоянию модификаторов и
        // нажатым клавишам через `ctx.input(|i| ...)`. Используем
        // `i.key_pressed(...)` + `i.modifiers.ctrl` для проверки Ctrl.
        //
        // ВАЖНО: `i.consume_key(...)` не используем — он удаляет событие,
        // и текстовые поля egui (TextEdit) могут не получить его. Просто
        // проверяем нажатие и вызываем действие. Если фокус в TextEdit,
        // Ctrl+N / Ctrl+P / Ctrl+S всё равно сработают как горячие клавиши.
        //
        // Исключение: Ctrl+S в TextEdit обычно вставляет символ, но в egui
        // Ctrl+S не имеет стандартного действия — мы его перехватываем.
        ctx.input(|i| {
            // Ctrl+N — новая заявка (с подтверждением через диалог).
            if i.key_pressed(egui::Key::N) && i.modifiers.ctrl {
                self.show_new_doc_dialog = true;
            }
            // Ctrl+P — сформировать PDF.
            // На Windows Ctrl+P — печать в браузерах, но в окне приложения
            // это наш обработчик. egui не перехватывает его по умолчанию.
            if i.key_pressed(egui::Key::P) && i.modifiers.ctrl && !i.modifiers.shift {
                self.generate_and_open_pdf();
            }
            // Ctrl+S — принудительное сохранение черновика (минуя debounce).
            if i.key_pressed(egui::Key::S) && i.modifiers.ctrl {
                self.save_draft();
                self.dirty = false;
                self.last_change = None;
                self.status_message = "Черновик сохранён (Ctrl+S)".to_string();
            }
            // Ctrl+Shift+H — открыть окно истории.
            if i.key_pressed(egui::Key::H) && i.modifiers.ctrl && i.modifiers.shift {
                let store = HistoryStore::new(self.history_dir.clone());
                self.history_entries = store.list();
                self.history_selected = None;
                self.show_history_window = true;
            }
            // Ctrl+Shift+T — переключить тему.
            if i.key_pressed(egui::Key::T) && i.modifiers.ctrl && i.modifiers.shift {
                self.toggle_theme(&ctx);
            }
            // Ctrl+Shift+C — открыть диалог копирования в чат.
            // «C» — Copy, как в большинстве приложений. Shift+Ctrl чтобы
            // не конфликтовать с обычным Ctrl+C (копирование выделенного текста).
            if i.key_pressed(egui::Key::C) && i.modifiers.ctrl && i.modifiers.shift {
                self.open_copy_dialog();
            }
        });

        // Ctrl+Z — откат последнего действия.
        // ВАЖНО: проверяем egui_wants_keyboard_input() — если фокус в TextEdit
        // (например, пользователь вводит имя товара), Ctrl+Z должен работать
        // как undo текста внутри TextEdit, а не как наш undo документа.
        // egui_wants_keyboard_input возвращает true, когда TextEdit «захватил»
        // ввод — в этом случае мы не перехватываем Ctrl+Z.
        let undo_pressed = ctx.input(|i| {
            i.key_pressed(egui::Key::Z)
                && (i.modifiers.ctrl || i.modifiers.command)
        });
        if undo_pressed && !ctx.egui_wants_keyboard_input() {
            self.undo();
        }

        if self.show_new_doc_dialog {
            let mut open = true;
            egui::Window::new("Новая заявка")
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ui, |ui| {
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
            egui::Window::new("История распечаток")
                .collapsible(false)
                .resizable(true)
                .default_width(900.0)
                .default_height(600.0)
                .open(&mut open)
                .show(ui, |ui| {
                    self.render_history(ui);
                });
            if !open {
                self.show_history_window = false;
            }
        }

        // Окно ввода процента наценки.
        // Открывается по кнопке «Наценка %» в тулбаре. Применяет процент
        // ко всем строкам с ценой, округляя до копеек. Undo доступно через
        // push_undo() внутри apply_markup.
        if self.show_markup_dialog {
            let mut open = true;
            egui::Window::new("Наценка на все товары")
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ui, |ui| {
                    ui.label("Введите процент наценки (10 = +10% к цене):");
                    ui.add_space(4.0);
                    ui.text_edit_singleline(&mut self.markup_input);
                    ui.add_space(4.0);
                    ui.label("Цены всех строк будут увеличены. Undo: Ctrl+Z.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Применить").clicked() {
                            // Нормализуем ввод: убираем пробелы, запятую → точка.
                            let cleaned: String = self
                                .markup_input
                                .chars()
                                .filter(|c| !c.is_whitespace())
                                .map(|c| if c == ',' { '.' } else { c })
                                .collect();
                            match cleaned.parse::<f64>() {
                                Ok(p) => {
                                    self.apply_markup(p);
                                    self.show_markup_dialog = false;
                                }
                                Err(_) => {
                                    self.status_message =
                                        "Некорректный процент".to_string();
                                }
                            }
                        }
                        if ui.button("Отмена").clicked() {
                            self.show_markup_dialog = false;
                        }
                    });
                });
            if !open {
                self.show_markup_dialog = false;
            }
        }

        // Диалог предпросмотра текста для копирования в мессенджер.
        // Текст уже сгенерирован в self.copy_dialog_text (формат «вариант D» —
        // emoji + бренд/артикул на отдельной строке). Пользователь может
        // отредактировать его в TextEdit перед копированием.
        //
        // Окно большое и немодальное (resizable) — удобно, когда текст длинный.
        // При нажатии «Копировать» — копируем self.copy_dialog_text в буфер,
        // закрываем окно, показываем статус.
        if self.show_copy_dialog {
            let mut open = true;
            egui::Window::new("Копирование в чат")
                .collapsible(false)
                .resizable(true)
                .default_width(640.0)
                .default_height(480.0)
                .min_width(400.0)
                .min_height(300.0)
                .open(&mut open)
                .show(ui, |ui| {
                    ui.label(
                        "Текст в формате для мессенджера (Telegram/WhatsApp). \
                         Можно отредактировать перед копированием:",
                    );
                    ui.add_space(4.0);

                    // Большое многострочное текстовое поле.
                    // .code_editor() даёт моноширинный шрифт и нумерацию строк,
                    // но здесь лучше обычный TextEdit — текст будет читаемым
                    // в пропорциональном шрифте, как в мессенджере.
                    let available_height = ui.available_height() - 36.0;
                    ui.add_sized(
                        [ui.available_width(), available_height.max(80.0)],
                        egui::TextEdit::multiline(&mut self.copy_dialog_text)
                            .desired_width(f32::MAX)
                            .font(egui::TextStyle::Monospace),
                    );

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        // Кнопка «Копировать» — копирует текст из поля.
                        if ui.button("📋 Копировать").clicked() {
                            ui.ctx().copy_text(self.copy_dialog_text.clone());
                            self.status_message =
                                "Текст скопирован в буфер обмена".to_string();
                            self.show_copy_dialog = false;
                        }
                        // Кнопка «Закрыть» — без копирования.
                        if ui.button("Закрыть").clicked() {
                            self.show_copy_dialog = false;
                        }
                        ui.separator();
                        // Подсказка по формату — мелким шрифтом.
                        ui.label(
                            egui::RichText::new(
                                "Формат: 🛒 шапка → 1️⃣ товар (бренд/артикул на новой строке) → 💰 итог",
                            )
                            .small()
                            .color(egui::Color32::GRAY),
                        );
                    });
                });
            if !open {
                self.show_copy_dialog = false;
            }
        }

        egui::Panel::bottom("status_bar").show(ui, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                if self.pdf_in_progress {
                    ui.spinner();
                    ui.label("Генерация PDF в фоне…");
                    ui.separator();
                }
                ui.label(&self.status_message);
            });
            ui.add_space(2.0);
        });

        egui::Panel::bottom("totals_panel")
            .resizable(false)
            .show(ui, |ui| { self.render_totals(ui); });

        egui::Panel::top("toolbar").show(ui, |ui| {
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
                if ui.button("Отменить (Ctrl+Z)").clicked() {
                    self.undo();
                }
                ui.separator();
                // Импорт только из буфера обмена — без файлового диалога.
                if ui.button("Вставить из буфера").clicked() {
                    self.import_csv_from_clipboard(ui.ctx());
                }
                ui.separator();
                if ui.button("Копировать текст").clicked() {
                    self.open_copy_dialog();
                }
                ui.separator();
                // Наценка на все товары — открывает диалог ввода процента.
                if ui.button("Наценка %").clicked() {
                    self.show_markup_dialog = true;
                }
                ui.separator();
                // Кнопка истории — текст, не эмодзи (егui не рендерит эмодзи).
                if ui.button("История").clicked() {
                    // Обновляем кэш истории при открытии окна.
                    let store = HistoryStore::new(self.history_dir.clone());
                    self.history_entries = store.list();
                    self.history_selected = None;
                    self.show_history_window = true;
                }
                // Кнопка переключения темы (тёмная/светлая).
                let theme_label = if self.dark_theme { "Светлая тема" } else { "Тёмная тема" };
                if ui.button(theme_label).clicked() {
                    self.toggle_theme(ui.ctx());
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

        egui::CentralPanel::default().show(ui, |ui| {
            self.render_table(ui);
        });

        self.ensure_trailing_empty();

        // Проверяем, не изменился ли документ. Если да — помечаем как dirty.
        // Реальная запись на диск через debounce 500мс.
        let cur_sig = filled_signature(&self.doc);
        if cur_sig != self.last_filled_signature {
            self.last_filled_signature = cur_sig;
            self.mark_dirty();
        }

        // Debounce: если с последнего изменения прошло > 500мс — пишем.
        self.maybe_flush_dirty();
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
                        .map(fmt::format_money)
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

    fn render_totals(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        // Кэш итогов — пересчёт только при изменении signature.
        let totals = self.totals();
        let total_str = totals.total_str.clone();
        let n_items = totals.n_items;
        let rubles_rounded = totals.rubles_rounded_str.clone();
        let words = totals.total_words.clone();
        let cash = totals.cash_str.clone();

        egui::Grid::new("totals_grid")
            .num_columns(2)
            .striped(false)
            .min_col_width(140.0)
            .show(ui, |ui| {
                ui.strong("Итого:");
                ui.label(total_str);
                ui.end_row();

                ui.strong("Всего наименований:");
                ui.label(format!("{}, на сумму {} руб.", n_items, rubles_rounded));
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
                if ui.button("Обновить").clicked() {
                    let store = HistoryStore::new(self.history_dir.clone());
                    self.history_entries = store.list();
                    self.history_selected = None;
                }
                if ui.button("Открыть папку").clicked() {
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

        egui::Panel::left("history_list")
            .resizable(true)
            .default_size(380.0)
            .show(ui, |ui| {
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
                        // selectable_label вместо SelectableLabel (убран в egui 0.36).
                        let avail_w = ui.available_width();
                        ui.allocate_ui(egui::Vec2::new(avail_w, 56.0), |ui| {
                            let resp = ui.selectable_label(is_selected, &button_text);
                            if resp.clicked() {
                                new_selection = Some(i);
                            }
                        });
                    }
                });
            });

        self.history_selected = new_selection;

        // Детали выбранной записи.
        //
        // Раньше: entry.clone() каждый кадр — клонировали всю запись,
        // включая Vec<HistoryItem> (десятки аллокаций на больших историях).
        // Причина клона: внутри блока было `self.history_entries = store.list()`
        // (после удаления), что конфликтует с заимствованием entry.
        //
        // Теперь: собираем «отложенные действия» в Option-переменные,
        // освобождаем borrow после show(), затем выполняем действия.
        let mut open_pdf_request: Option<String> = None;
        let mut delete_request: Option<(String, String)> = None;
        // (items, doc_number, total, total_words, items_count) — нужно для
        // копирования записи из истории в буфер обмена. Clone items —
        // компромисс: он копирует Vec<HistoryItem>, но только при клике
        // пользователя. doc_number нужен для шапки «🛒 Заявка № ...».
        let mut copy_history_request: Option<(Vec<HistoryItem>, String, f64, String, usize)> = None;

        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(idx) = self.history_selected {
                if let Some(entry) = self.history_entries.get(idx) {
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

                    let items = &entry.items;
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
                                for item in items {
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
                        if let Some(pdf) = &entry.pdf_path {
                            if ui.button("Открыть PDF").clicked() {
                                open_pdf_request = Some(pdf.clone());
                            }
                        }
                        // Кнопка копирования заявки из истории в буфер обмена.
                        // Полезно, когда нужно переотправить старую заявку в Telegram
                        // без повторного ввода товаров.
                        if ui.button("Скопировать в чат").clicked() {
                            copy_history_request = Some((
                                entry.items.clone(),
                                entry.number.clone(),
                                entry.total,
                                entry.total_words.clone(),
                                entry.items_count,
                            ));
                        }
                        if ui.button("Удалить из истории").clicked() {
                            delete_request =
                                Some((entry.number.clone(), entry.timestamp.clone()));
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

        // Выполняем отложенные действия после освобождения borrow.
        if let Some(pdf_path) = open_pdf_request {
            let _ = open::that(&pdf_path);
        }
        if let Some((number, timestamp)) = delete_request {
            let store = HistoryStore::new(self.history_dir.clone());
            match store.delete(&number, &timestamp) {
                Ok(()) => {
                    self.history_entries = store.list();
                    self.history_selected = None;
                }
                Err(e) => {
                    eprintln!("Не удалось удалить: {}", e);
                }
            }
        }
        if let Some((items, doc_number, total, total_words, n)) = copy_history_request {
            // Открываем диалог предпросмотра — пользователь может
            // отредактировать текст перед копированием.
            self.open_copy_dialog_for_history(&items, &doc_number, total, &total_words, n);
        }
    }
}

/// Унифицированная информация о строке товара для копирования в чат.
///
/// Нужна, чтобы copy_to_clipboard (для текущего документа) и
/// copy_history_to_clipboard (для записи из истории) использовали одну
/// функцию форматирования. LineItem и HistoryItem имеют разные поля
/// (String vs f64), но обе приводятся к этому общему типу.
struct ChatLineInfo {
    name: String,
    qty: f64,
    unit: String,
    price: f64,
    sum: f64,
}

/// Форматирует список товаров в текст для мессенджера (вариант D).
///
/// Формат с emoji и визуальной структурой. Бренд и артикул — на отдельной
/// строке с отступом, чтобы основное наименование было легко прочитать
/// на узком экране телефона в Telegram/WhatsApp.
///
/// Пример:
/// ```text
/// 🛒 Заявка № DZHOД123456
///
/// 1️⃣ Сайлентблок рычага подвески перед прав/лев LADA LARGUS
///    LYNXauto (C8617) · 2 шт × 358,00 = 716,00 ₽
///
/// 2️⃣ Наконечник рулевой тяги перед лев LADA LARGUS/XRAY
///    LYNXauto (C4147L) · 1 × 1 142,00 = 1 142,00 ₽
///
/// 💰 Итого: 3 930,22 ₽
/// 📋 7 наименований
/// Три тысячи девятьсот тридцать рублей 22 копейки
/// ```
///
/// Особенности:
/// - emoji используются как визуальные маркеры (🛒 заявка, 1️⃣ номер,
///   💰 итог, 📋 количество). На большинстве мессенджеров рендерятся
///   корректно (Telegram, WhatsApp, iMessage, VK).
/// - Бренд и артикул (если есть в наименовании) переносятся на отдельную
///   строку с отступом в 3 пробела — на телефоне основное наименование
///   занимает первую строку целиком.
/// - Если qty=1, «шт» не пишем — короче.
/// - Если qty целое (2, 5, 8), показываем без дробной части.
/// - «₽» вместо «руб.» — компактнее и современнее.
/// - Неразрывный пробел NBSP в числах заменён на обычный — некоторые
///   Android-клиенты Telegram рендерят NBSP как странный символ.
fn format_lines_for_messenger(
    lines: &[ChatLineInfo],
    doc_number: &str,
    total: f64,
    total_words: &str,
    n: usize,
) -> String {
    let mut text = String::new();

    // Шапка — короткая, только номер заявки.
    text.push_str("🛒 Заявка № ");
    text.push_str(doc_number);
    text.push_str("\n\n");

    for (idx, line) in (1u32..).zip(lines.iter()) {
        // 1️⃣, 2️⃣, ... — emoji-нумерация 1-9. Для 10+ падаем обратно на «10.»
        // (вариации эмодзи 10-20 нестабильны на старых ОС).
        let prefix = match idx {
            1 => "1️⃣ ",
            2 => "2️⃣ ",
            3 => "3️⃣ ",
            4 => "4️⃣ ",
            5 => "5️⃣ ",
            6 => "6️⃣ ",
            7 => "7️⃣ ",
            8 => "8️⃣ ",
            9 => "9️⃣ ",
            _ => {
                // Для 10+ — текстовая нумерация «N.».
                // Вариации эмодзи 1️⃣0️⃣ на старых телефонах рендерятся как два
                // отдельных символа. Проще откатиться к «N.».
                text.push_str(&format!("{}. ", idx));
                ""  // prefix уже записан в text выше
            }
        };
        text.push_str(prefix);

        // Разбиваем наименование на «основное» и «доп. информация».
        // Доп. информация — это [Бренд] и (Артикул), которые импорт CSV
        // добавляет в конец наименования. Их переносим на следующую строку.
        let (main_name, extras) = split_name_and_extras(&line.name);
        text.push_str(&main_name);
        text.push('\n');

        // Доп. строка с брендом/артикулом и количеством/ценой.
        text.push_str("   ");  // отступ 3 пробела
        if !extras.is_empty() {
            text.push_str(&extras);
            text.push_str(" · ");
        }

        // Количество: для qty=1 — просто «1», для целых — без дробной части.
        let qty_str = format_qty_for_messenger(line.qty, &line.unit);
        text.push_str(&qty_str);

        // Цена и сумма.
        text.push_str(" × ");
        text.push_str(&fmt::format_money(line.price));
        text.push_str(" = ");
        text.push_str(&fmt::format_money(line.sum));
        text.push_str(" ₽\n");

        // Пустая строка между товарами — визуально разделяет блоки.
        text.push('\n');
    }

    // Подвал — итоги с emoji.
    text.push_str("💰 Итого: ");
    text.push_str(&fmt::format_money(total));
    text.push_str(" ₽\n");
    text.push_str(&format!("📋 {} наименований\n", n));
    text.push_str(total_words);
    text.push('\n');

    text
}

/// Разбивает полное наименование на основную часть и доп. информацию.
///
/// Импорт CSV формирует наименование как «Товар [Бренд] (Артикул)» —
/// бренд и артикул могут идти в любом порядке (в зависимости от поставщика).
/// Здесь мы разделяем это на:
/// - main_name: «Товар» — основное наименование товара
/// - extras: «Бренд (Артикул)» — доп. информация, переносится на след. строку
///
/// Если в наименовании нет [Бренд] или (Артикул), extras будет пустым,
/// а main_name — полным наименованием.
fn split_name_and_extras(full_name: &str) -> (String, String) {
    // Ищем последние [Бренд] и (Артикул) в исходной строке —
    // независимый поиск по обоим, т.к. порядок может быть любым.
    let brand = full_name
        .rfind(']')
        .and_then(|close| full_name[..close].rfind('['))
        .map(|open| full_name[open + 1..full_name.rfind(']').unwrap()].trim().to_string())
        .filter(|b| !b.is_empty());

    let article = full_name
        .rfind(')')
        .and_then(|close| full_name[..close].rfind('('))
        .map(|open| {
            let close = full_name.rfind(')').unwrap();
            full_name[open + 1..close].trim().to_string()
        })
        .filter(|a| !a.is_empty())
        .map(|a| format!("({})", a));

    // Убираем [Бренд] и (Артикул) из main_name, даже если они пустые.
    // Используем replace для «[]» и «()» (пустые скобки) — это нужно,
    // когда пользователь ввёл «Товар [] ()» — extras будут пустыми,
    // но скобки не должны остаться в main_name.
    let mut main = full_name.to_string();
    if let Some(b) = &brand {
        main = main.replace(&format!("[{}]", b), "");
    } else {
        // Нет бренда — но могут быть пустые [].
        main = main.replace("[]", "");
    }
    if let Some(a) = &article {
        main = main.replace(a, "");
    } else {
        // Нет артикула — но могут быть пустые ().
        main = main.replace("()", "");
    }
    // Схлопываем множественные пробелы в один и тримим концы.
    let main = main
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");

    // Собираем extras: «Бренд (Артикул)» или только одно из них.
    let mut extras_parts: Vec<String> = Vec::new();
    if let Some(b) = brand {
        extras_parts.push(b);
    }
    if let Some(a) = article {
        extras_parts.push(a);
    }
    let extras = extras_parts.join(" ");

    (main, extras)
}

/// Форматирует количество для мессенджера: «1» для qty=1 (без «шт»),
/// «2 шт» для целых штук, «1,500 к-кт» для дробных.
fn format_qty_for_messenger(qty: f64, unit: &str) -> String {
    if (qty - 1.0).abs() < 1e-9 {
        // Для qty=1 не пишем единицу — короче и понятнее.
        return "1".to_string();
    }
    if qty.fract().abs() < 1e-9 {
        // Целое количество — без дробной части.
        format!("{} {}", qty as i64, unit)
    } else {
        // Дробное — 3 знака после запятой (стандарт для количества).
        format!("{} {}", format!("{:.3}", qty).replace('.', ","), unit)
    }
}

/// Вспомогательная функция: форматирование количества в коротком виде.
/// Раньше дублировала `fmt::format_qty` — теперь делегирует туда.
///
/// Оставлена как тонкая обёртка, чтобы не менять вызовы в `copy_to_clipboard`
/// и `render_history`. В будущем можно убрать совсем.
fn format_qty_short(v: f64) -> String {
    fmt::format_qty(v)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Тест: split_name_and_extras корректно разбивает «Товар [Бренд] (Артикул)».
    /// Это формат, который csv_import создаёт в on_close.
    #[test]
    fn split_name_extracts_brand_and_article() {
        let (main, extras) = split_name_and_extras(
            "Сайлентблок рычага подвески | перед прав/лев | LADA LARGUS [LYNXauto] (C8617)",
        );
        assert_eq!(main, "Сайлентблок рычага подвески | перед прав/лев | LADA LARGUS");
        assert_eq!(extras, "LYNXauto (C8617)");
    }

    #[test]
    fn split_name_only_brand_no_article() {
        let (main, extras) = split_name_and_extras("Фильтр салонный [BM]");
        assert_eq!(main, "Фильтр салонный");
        assert_eq!(extras, "BM");
    }

    #[test]
    fn split_name_only_article_no_brand() {
        let (main, extras) = split_name_and_extras("Фильтр масляный (OG313HQ)");
        assert_eq!(main, "Фильтр масляный");
        assert_eq!(extras, "(OG313HQ)");
    }

    #[test]
    fn split_name_no_extras() {
        let (main, extras) = split_name_and_extras("Просто наименование товара");
        assert_eq!(main, "Просто наименование товара");
        assert_eq!(extras, "");
    }

    #[test]
    fn split_name_empty_brackets_ignored() {
        // Если скобки пустые — extras не должен их включать.
        let (main, extras) = split_name_and_extras("Товар [] ()");
        assert_eq!(main, "Товар");
        assert_eq!(extras, "");
    }

    /// Тест: format_qty_for_messenger — qty=1 без единицы.
    #[test]
    fn qty_one_has_no_unit() {
        assert_eq!(format_qty_for_messenger(1.0, "шт"), "1");
        assert_eq!(format_qty_for_messenger(1.0, "к-кт"), "1");
    }

    /// Тест: целое количество — без дробной части.
    #[test]
    fn qty_integer_no_fraction() {
        assert_eq!(format_qty_for_messenger(2.0, "шт"), "2 шт");
        assert_eq!(format_qty_for_messenger(8.0, "к-кт"), "8 к-кт");
    }

    /// Тест: дробное количество — 3 знака после запятой, запятая.
    #[test]
    fn qty_fraction_three_decimals() {
        assert_eq!(format_qty_for_messenger(1.5, "шт"), "1,500 шт");
        assert_eq!(format_qty_for_messenger(0.5, "к-кт"), "0,500 к-кт");
    }

    /// Тест: полный формат мессенджера на 3 товарах.
    /// Проверяем структуру: шапка, нумерация, итоги, прописью.
    #[test]
    fn messenger_format_full_structure() {
        let lines = vec![
            ChatLineInfo {
                name: "Товар A [Brand1] (Art1)".to_string(),
                qty: 2.0,
                unit: "шт".to_string(),
                price: 100.0,
                sum: 200.0,
            },
            ChatLineInfo {
                name: "Товар B [Brand2] (Art2)".to_string(),
                qty: 1.0,
                unit: "шт".to_string(),
                price: 500.0,
                sum: 500.0,
            },
            ChatLineInfo {
                name: "Товар C".to_string(),
                qty: 1.0,
                unit: "к-кт".to_string(),
                price: 50.0,
                sum: 50.0,
            },
        ];
        let text = format_lines_for_messenger(
            &lines,
            "DZHOД123456",
            750.0,
            "Семьсот пятьдесят рублей 00 копеек",
            3,
        );

        // Шапка с номером заявки.
        assert!(text.contains("🛒 Заявка № DZHOД123456"), "должна быть шапка");

        // Нумерация 1️⃣, 2️⃣, 3️⃣.
        assert!(text.contains("1️⃣"), "должна быть 1️⃣");
        assert!(text.contains("2️⃣"), "должна быть 2️⃣");
        assert!(text.contains("3️⃣"), "должна быть 3️⃣");

        // Бренд и артикул на отдельной строке (с отступом).
        assert!(text.contains("   Brand1 (Art1)"), "бренд и артикул должны быть с отступом");

        // Для qty=1 не пишем «шт».
        // «1 × 500» для второго товара (qty=1, нет «шт»).
        assert!(text.contains("1 × 500,00 = 500,00 ₽"), "qty=1 без ед.");

        // Для qty=2 — «2 шт».
        assert!(text.contains("2 шт × 100,00 = 200,00 ₽"), "qty=2 с ед.");

        // Подвал — итоги с emoji.
        assert!(text.contains("💰 Итого: 750,00 ₽"), "должен быть итог");
        assert!(text.contains("📋 3 наименований"), "должно быть количество");
        assert!(
            text.contains("Семьсот пятьдесят рублей 00 копеек"),
            "должна быть прописью"
        );
    }

    /// Тест: 10+ товаров падает обратно на текстовую нумерацию (без emoji),
    /// потому что «1️⃣0️⃣» на старых телефонах рендерится как два символа.
    #[test]
    fn messenger_format_10_plus_uses_text_numbers() {
        let mut lines = Vec::new();
        // Создаём 11 товаров. После нумерации 1..11: 1️⃣...9️⃣ для 1-9,
        // «10.» для 10-го, «11.» для 11-го.
        for i in 0..11 {
            lines.push(ChatLineInfo {
                name: format!("Товар {}", i),
                qty: 1.0,
                unit: "шт".to_string(),
                price: 100.0,
                sum: 100.0,
            });
        }
        let text = format_lines_for_messenger(
            &lines,
            "DZHOД123456",
            1100.0,
            "Одна тысяча сто рублей 00 копеек",
            11,
        );

        // 10-й по порядку товар (индекс 9 в lines) должен использовать «10.»
        // а не «1️⃣0️⃣».
        assert!(
            text.contains("10. Товар 9"),
            "10+ должен быть с точкой, не emoji. Текст:\n{}",
            text
        );
    }
}
