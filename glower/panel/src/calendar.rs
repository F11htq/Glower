//! Календарь — месяц сеткой и события по дням (glower-calendar).
//!
//! События лежат в ~/.config/glower/calendar.json: «2026-10-23» → список
//! {t, time}. У события со временем рабочий стол сам напомнит в срок —
//! даже когда Календарь закрыт.

use crate::apps::{кнопка, подготовь, пиши_данные, читай_данные};
use gtk::glib;
use gtk::prelude::*;
use serde_json::{json, Value};
use std::cell::Cell;
use std::rc::Rc;

const СТИЛЬ: &str = r#"
.calendar .month { font-size: 22px; font-weight: 300; }
.calendar .dow { opacity: 0.55; font-size: 12px; padding: 4px; }
.calendar button.day {
  border-radius: 10px;
  padding: 6px 8px;
  background-color: alpha(@glower_fg, 0.04);
}
.calendar button.day.out { opacity: 0.4; }
.calendar button.day.today { box-shadow: inset 0 0 0 2px @glower_accent; }
.calendar button.day.sel { background-color: alpha(@glower_accent, 0.25); }
.calendar .num { font-weight: bold; }
.calendar .ev { font-size: 11px; background-color: alpha(@glower_accent, 0.35); border-radius: 4px; padding: 0 4px; }
.calendar .side { padding: 14px; border-left: 1px solid @glower_line; }
.calendar .side-title { font-size: 16px; font-weight: bold; }
"#;

pub const ФАЙЛ: &str = "calendar.json";
const МЕСЯЦЫ: [&str; 12] = ["Январь", "Февраль", "Март", "Апрель", "Май", "Июнь", "Июль", "Август", "Сентябрь", "Октябрь", "Ноябрь", "Декабрь"];
const МЕСЯЦЫ_Р: [&str; 12] = ["января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа", "сентября", "октября", "ноября", "декабря"];

pub fn ключ(год: i32, месяц: u32, день: u32) -> String {
    format!("{год:04}-{месяц:02}-{день:02}")
}

pub fn дней_в_месяце(год: i32, месяц: u32) -> u32 {
    match месяц {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (год % 4 == 0 && год % 100 != 0) || год % 400 == 0 => 29,
        _ => 28,
    }
}

/// День недели первого числа: 0 — понедельник
pub fn сдвиг_месяца(год: i32, месяц: u32) -> u32 {
    glib::DateTime::from_local(год, месяц as i32, 1, 0, 0, 0.0).map(|д| (д.day_of_week() - 1) as u32).unwrap_or(0)
}

/// События дня по порядку времени
pub fn события(все: &Value, к: &str) -> Vec<Value> {
    let mut с: Vec<Value> = все[к].as_array().cloned().unwrap_or_default();
    с.sort_by_key(|е| е["time"].as_str().unwrap_or("99:99").to_string());
    с
}

struct Кал {
    год: Cell<i32>,
    месяц: Cell<u32>,
    выбран: Cell<(i32, u32, u32)>,
    сетка: gtk::Grid,
    заголовок: gtk::Label,
    бок: gtk::Box,
}

