//! Модуль графического интерфейса на egui/eframe.

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
            pdf_result_rx: None,
        }
    }

    /// Переключает между тёмной и светлой темой.
    pub fn toggle_theme(&mut self, ctx: &egui::Context) {
        self.dark_theme = !self.dark_theme;
        self.apply_theme(ctx);
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
    /// мессенджеры или email. Формат: только список товаров + итоги
    /// (без шапки с номером заявки и поставщиком — по запросу пользователя).
    fn copy_to_clipboard(&mut self, ctx: &egui::Context) {
        let total = self.doc.total();
        let total_str = fmt::format_money(total);
        let total_words = crate::money_words::amount_to_words(total);
        let n = self.doc.filled_count();
        let x = fmt::format_rubles_rounded(total);

        let mut text = String::new();

        // Список товаров — без шапки, сразу нумерованный список.
        for (idx, line) in (1u32..).zip(self.doc.filled_lines()) {
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
        }

        text.push('\n');
        text.push_str(&format!("Итого: {} руб.\n", total_str));
        text.push_str(&format!(
            "Всего наименований {}, на сумму {} руб.\n",
            n, x
        ));
        text.push_str(&format!("{}\n", total_words));

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
        });

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
                ui.separator();
                // Импорт только из буфера обмена — без файлового диалога.
                if ui.button("Вставить из буфера").clicked() {
                    self.import_csv_from_clipboard(ui.ctx());
                }
                ui.separator();
                if ui.button("Копировать текст").clicked() {
                    self.copy_to_clipboard(ui.ctx());
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
