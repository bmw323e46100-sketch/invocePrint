//! Модуль истории распечаток (RON-формат).
//! Каждая распечатанная заявка сохраняется в отдельный .ron-файл.

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
        ron_filename_for(&self.number, &self.timestamp)
    }
}

/// Вынесенная логика построения имени RON-файла по (number, timestamp).
///
/// Имя файла детерминированно кодирует оба поля, поэтому для удаления
/// записи по (number, timestamp) можно собрать имя напрямую и удалить
/// файл — без чтения и парсинга всех файлов истории.
///
/// Раньше `HistoryStore::delete` делал O(n) чтений и RON-десериализаций
/// ради одной записи. Теперь — O(1) один `remove_file`.
fn ron_filename_for(number: &str, timestamp: &str) -> String {
    // Та же трансформация символов, что и в HistoryEntry::ron_filename.
    let safe_ts = timestamp
        .chars()
        .map(|c| match c {
            ':' | ' ' | 'T' => '-',
            '+' => 'p',
            '.' => '-',
            _ => c,
        })
        .collect::<String>();
    format!("Заявка {} {}.ron", number, safe_ts)
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
    ///
    /// Раньше: O(n) чтений и RON-десериализаций (читали все файлы, парсили,
    /// сравнивали). На истории из 500 записей это 500 чтений с диска и
    /// 500 парсингов RON — заметно при каждом удалении.
    ///
    /// Теперь: O(1). Имя файла детерминированно строится из (number, timestamp)
    /// через `ron_filename_for` — собираем путь напрямую и вызываем
    /// `remove_file`. Никакого чтения или парсинга.
    ///
    /// Гарантия корректности: `ron_filename` вызывается при `save()` и
    /// `delete()` с одной и той же логикой трансформации, поэтому имя файла
    /// совпадает. Если файл не найден или не удалился — возвращаем ошибку.
    pub fn delete(&self, number: &str, timestamp: &str) -> Result<(), String> {
        let filename = ron_filename_for(number, timestamp);
        let path = self.dir.join(&filename);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(format!("Запись не найдена: {}", path.display()))
            }
            Err(e) => Err(format!("Не удалось удалить {}: {}", path.display(), e)),
        }
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

    /// Тест на O(1) удаление: при 100 записях в истории удаление одной
    /// не должно читать/парсить остальные. Косвенное доказательство:
    /// создаём 100 записей, удаляем одну, и проверяем, что:
    /// 1. Удаление прошло успешно.
    /// 2. Осталось ровно 99 записей.
    /// 3. Удаление записи с несуществующим timestamp возвращает ошибку
    ///    «Запись не найдена» — без паники.
    #[test]
    fn delete_with_large_history() {
        let tmp = std::env::temp_dir().join("zayavka_history_test_large");
        let _ = std::fs::remove_dir_all(&tmp);
        let store = HistoryStore::new(tmp.clone());

        // Создаём 100 записей с разными номерами и timestamp.
        let mut entries = Vec::new();
        for i in 0..100 {
            let mut entry = HistoryEntry::from_document(&make_doc(), None);
            entry.number = format!("DZHOД{:06}", i);
            entry.timestamp = format!("2026-10-{:02}T10:00:00+00:00", i + 1);
            entries.push(entry);
        }
        for entry in &entries {
            let _ = store.save(entry).expect("save failed");
        }
        assert_eq!(store.list().len(), 100);

        // Удаляем 50-ю запись.
        let target = &entries[50];
        let t = std::time::Instant::now();
        store
            .delete(&target.number, &target.timestamp)
            .expect("delete failed");
        let elapsed = t.elapsed();
        // O(1) удаление должно быть намного быстрее, чем чтение 100 RON-файлов.
        // На CI это может быть медленнее, поэтому宽松ный лимит 1 секунда.
        assert!(
            elapsed.as_millis() < 1000,
            "Удаление заняло {:?} — ожидалось < 1с для O(1)",
            elapsed
        );

        // Проверяем, что осталось 99.
        assert_eq!(store.list().len(), 99);

        // Проверяем, что удалённая запись действительно исчезла.
        let list = store.list();
        assert!(!list
            .iter()
            .any(|e| e.number == target.number && e.timestamp == target.timestamp));

        // Удаление несуществующей записи должно вернуть ошибку.
        let result = store.delete("DZHOД999999", "2026-10-01T10:00:00+00:00");
        assert!(result.is_err(), "Ожидалась ошибка для несуществующей записи");
        let err = result.unwrap_err();
        assert!(
            err.contains("не найден") || err.contains("не найдена"),
            "Сообщение об ошибке должно упоминать «не найден»: {}",
            err
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
