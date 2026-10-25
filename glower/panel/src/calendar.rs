//! Календарь — месяц сеткой и события по дням (glower-calendar).
//!
//! События лежат в ~/.config/glower/calendar.json: «2026-10-23» → список
//! {t, time, end, repeat, remind, note}. Ключ — день начала; событие с
//! повтором (day, week, month, year) показывается и во все следующие дни,
//! на которые выпадает. У события со временем рабочий стол сам напомнит —
//! в срок или заранее (remind — за сколько минут; −1 — не напоминать),
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
.calendar button.event { padding: 8px 10px; border-radius: 10px; background-color: alpha(@glower_fg, 0.05); }
.calendar button.event:hover { background-color: alpha(@glower_fg, 0.10); }
.calendar .ev-time { font-weight: bold; min-width: 44px; }
.calendar .ev-sub { font-size: 11px; opacity: 0.6; }
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

/// Повторы: ключ в файле и как сказать человеку
pub const ПОВТОРЫ: [(&str, &str); 5] = [("", "Не повторять"), ("day", "Каждый день"), ("week", "Каждую неделю"), ("month", "Каждый месяц"), ("year", "Каждый год")];
/// Напоминания: за сколько минут (−1 — не напоминать)
pub const НАПОМИНАНИЯ: [(i64, &str); 7] = [(-1, "Не напоминать"), (0, "В момент начала"), (5, "За 5 минут"), (15, "За 15 минут"), (30, "За 30 минут"), (60, "За час"), (1440, "За день")];

/// «2026-10-23» → (2026, 10, 23)
pub fn разбери(ключ: &str) -> Option<(i32, u32, u32)> {
    let mut ч = ключ.split('-');
    let г = ч.next()?.parse().ok()?;
    let м: u32 = ч.next()?.parse().ok()?;
    let д: u32 = ч.next()?.parse().ok()?;
    ((1..=12).contains(&м) && (1..=31).contains(&д)).then_some((г, м, д))
}

fn номер_дня(г: i32, м: u32, д: u32) -> i64 {
    glib::DateTime::from_utc(г, м as i32, д as i32, 12, 0, 0.0).map(|т| т.to_unix() / 86400).unwrap_or(0)
}

/// Выпадает ли событие, начатое в день «начало», на день «день»
pub fn бывает(начало: &str, е: &Value, день: &str) -> bool {
    if начало == день {
        return true;
    }
    let (Some((г0, м0, д0)), Some((г, м, д))) = (разбери(начало), разбери(день)) else { return false };
    if (г, м, д) < (г0, м0, д0) {
        return false;
    }
    // В коротком месяце событие 31-го числа приходится на последний день
    let число = |г: i32, м: u32| д0.min(дней_в_месяце(г, м));
    match е["repeat"].as_str().unwrap_or("") {
        "day" => true,
        "week" => (номер_дня(г, м, д) - номер_дня(г0, м0, д0)) % 7 == 0,
        "month" => д == число(г, м),
        "year" => м == м0 && д == число(г, м),
        _ => false,
    }
}

/// События дня по порядку времени — и свои, и повторяющиеся с прошлых
/// дней. У каждого — откуда оно: «_из» (день начала) и «_н» (номер в нём),
/// чтобы его можно было поправить или удалить
pub fn события(все: &Value, к: &str) -> Vec<Value> {
    let mut с: Vec<Value> = Vec::new();
    if let Some(дни) = все.as_object() {
        for (начало, список) in дни {
            let Some(список) = список.as_array() else { continue };
            for (н, е) in список.iter().enumerate() {
                let своё = начало == к;
                let повтор = !е["repeat"].as_str().unwrap_or("").is_empty();
                if своё || (повтор && бывает(начало, е, к)) {
                    let mut е = е.clone();
                    е["_из"] = json!(начало);
                    е["_н"] = json!(н);
                    с.push(е);
                }
            }
        }
    }
    с.sort_by_key(|е| е["time"].as_str().filter(|т| !т.is_empty()).unwrap_or("").to_string());
    с
}

