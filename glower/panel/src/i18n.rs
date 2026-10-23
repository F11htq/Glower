// Язык интерфейса. Строки в коде остаются русскими, а перевод ставится
// поверх: каждая надпись, подсказка и заголовок, показываясь на экране,
// ищется в словаре выбранного языка. Чего в словаре нет, остаётся как есть

use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

/// Строка с подстановками: «Удалить «{имя}»?» — куски текста между ними
struct Образец {
    куски: Vec<String>,
    имена: Vec<String>,
    цель: String,
}

struct Словарь {
    точные: HashMap<String, String>,
    образцы: Vec<Образец>,
    /// Уже переведённое и то, чего в словаре нет: второй раз не ищем
    готовые: RefCell<HashSet<String>>,
    мимо: RefCell<HashSet<String>>,
}

thread_local! {
    static СЛОВАРЬ: RefCell<Option<std::rc::Rc<Словарь>>> = const { RefCell::new(None) };
}

fn текст_словаря(язык: &str) -> Option<&'static str> {
    Some(match язык {
        "en" => include_str!("lang/en.json"),
        "uk" => include_str!("lang/uk.json"),
        "be" => include_str!("lang/be.json"),
        "kk" => include_str!("lang/kk.json"),
        "de" => include_str!("lang/de.json"),
        "fr" => include_str!("lang/fr.json"),
        "es" => include_str!("lang/es.json"),
        "it" => include_str!("lang/it.json"),
        "pt" => include_str!("lang/pt.json"),
        "pl" => include_str!("lang/pl.json"),
        "tr" => include_str!("lang/tr.json"),
        _ => return None,
    })
}

/// Разбор «{имя}», «{}», «{:.1}»: куски текста и имена подстановок
fn разбери(строка: &str) -> (Vec<String>, Vec<String>) {
    let mut куски = vec![String::new()];
    let mut имена = vec![];
    let mut ост = строка;
    while let Some(н) = ост.find('{') {
        let Some(к) = ост[н..].find('}') else { break };
        куски.last_mut().unwrap().push_str(&ост[..н]);
        let внутри = &ост[н + 1..н + к];
        имена.push(внутри.split(':').next().unwrap_or("").to_string());
        куски.push(String::new());
        ост = &ост[н + к + 1..];
    }
    куски.last_mut().unwrap().push_str(ост);
    (куски, имена)
}

fn есть_кириллица(т: &str) -> bool {
    т.chars().any(|с| ('\u{400}'..='\u{4ff}').contains(&с))
}

fn собери(пары: HashMap<String, String>) -> Словарь {
    let mut точные = HashMap::new();
    let mut образцы = vec![];
    for (ру, перевод) in пары {
        if ру.contains('{') {
            let (куски, имена) = разбери(&ру);
            // Образец из одних подстановок подошёл бы к чему угодно
            if !имена.is_empty() && куски.iter().any(|к| есть_кириллица(к)) {
                образцы.push(Образец { куски, имена, цель: перевод });
                continue;
            }
        }
        точные.insert(ру, перевод);
    }
    // Длинные образцы точнее: «Удалить все программы {}» раньше «Удалить {}»
    образцы.sort_by_key(|о| std::cmp::Reverse(о.куски.iter().map(|к| к.len()).sum::<usize>()));
    Словарь { точные, образцы, готовые: RefCell::default(), мимо: RefCell::default() }
}

impl Образец {
    fn подойди(&self, т: &str, точные: &HashMap<String, String>) -> Option<String> {
        let первый = &self.куски[0];
        if !т.starts_with(первый.as_str()) {
            return None;
        }
        let mut поз = первый.len();
        let mut взятое = vec![];
        let n = self.куски.len();
        for (i, кусок) in self.куски.iter().enumerate().skip(1) {
            if i == n - 1 {
                if !т.ends_with(кусок.as_str()) || т.len() - кусок.len() < поз {
                    return None;
                }
                взятое.push(&т[поз..т.len() - кусок.len()]);
            } else if кусок.is_empty() {
                // Две подстановки подряд: всё достаётся первой
                взятое.push("");
            } else {
                let н = т[поз..].find(кусок.as_str())?;
                взятое.push(&т[поз..поз + н]);
                поз += н + кусок.len();
            }
        }
        // «{}{}» в конце: первой — всё, второй — ничего, порядок сохранится
        let (цель_куски, цель_имена) = разбери(&self.цель);
        let mut без_имени = self.имена.iter().zip(взятое.iter()).filter(|(и, _)| и.is_empty()).map(|(_, в)| *в);
        let mut вышло = цель_куски[0].clone();
        for (имя, кусок) in цель_имена.iter().zip(цель_куски.iter().skip(1)) {
            let значение = if имя.is_empty() {
                без_имени.next().unwrap_or("")
            } else {
                self.имена.iter().position(|и| и == имя).map(|п| взятое[п]).unwrap_or("")
            };
            // Подставленное тоже может быть надписью: «Этот компьютер — Проводник»
            вышло.push_str(точные.get(значение).map(|с| с.as_str()).unwrap_or(значение));
            вышло.push_str(кусок);
        }
        Some(вышло)
    }
}