pub fn запусти(_доводы: Vec<String>) -> ! {
    подготовь("glower-calendar", "Календарь");
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль календаря");
    gtk::StyleContext::add_provider_for_screen(&gtk::gdk::Screen::default().expect("экран"), &стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_title("Календарь");
    окно.set_default_size(960, 640);
    окно.set_icon_name(Some("x-office-calendar"));
    окно.style_context().add_class("glower-app");
    crate::apps::строка_заголовка(&окно);
    окно.style_context().add_class("calendar");
    окно.connect_destroy(|_| gtk::main_quit());

    let сейчас = glib::DateTime::now_local().expect("время");
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    окно.add(&ряд);
    let лево = gtk::Box::new(gtk::Orientation::Vertical, 10);
    лево.set_margin(16);
    лево.set_hexpand(true);
    ряд.add(&лево);
    let шапка = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let заголовок = gtk::Label::new(None);
    заголовок.style_context().add_class("month");
    заголовок.set_xalign(0.0);
    заголовок.set_hexpand(true);
    шапка.add(&заголовок);
    let назад = кнопка("‹", "Предыдущий месяц");
    let сегодня = кнопка("Сегодня", "К сегодняшнему дню");
    let вперёд = кнопка("›", "Следующий месяц");
    шапка.add(&назад);
    шапка.add(&сегодня);
    шапка.add(&вперёд);
    лево.add(&шапка);
    let сетка = gtk::Grid::new();
    сетка.set_row_spacing(6);
    сетка.set_column_spacing(6);
    сетка.set_row_homogeneous(false);
    сетка.set_column_homogeneous(true);
    сетка.set_vexpand(true);
    лево.add(&сетка);
    let бок = gtk::Box::new(gtk::Orientation::Vertical, 8);
    бок.style_context().add_class("side");
    бок.set_size_request(340, -1);
    ряд.add(&бок);

    let к = Rc::new(Кал {
        год: Cell::new(сейчас.year()),
        месяц: Cell::new(сейчас.month() as u32),
        выбран: Cell::new((сейчас.year(), сейчас.month() as u32, сейчас.day_of_month() as u32)),
        сетка,
        заголовок,
        бок,
    });
    {
        let к = к.clone();
        назад.connect_clicked(move |_| листай(&к, -1));
    }
    {
        let к = к.clone();
        вперёд.connect_clicked(move |_| листай(&к, 1));
    }
    {
        let к = к.clone();
        сегодня.connect_clicked(move |_| {
            let н = glib::DateTime::now_local().expect("время");
            к.год.set(н.year());
            к.месяц.set(н.month() as u32);
            к.выбран.set((н.year(), н.month() as u32, н.day_of_month() as u32));
            рисуй(&к);
        });
    }
    // Колесо мыши листает месяцы
    {
        let к2 = к.clone();
        лево.add_events(gtk::gdk::EventMask::SCROLL_MASK);
        окно.connect_scroll_event(move |_, е| {
            match е.direction() {
                gtk::gdk::ScrollDirection::Up => листай(&к2, -1),
                gtk::gdk::ScrollDirection::Down => листай(&к2, 1),
                _ => {}
            }
            glib::Propagation::Proceed
        });
    }
    рисуй(&к);
    окно.show_all();
    gtk::main();
    std::process::exit(0);
}

fn листай(к: &Rc<Кал>, шаг: i32) {
    let mut м = к.месяц.get() as i32 + шаг;
    let mut г = к.год.get();
    if м < 1 {
        м = 12;
        г -= 1;
    }
    if м > 12 {
        м = 1;
        г += 1;
    }
    к.год.set(г);
    к.месяц.set(м as u32);
    рисуй(к);
}

fn рисуй(к: &Rc<Кал>) {
    let (г, м) = (к.год.get(), к.месяц.get());
    к.заголовок.set_text(&format!("{} {г}", МЕСЯЦЫ[(м - 1) as usize]));
    for р in к.сетка.children() {
        к.сетка.remove(&р);
    }
    for (i, д) in ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"].iter().enumerate() {
        let л = gtk::Label::new(Some(д));
        л.style_context().add_class("dow");
        к.сетка.attach(&л, i as i32, 0, 1, 1);
    }
    let все = читай_данные(ФАЙЛ);
    let н = glib::DateTime::now_local().expect("время");
    let сегодня = (н.year(), н.month() as u32, н.day_of_month() as u32);
    let сдвиг = сдвиг_месяца(г, м);
    let (пг, пм) = if м == 1 { (г - 1, 12) } else { (г, м - 1) };
    let (сг, см) = if м == 12 { (г + 1, 1) } else { (г, м + 1) };
    for клетка in 0..42u32 {
        let (год, мес, день, чужой) = if клетка < сдвиг {
            (пг, пм, дней_в_месяце(пг, пм) - сдвиг + клетка + 1, true)
        } else if клетка - сдвиг < дней_в_месяце(г, м) {
            (г, м, клетка - сдвиг + 1, false)
        } else {
            (сг, см, клетка - сдвиг - дней_в_месяце(г, м) + 1, true)
        };
        let б = gtk::Button::new();
        б.style_context().add_class("day");
        if чужой {
            б.style_context().add_class("out");
        }
        if (год, мес, день) == сегодня {
            б.style_context().add_class("today");
        }
        if (год, мес, день) == к.выбран.get() {
            б.style_context().add_class("sel");
        }
        let столбик = gtk::Box::new(gtk::Orientation::Vertical, 2);
        столбик.set_valign(gtk::Align::Start);
        let ч = gtk::Label::new(Some(&день.to_string()));
        ч.style_context().add_class("num");
        ч.set_xalign(0.0);
        столбик.add(&ч);
        let сд = события(&все, &ключ(год, мес, день));
        for е in сд.iter().take(2) {
            let т = format!("{}{}", е["time"].as_str().filter(|т| !т.is_empty()).map(|т| format!("{т} ")).unwrap_or_default(), е["t"].as_str().unwrap_or(""));
            let л = gtk::Label::new(Some(&т));
            л.style_context().add_class("ev");
            л.set_xalign(0.0);
            л.set_ellipsize(gtk::pango::EllipsizeMode::End);
            л.set_max_width_chars(12);
            столбик.add(&л);
        }
        if сд.len() > 2 {
            let л = gtk::Label::new(Some(&format!("ещё {}", сд.len() - 2)));
            л.style_context().add_class("muted");
            л.set_xalign(0.0);
            столбик.add(&л);
        }
        б.add(&столбик);
        б.set_size_request(-1, 76);
        let к2 = к.clone();
        б.connect_clicked(move |_| {
            к2.выбран.set((год, мес, день));
            if чужой {
                к2.год.set(год);
                к2.месяц.set(мес);
            }
            рисуй(&к2);
        });
        к.сетка.attach(&б, (клетка % 7) as i32, (клетка / 7 + 1) as i32, 1, 1);
    }
    к.сетка.show_all();
    бок(к);
}