/// Время события словами: «весь день», «09:30», «09:30–10:15»
pub fn когда(е: &Value) -> String {
    let начало = е["time"].as_str().unwrap_or("");
    let конец = е["end"].as_str().unwrap_or("");
    match (начало.is_empty(), конец.is_empty()) {
        (true, _) => "Весь день".into(),
        (false, true) => начало.to_string(),
        (false, false) => format!("{начало}–{конец}"),
    }
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
            let т = if е["repeat"].as_str().unwrap_or("").is_empty() { т } else { format!("🔁 {т}") };
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

/// Справа — события выбранного дня. Щелчок по событию открывает его
/// целиком, «＋ Событие» — новое; строка внизу — быстро, одним названием
fn бок(к: &Rc<Кал>) {
    for р in к.бок.children() {
        к.бок.remove(&р);
    }
    let (г, м, д) = к.выбран.get();
    let ключ_дня = ключ(г, м, д);
    let шапка = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let т = gtk::Label::new(Some(&format!("{д} {}", МЕСЯЦЫ_Р[(м - 1) as usize])));
    т.style_context().add_class("side-title");
    т.set_xalign(0.0);
    т.set_hexpand(true);
    шапка.add(&т);
    let новое = кнопка("＋ Событие", "Новое событие с датой, временем и напоминанием");
    новое.style_context().add_class("accent");
    {
        let к2 = к.clone();
        новое.connect_clicked(move |_| окно_события(&к2, None));
    }
    шапка.add(&новое);
    к.бок.add(&шапка);
    let все = читай_данные(ФАЙЛ);
    let список = события(&все, &ключ_дня);
    if список.is_empty() {
        let п = gtk::Label::new(Some("Событий нет"));
        п.style_context().add_class("muted");
        п.set_xalign(0.0);
        к.бок.add(&п);
    }
    for е in список {
        let б = gtk::Button::new();
        б.style_context().add_class("event");
        let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let в = gtk::Label::new(Some(&когда(&е)));
        в.style_context().add_class("ev-time");
        в.set_xalign(0.0);
        ряд.add(&в);
        let столбик = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let н = gtk::Label::new(Some(е["t"].as_str().unwrap_or("")));
        н.set_xalign(0.0);
        н.set_ellipsize(gtk::pango::EllipsizeMode::End);
        столбик.add(&н);
        let mut подробно: Vec<String> = Vec::new();
        if let Some((_, имя)) = ПОВТОРЫ.iter().find(|(к, _)| !к.is_empty() && е["repeat"].as_str() == Some(к)) {
            подробно.push(format!("🔁 {имя}"));
        }
        if !е["time"].as_str().unwrap_or("").is_empty() {
            let за = е["remind"].as_i64().unwrap_or(0);
            if let Some((_, имя)) = НАПОМИНАНИЯ.iter().find(|(м, _)| *м == за) {
                if за >= 0 {
                    подробно.push(format!("🔔 {}", имя.to_lowercase()));
                }
            }
        }
        if !подробно.is_empty() {
            let п = gtk::Label::new(Some(&подробно.join(" · ")));
            п.style_context().add_class("ev-sub");
            п.set_xalign(0.0);
            столбик.add(&п);
        }
        столбик.set_hexpand(true);
        ряд.add(&столбик);
        б.add(&ряд);
        let к2 = к.clone();
        let откуда = (е["_из"].as_str().unwrap_or("").to_string(), е["_н"].as_u64().unwrap_or(0) as usize);
        б.connect_clicked(move |_| окно_события(&к2, Some(откуда.clone())));
        к.бок.add(&б);
    }
    let добавить = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    добавить.set_margin_top(10);
    let текст = gtk::Entry::new();
    текст.set_placeholder_text(Some("Быстро: название и Enter"));
    текст.set_hexpand(true);
    добавить.add(&текст);
    к.бок.add(&добавить);
    let к2 = к.clone();
    текст.connect_activate(move |т| {
        let название = т.text().trim().to_string();
        if название.is_empty() {
            return;
        }
        let mut все = читай_данные(ФАЙЛ);
        if !все.is_object() {
            все = json!({});
        }
        let mut с: Vec<Value> = все[&ключ_дня].as_array().cloned().unwrap_or_default();
        с.push(json!({ "t": название, "time": "" }));
        все[&ключ_дня] = Value::Array(с);
        пиши_данные(ФАЙЛ, &все);
        рисуй(&к2);
    });
    к.бок.show_all();
}

/// Окно события: название, дата (календарь), время начала и конца или
/// «весь день», повтор, напоминание и заметка. «откуда» — день начала и
/// номер события в нём; None — новое на выбранный день
fn окно_события(к: &Rc<Кал>, откуда: Option<(String, usize)>) {
    let все = читай_данные(ФАЙЛ);
    let было = откуда.as_ref().and_then(|(день, н)| все[день].as_array().and_then(|с| с.get(*н)).cloned()).unwrap_or_else(|| json!({}));
    let (г, м, д) = откуда.as_ref().and_then(|(день, _)| разбери(день)).unwrap_or_else(|| к.выбран.get());
    let верх = к.бок.toplevel().and_then(|в| в.downcast::<gtk::Window>().ok());
    let окно = gtk::Dialog::with_buttons(
        Some(if откуда.is_some() { "Событие" } else { "Новое событие" }),
        верх.as_ref(),
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[("Отмена", gtk::ResponseType::Cancel), ("Сохранить", gtk::ResponseType::Accept)],
    );
    if откуда.is_some() {
        окно.add_button("Удалить", gtk::ResponseType::Reject);
    }
    окно.set_default_response(gtk::ResponseType::Accept);
    окно.style_context().add_class("glower-app");
    let с = gtk::Grid::new();
    с.set_row_spacing(10);
    с.set_column_spacing(12);
    с.set_margin(16);
    let подпись = |т: &str, ряд: i32| {
        let л = gtk::Label::new(Some(т));
        л.set_xalign(1.0);
        с.attach(&л, 0, ряд, 1, 1);
    };
    подпись("Название", 0);
    let название = gtk::Entry::new();
    название.set_text(было["t"].as_str().unwrap_or(""));
    название.set_placeholder_text(Some("Что за событие"));
    название.set_activates_default(true);
    название.set_width_chars(32);
    с.attach(&название, 1, 0, 1, 1);

    // Дата: кнопка с датой словами, по щелчку — календарь
    подпись("Дата", 1);
    let выбор = gtk::Calendar::new();
    выбор.select_month(м - 1, г as u32);
    выбор.select_day(д);
    let дата = gtk::MenuButton::new();
    let дата_текст = gtk::Label::new(None);
    let покажи_дату = {
        let (выбор, дата_текст) = (выбор.clone(), дата_текст.clone());
        move || {
            let (г, м, д) = выбор.date();
            дата_текст.set_text(&format!("{д} {} {г}", МЕСЯЦЫ_Р[м as usize]));
        }
    };
    покажи_дату();
    дата.add(&дата_текст);
    let всплывашка = gtk::Popover::new(Some(&дата));
    выбор.set_margin(8);
    всплывашка.add(&выбор);
    выбор.show();
    дата.set_popover(Some(&всплывашка));
    {
        let (п, всплывашка) = (покажи_дату.clone(), всплывашка.clone());
        выбор.connect_day_selected(move |_| п());
        выбор.connect_day_selected_double_click(move |_| всплывашка.popdown());
    }
    дата.set_halign(gtk::Align::Start);
    с.attach(&дата, 1, 1, 1, 1);

    // Время: начало, по желанию конец, или весь день
    подпись("Время", 2);
    let часы_мин = |т: &str, по_умолчанию: (u32, u32)| -> (u32, u32) {
        let mut ч = т.split(':');
        match (ч.next().and_then(|х| х.parse().ok()), ч.next().and_then(|х| х.parse().ok())) {
            (Some(h), Some(m)) if время_годно(т) => (h, m),
            _ => по_умолчанию,
        }
    };
    let поле_времени = |ч: u32, м: u32| {
        let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let часы = gtk::SpinButton::with_range(0.0, 23.0, 1.0);
        часы.set_value(ч as f64);
        часы.set_wrap(true);
        let минуты = gtk::SpinButton::with_range(0.0, 59.0, 5.0);
        минуты.set_value(м as f64);
        минуты.set_wrap(true);
        // Минуты — двумя цифрами: «09:05», а не «9:5»
        for п in [&часы, &минуты] {
            п.connect_output(|п| {
                п.set_text(&format!("{:02}", п.value() as u32));
                glib::Propagation::Stop
            });
        }
        ряд.add(&часы);
        ряд.add(&gtk::Label::new(Some(":")));
        ряд.add(&минуты);
        (ряд, часы, минуты)
    };
    let весь_день = было["time"].as_str().unwrap_or("").is_empty() && откуда.is_some();
    let (нч, нм) = часы_мин(было["time"].as_str().unwrap_or(""), (9, 0));
    let (кч, км) = часы_мин(было["end"].as_str().unwrap_or(""), ((нч + 1).min(23), нм));
    let время_ряд = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let (начало_ряд, нач_ч, нач_м) = поле_времени(нч, нм);
    let до = gtk::CheckButton::with_label("до");
    до.set_active(!было["end"].as_str().unwrap_or("").is_empty());
    let (конец_ряд, кон_ч, кон_м) = поле_времени(кч, км);
    время_ряд.add(&начало_ряд);
    время_ряд.add(&до);
    время_ряд.add(&конец_ряд);
    с.attach(&время_ряд, 1, 2, 1, 1);
    let целый = gtk::CheckButton::with_label("Весь день");
    целый.set_active(весь_день);
    с.attach(&целый, 1, 3, 1, 1);
    let обнови_время = {
        let (целый, до, начало_ряд, конец_ряд) = (целый.clone(), до.clone(), начало_ряд.clone(), конец_ряд.clone());
        move || {
            let есть_время = !целый.is_active();
            начало_ряд.set_sensitive(есть_время);
            до.set_sensitive(есть_время);
            конец_ряд.set_sensitive(есть_время && до.is_active());
        }
    };
    обнови_время();
    {
        let о = обнови_время.clone();
        целый.connect_toggled(move |_| о());
        let о = обнови_время.clone();
        до.connect_toggled(move |_| о());
    }

    подпись("Повтор", 4);
    let повтор = gtk::ComboBoxText::new();
    for (ключ, имя) in ПОВТОРЫ {
        повтор.append(Some(if ключ.is_empty() { "-" } else { ключ }), имя);
    }
    повтор.set_active_id(Some(было["repeat"].as_str().filter(|п| !п.is_empty()).unwrap_or("-")));
    с.attach(&повтор, 1, 4, 1, 1);

    подпись("Напомнить", 5);
    let напомнить = gtk::ComboBoxText::new();
    for (мин, имя) in НАПОМИНАНИЯ {
        напомнить.append(Some(&мин.to_string()), имя);
    }
    напомнить.set_active_id(Some(&было["remind"].as_i64().unwrap_or(0).to_string()));
    if напомнить.active_id().is_none() {
        напомнить.set_active_id(Some("0"));
    }
    с.attach(&напомнить, 1, 5, 1, 1);
    {
        let н = напомнить.clone();
        let обн = move |ц: &gtk::CheckButton| н.set_sensitive(!ц.is_active());
        обн(&целый);
        целый.connect_toggled(обн);
    }

    подпись("Заметка", 6);
    let заметка = gtk::TextView::new();
    заметка.set_wrap_mode(gtk::WrapMode::WordChar);
    if let Some(б) = заметка.buffer() {
        б.set_text(было["note"].as_str().unwrap_or(""));
    }
    let рамка = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    рамка.set_min_content_height(70);
    рамка.set_shadow_type(gtk::ShadowType::In);
    рамка.add(&заметка);
    с.attach(&рамка, 1, 6, 1, 1);

    окно.content_area().add(&с);
    окно.show_all();
    let к2 = к.clone();
    окно.connect_response(move |о, ответ| {
        if ответ == gtk::ResponseType::Accept && название.text().trim().is_empty() {
            название.grab_focus();
            return;
        }
        let mut все = читай_данные(ФАЙЛ);
        if !все.is_object() {
            все = json!({});
        }
        // Старое место события — убираем: дата могла поменяться
        if ответ == gtk::ResponseType::Accept || ответ == gtk::ResponseType::Reject {
            if let Some((день, н)) = откуда.as_ref() {
                if let Some(с) = все[день.as_str()].as_array_mut() {
                    if *н < с.len() {
                        с.remove(*н);
                    }
                    if с.is_empty() {
                        if let Some(о) = все.as_object_mut() {
                            о.remove(день.as_str());
                        }
                    }
                }
            }
        }
        if ответ == gtk::ResponseType::Accept {
            let (г, м, д) = выбор.date();
            let день = ключ(г as i32, м + 1, д);
            let время = |ч: &gtk::SpinButton, м: &gtk::SpinButton| format!("{:02}:{:02}", ч.value() as u32, м.value() as u32);
            let с_временем = !целый.is_active();
            let mut е = json!({
                "t": название.text().trim(),
                "time": if с_временем { время(&нач_ч, &нач_м) } else { String::new() },
                "remind": напомнить.active_id().and_then(|и| и.parse::<i64>().ok()).unwrap_or(0),
            });
            if с_временем && до.is_active() {
                е["end"] = json!(время(&кон_ч, &кон_м));
            }
            if let Some(п) = повтор.active_id().filter(|п| п.as_str() != "-") {
                е["repeat"] = json!(п.as_str());
            }
            let текст = заметка.buffer().and_then(|б| б.text(&б.start_iter(), &б.end_iter(), false)).map(|т| т.trim().to_string()).unwrap_or_default();
            if !текст.is_empty() {
                е["note"] = json!(текст);
            }
            let mut с: Vec<Value> = все[&день].as_array().cloned().unwrap_or_default();
            с.push(е);
            все[&день] = Value::Array(с);
            к2.выбран.set((г as i32, м + 1, д));
            к2.год.set(г as i32);
            к2.месяц.set(м + 1);
        }
        if ответ == gtk::ResponseType::Accept || ответ == gtk::ResponseType::Reject {
            пиши_данные(ФАЙЛ, &все);
        }
        unsafe { о.destroy() };
        рисуй(&к2);
    });
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
    fn повторы_выпадают_на_свои_дни() {
        let е = |п: &str| json!({ "t": "x", "repeat": п });
        assert!(бывает("2026-10-23", &е(""), "2026-10-23"));
        assert!(!бывает("2026-10-23", &е(""), "2026-10-24"));
        assert!(бывает("2026-10-23", &е("day"), "2026-11-02"));
        assert!(!бывает("2026-10-23", &е("day"), "2026-10-22"));
        assert!(бывает("2026-10-23", &е("week"), "2026-10-30"));
        assert!(!бывает("2026-10-23", &е("week"), "2026-10-31"));
        assert!(бывает("2026-01-31", &е("month"), "2026-02-28"));
        assert!(бывает("2026-10-23", &е("month"), "2026-12-23"));
        assert!(бывает("2024-02-29", &е("year"), "2025-02-28"));
        let все = json!({ "2026-10-01": [{ "t": "Планёрка", "time": "10:00", "repeat": "week" }], "2026-10-08": [{ "t": "Обед", "time": "13:00" }] });
        let с = события(&все, "2026-10-08");
        assert_eq!(с.len(), 2);
        assert_eq!(с[0]["t"], "Планёрка");
        assert_eq!(с[0]["_из"], "2026-10-01");
        assert_eq!(когда(&json!({ "time": "09:30", "end": "10:15" })), "09:30–10:15");
        assert_eq!(когда(&json!({ "time": "" })), "Весь день");
    }

    #[test]
    fn время_проверяется() {
        assert!(время_годно("09:30") && время_годно("23:59"));
        assert!(!время_годно("24:00") && !время_годно("9") && !время_годно("ab:cd"));
    }
}