impl Словарь {
    /// Точное совпадение; «Пятница» ищем и как «пятница» — в словаре дни
    /// недели и месяцы строчными, а в начале фразы они с заглавной
    fn точно(&self, т: &str) -> Option<String> {
        if let Some(п) = self.точные.get(т) {
            return Some(п.clone());
        }
        let mut буквы = т.chars();
        let первая = буквы.next()?;
        if !первая.is_uppercase() {
            return None;
        }
        let строчная: String = первая.to_lowercase().chain(буквы).collect();
        let п = self.точные.get(&строчная)?;
        let mut п_буквы = п.chars();
        let п_первая = п_буквы.next()?;
        Some(п_первая.to_uppercase().chain(п_буквы).collect())
    }

    /// Склейки вроде «Пт, 23 октября» или «🏠 Главная — Параметры»: слова
    /// между цифрами, значками и знаками переводим по отдельности. Только если
    /// известны все русские куски — полуперевод хуже русского
    fn по_кускам(&self, т: &str) -> Option<String> {
        let буквы: Vec<char> = т.chars().collect();
        let mut вышло = String::new();
        let mut кусок = String::new();
        let mut было = false;
        let mut сбрось = |кусок: &mut String, вышло: &mut String| -> Option<()> {
            if есть_кириллица(кусок) {
                вышло.push_str(&self.точно(кусок)?);
                было = true;
            } else {
                вышло.push_str(кусок);
            }
            кусок.clear();
            Some(())
        };
        for (i, &с) in буквы.iter().enumerate() {
            let слово = с.is_alphabetic()
                || (с == ' ' && i > 0 && буквы[i - 1].is_alphabetic() && буквы.get(i + 1).is_some_and(|н| н.is_alphabetic()));
            if слово {
                кусок.push(с);
            } else {
                сбрось(&mut кусок, &mut вышло)?;
                вышло.push(с);
            }
        }
        сбрось(&mut кусок, &mut вышло)?;
        было.then_some(вышло)
    }

    fn переведи(&self, т: &str) -> Option<String> {
        if !есть_кириллица(т) || self.готовые.borrow().contains(т) || self.мимо.borrow().contains(т) {
            return None;
        }
        let найдено = self.точно(т).or_else(|| {
            // Пробелы по краям — часть вёрстки, а не строки
            let чистое = т.trim();
            if чистое.len() != т.len() && !чистое.is_empty() {
                let н = т.find(чистое).unwrap_or(0);
                return self.точно(чистое).map(|п| format!("{}{п}{}", &т[..н], &т[н + чистое.len()..]));
            }
            None
        }).or_else(|| self.образцы.iter().find_map(|о| о.подойди(т, &self.точные)))
            .or_else(|| self.по_кускам(т));
        match &найдено {
            Some(п) => {
                self.готовые.borrow_mut().insert(п.clone());
            }
            None => {
                self.мимо.borrow_mut().insert(т.to_string());
            }
        }
        найдено
    }
}

fn словарь() -> Option<std::rc::Rc<Словарь>> {
    СЛОВАРЬ.with(|с| с.borrow().clone())
}

/// Перевод строки для мест, где надпись не виджет: пункты списков выбора
pub fn tr(т: &str) -> String {
    словарь().and_then(|с| с.переведи(т)).unwrap_or_else(|| т.to_string())
}

fn под_запретом(в: &gtk::Widget) -> bool {
    let mut тек = Some(в.clone());
    while let Some(w) = тек {
        if w.style_context().has_class("no-i18n") {
            return true;
        }
        тек = w.parent();
    }
    false
}

fn переведи_виджет(в: &gtk::Widget) {
    let Some(с) = словарь() else { return };
    if под_запретом(в) {
        return;
    }
    let впервые = unsafe { в.data::<bool>("glower-i18n").is_none() };
    if впервые {
        unsafe { в.set_data("glower-i18n", true) };
    }
    if let Some(н) = в.downcast_ref::<gtk::Label>() {
        if let Some(п) = с.переведи(&н.label()) {
            н.set_label(&п);
        }
        if впервые {
            н.connect_label_notify(|н| {
                if let Some(п) = словарь().and_then(|с| с.переведи(&н.label())) {
                    н.set_label(&п);
                }
            });
        }
    }
    if let Some(п) = в.tooltip_text().and_then(|т| с.переведи(&т)) {
        в.set_tooltip_text(Some(&п));
    }
    if впервые && в.tooltip_text().is_some() {
        в.connect_notify_local(Some("tooltip-text"), |в, _| {
            if let Some(п) = в.tooltip_text().and_then(|т| словарь().and_then(|с| с.переведи(&т))) {
                в.set_tooltip_text(Some(&п));
            }
        });
    }
    if let Some(п) = в.downcast_ref::<gtk::Entry>() {
        if let Some(н) = п.placeholder_text().and_then(|т| с.переведи(&т)) {
            п.set_placeholder_text(Some(&н));
        }
    }
    if let Some(о) = в.downcast_ref::<gtk::Window>() {
        if let Some(н) = о.title().and_then(|т| с.переведи(&т)) {
            о.set_title(&н);
        }
        if впервые {
            о.connect_title_notify(|о| {
                if let Some(н) = о.title().and_then(|т| словарь().and_then(|с| с.переведи(&т))) {
                    о.set_title(&н);
                }
            });
        }
    }
}