/// Справа — события выбранного дня: поменять, удалить, добавить
fn бок(к: &Rc<Кал>) {
    for р in к.бок.children() {
        к.бок.remove(&р);
    }
    let (г, м, д) = к.выбран.get();
    let ключ_дня = ключ(г, м, д);
    let т = gtk::Label::new(Some(&format!("{д} {}", МЕСЯЦЫ_Р[(м - 1) as usize])));
    т.style_context().add_class("side-title");
    т.set_xalign(0.0);
    к.бок.add(&т);
    let все = читай_данные(ФАЙЛ);
    let список = события(&все, &ключ_дня);
    if список.is_empty() {
        let п = gtk::Label::new(Some("Событий нет"));
        п.style_context().add_class("muted");
        п.set_xalign(0.0);
        к.бок.add(&п);
    }
    for (i, е) in список.iter().enumerate() {
        let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let время = gtk::Entry::new();
        время.set_text(е["time"].as_str().unwrap_or(""));
        время.set_placeholder_text(Some("чч:мм"));
        время.set_width_chars(5);
        время.set_max_length(5);
        let текст = gtk::Entry::new();
        текст.set_text(е["t"].as_str().unwrap_or(""));
        текст.set_hexpand(true);
        let удалить = кнопка("×", "Удалить событие");
        ряд.add(&время);
        ряд.add(&текст);
        ряд.add(&удалить);
        к.бок.add(&ряд);
        let сохрани = {
            let (ключ_дня, время, текст) = (ключ_дня.clone(), время.clone(), текст.clone());
            move || {
                let mut все = читай_данные(ФАЙЛ);
                if !все.is_object() {
                    все = json!({});
                }
                let mut с = события(&все, &ключ_дня);
                if let Some(е) = с.get_mut(i) {
                    е["t"] = json!(текст.text().trim());
                    let в = время.text().trim().to_string();
                    е["time"] = json!(if время_годно(&в) { в } else { String::new() });
                    // Изменилось — напомнить заново
                    е["done"] = json!(false);
                }
                все[&ключ_дня] = Value::Array(с);
                пиши_данные(ФАЙЛ, &все);
            }
        };
        {
            let (с, к2) = (сохрани.clone(), к.clone());
            текст.connect_activate(move |_| {
                с();
                рисуй(&к2);
            });
            let с = сохрани.clone();
            текст.connect_focus_out_event(move |_, _| {
                с();
                glib::Propagation::Proceed
            });
        }
        {
            let (с, к2) = (сохрани.clone(), к.clone());
            время.connect_activate(move |_| {
                с();
                рисуй(&к2);
            });
            время.connect_focus_out_event(move |_, _| {
                сохрани();
                glib::Propagation::Proceed
            });
        }
        let (к2, ключ_дня) = (к.clone(), ключ_дня.clone());
        удалить.connect_clicked(move |_| {
            let mut все = читай_данные(ФАЙЛ);
            let mut с = события(&все, &ключ_дня);
            if i < с.len() {
                с.remove(i);
            }
            if с.is_empty() {
                if let Some(о) = все.as_object_mut() {
                    о.remove(&ключ_дня);
                }
            } else {
                все[&ключ_дня] = Value::Array(с);
            }
            пиши_данные(ФАЙЛ, &все);
            рисуй(&к2);
        });
    }
    let добавить = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    добавить.set_margin_top(10);
    let время = gtk::Entry::new();
    время.set_placeholder_text(Some("чч:мм"));
    время.set_width_chars(5);
    время.set_max_length(5);
    let текст = gtk::Entry::new();
    текст.set_placeholder_text(Some("Новое событие"));
    текст.set_hexpand(true);
    let плюс = кнопка("＋", "Добавить событие");
    плюс.style_context().add_class("accent");
    добавить.add(&время);
    добавить.add(&текст);
    добавить.add(&плюс);
    к.бок.add(&добавить);
    let подсказка = gtk::Label::new(Some("Со временем — система напомнит в срок."));
    подсказка.style_context().add_class("muted");
    подсказка.set_xalign(0.0);
    подсказка.set_line_wrap(true);
    к.бок.add(&подсказка);
    let добавь = {
        let (к2, время, текст) = (к.clone(), время.clone(), текст.clone());
        Rc::new(move || {
            let т = текст.text().trim().to_string();
            if т.is_empty() {
                return;
            }
            let в = время.text().trim().to_string();
            let mut все = читай_данные(ФАЙЛ);
            if !все.is_object() {
                все = json!({});
            }
            let mut с = события(&все, &ключ_дня);
            с.push(json!({ "t": т, "time": if время_годно(&в) { в } else { String::new() } }));
            все[&ключ_дня] = Value::Array(с);
            пиши_данные(ФАЙЛ, &все);
            рисуй(&к2);
        })
    };
    {
        let д = добавь.clone();
        плюс.connect_clicked(move |_| д());
    }
    текст.connect_activate(move |_| добавь());
    к.бок.show_all();
}

pub fn время_годно(в: &str) -> bool {
    let mut ч = в.split(':');
    match (ч.next().and_then(|х| х.parse::<u32>().ok()), ч.next().and_then(|х| х.parse::<u32>().ok()), ч.next()) {
        (Some(h), Some(m), None) => h < 24 && m < 60,
        _ => false,
    }
}

#[cfg(test)]
mod проверки {
    use super::*;

    #[test]
    fn месяцы_и_дни() {
        assert_eq!(дней_в_месяце(2024, 2), 29);
        assert_eq!(дней_в_месяце(2026, 2), 28);
        assert_eq!(дней_в_месяце(2026, 10), 31);
        // 1 октября 2026 — четверг
        assert_eq!(сдвиг_месяца(2026, 10), 3);
        assert_eq!(ключ(2026, 3, 7), "2026-03-07");
    }

    #[test]
    fn время_проверяется() {
        assert!(время_годно("09:30") && время_годно("23:59"));
        assert!(!время_годно("24:00") && !время_годно("9") && !время_годно("ab:cd"));
    }
}