unsafe extern "C" fn при_показе(
    _подсказка: *mut glib::gobject_ffi::GSignalInvocationHint,
    _число: u32,
    доводы: *const glib::gobject_ffi::GValue,
    _данные: glib::ffi::gpointer,
) -> glib::ffi::gboolean {
    let значение = &*(доводы as *const glib::Value);
    if let Ok(в) = значение.get::<gtk::Widget>() {
        переведи_виджет(&в);
    }
    glib::ffi::GTRUE
}

/// Включить перевод в этой программе. Зовётся сразу после gtk::init
pub fn подключи() {
    let язык = crate::core::язык_сейчас();
    let Some(текст) = текст_словаря(&язык) else { return };
    let пары: HashMap<String, String> = serde_json::from_str(текст).unwrap_or_default();
    СЛОВАРЬ.with(|с| *с.borrow_mut() = Some(std::rc::Rc::new(собери(пары))));
    // Каждый виджет, появляясь на экране, проходит через «map» — тут и переводим,
    // до первого кадра, чтобы русский не мелькал
    unsafe {
        use glib::translate::IntoGlib;
        let id = glib::gobject_ffi::g_signal_lookup(c"map".as_ptr(), gtk::Widget::static_type().into_glib());
        if id != 0 {
            glib::gobject_ffi::g_signal_add_emission_hook(id, 0, Some(при_показе), std::ptr::null_mut(), None);
        }
    }
}

#[cfg(test)]
mod проверки {
    use super::*;

    fn словарь_из(пары: &[(&str, &str)]) -> Словарь {
        собери(пары.iter().map(|(а, б)| (а.to_string(), б.to_string())).collect())
    }

    #[test]
    fn точный_и_с_подстановками() {
        let с = словарь_из(&[
            ("Удалить", "Delete"),
            ("Удалить «{имя}»?", "Delete “{имя}”?"),
            ("🔎 «{что}» в «{}»", "🔎 «{}» ішінде «{что}»"),
            ("Стр {}, столб {}", "Ln {}, Col {}"),
        ]);
        assert_eq!(с.переведи("Удалить").as_deref(), Some("Delete"));
        assert_eq!(с.переведи("  Удалить ").as_deref(), Some("  Delete "));
        assert_eq!(с.переведи("Удалить «Отчёт.txt»?").as_deref(), Some("Delete “Отчёт.txt”?"));
        assert_eq!(с.переведи("🔎 «кот» в «Документы»").as_deref(), Some("🔎 «Документы» ішінде «кот»"));
        assert_eq!(с.переведи("Стр 3, столб 14").as_deref(), Some("Ln 3, Col 14"));
        assert_eq!(с.переведи("Удалить всё"), None);
        assert_eq!(с.переведи("Удалить «Удалить»?").as_deref(), Some("Delete “Delete”?"));
        let с = словарь_из(&[("Главная", "Home"), ("Параметры", "Settings"), ("октября", "October"), ("Пт", "Fri")]);
        assert_eq!(с.переведи("🏠 Главная").as_deref(), Some("🏠 Home"));
        assert_eq!(с.переведи("Главная — Параметры").as_deref(), Some("Home — Settings"));
        assert_eq!(с.переведи("Пт, 23 октября").as_deref(), Some("Fri, 23 October"));
        assert_eq!(с.переведи("Главная страница"), None);
        let с = словарь_из(&[("пятница", "vendredi"), ("октября", "octobre")]);
        assert_eq!(с.переведи("Пятница, 23 октября").as_deref(), Some("Vendredi, 23 octobre"));
        // Переведённое второй раз не трогаем
        assert_eq!(с.переведи("Delete"), None);
    }

    #[test]
    fn словари_читаются_и_подстановки_сходятся() {
        for (k, _, _) in crate::core::ЯЗЫКИ.iter().skip(1) {
            let т = текст_словаря(k).unwrap_or_else(|| panic!("нет словаря {k}"));
            let пары: HashMap<String, String> = serde_json::from_str(т).expect(k);
            assert!(пары.len() > 1000, "{k}: мало строк");
            for (ру, п) in &пары {
                let mut а = разбери(ру).1;
                let mut б = разбери(п).1;
                а.sort();
                б.sort();
                assert_eq!(а, б, "{k}: {ру} → {п}");
            }
        }
    }
}
