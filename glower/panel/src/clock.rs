//! Часы — циферблат, будильники, секундомер, таймер и мировое время
//! (glower-clock).
//!
//! Будильники лежат в ~/.config/glower/clock.json, и звонит их рабочий
//! стол: Часы можно закрыть, а будильник всё равно сработает.

use crate::agent::скажи;
use crate::apps::{кнопка, подготовь, пиши_данные, читай_данные};
use gtk::cairo;
use gtk::glib;
use gtk::prelude::*;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::f64::consts::PI;
use std::rc::Rc;
use std::time::{Duration, Instant};

const СТИЛЬ: &str = r#"
.clock .big { font-size: 46px; font-weight: 300; font-feature-settings: "tnum"; }
.clock .sub { opacity: 0.6; }
.clock stackswitcher button { padding: 6px 16px; }
.clock .row { padding: 10px 12px; border-radius: 10px; background-color: alpha(@glower_fg, 0.05); }
.clock .row-time { font-size: 24px; font-weight: 300; }
.clock button.day { min-width: 30px; padding: 2px 4px; border-radius: 8px; }
.clock button.day.on { background-color: alpha(@glower_accent, 0.5); }
"#;

pub const ФАЙЛ: &str = "clock.json";
pub const ДНИ: [&str; 7] = ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"];

/// Города мира: имя и пояс IANA
const ГОРОДА: &[(&str, &str)] = &[
    ("Москва", "Europe/Moscow"),
    ("Лондон", "Europe/London"),
    ("Берлин", "Europe/Berlin"),
    ("Нью-Йорк", "America/New_York"),
    ("Сан-Франциско", "America/Los_Angeles"),
    ("Дубай", "Asia/Dubai"),
    ("Токио", "Asia/Tokyo"),
    ("Пекин", "Asia/Shanghai"),
    ("Сидней", "Australia/Sydney"),
    ("Новосибирск", "Asia/Novosibirsk"),
    ("Владивосток", "Asia/Vladivostok"),
    ("Киев", "Europe/Kyiv"),
    ("Алматы", "Asia/Almaty"),
    ("Минск", "Europe/Minsk"),
];

fn данные() -> Value {
    let д = читай_данные(ФАЙЛ);
    if д.is_object() { д } else { json!({ "alarms": [], "world": ["Europe/London", "America/New_York", "Asia/Tokyo"] }) }
}

pub fn мс_строкой(мс: u128) -> String {
    format!("{:02}:{:02}.{:02}", мс / 60000, (мс / 1000) % 60, (мс / 10) % 100)
}

pub fn сек_строкой(с: u64) -> String {
    if с >= 3600 { format!("{}:{:02}:{:02}", с / 3600, (с / 60) % 60, с % 60) } else { format!("{:02}:{:02}", с / 60, с % 60) }
}

pub fn запусти(_доводы: Vec<String>) -> ! {
    подготовь("glower-clock", "Часы");
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль часов");
    gtk::StyleContext::add_provider_for_screen(&gtk::gdk::Screen::default().expect("экран"), &стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_title("Часы");
    окно.set_default_size(560, 600);
    окно.set_icon_name(Some("alarm-symbolic"));
    окно.style_context().add_class("glower-app");
    окно.style_context().add_class("clock");
    окно.connect_destroy(|_| gtk::main_quit());

    let колонка = gtk::Box::new(gtk::Orientation::Vertical, 0);
    окно.add(&колонка);
    let стопка = gtk::Stack::new();
    стопка.set_transition_type(gtk::StackTransitionType::Crossfade);
    let переключатель = gtk::StackSwitcher::new();
    переключатель.set_stack(Some(&стопка));
    переключатель.set_halign(gtk::Align::Center);
    переключатель.set_margin(10);
    колонка.add(&переключатель);
    стопка.set_vexpand(true);
    колонка.add(&стопка);

    стопка.add_titled(&циферблат(), "clock", "Часы");
    стопка.add_titled(&будильники(&окно), "alarm", "Будильник");
    стопка.add_titled(&секундомер(), "sw", "Секундомер");
    стопка.add_titled(&таймер(), "timer", "Таймер");
    стопка.add_titled(&мир(&окно), "world", "Мир");

    окно.show_all();
    gtk::main();
    std::process::exit(0);
}

fn циферблат() -> gtk::Widget {
    let к = gtk::Box::new(gtk::Orientation::Vertical, 8);
    к.set_margin(16);
    let холст = gtk::DrawingArea::new();
    холст.set_size_request(280, 280);
    холст.set_halign(gtk::Align::Center);
    холст.connect_draw(|в, с| {
        рисуй_циферблат(с, в.allocated_width() as f64, в.allocated_height() as f64, в);
        glib::Propagation::Stop
    });
    к.add(&холст);
    let цифры = gtk::Label::new(None);
    цифры.style_context().add_class("big");
    к.add(&цифры);
    let дата = gtk::Label::new(None);
    дата.style_context().add_class("sub");
    к.add(&дата);
    let обнови = move || {
        let н = glib::DateTime::now_local().expect("время");
        цифры.set_text(&н.format("%H:%M:%S").map(|т| т.to_string()).unwrap_or_default());
        let (_, д) = crate::menus::время_и_дата();
        let день = crate::menus::ДНИ[(н.day_of_week() as usize).clamp(1, 7) - 1];
        дата.set_text(&format!("{день}, {д} {}", н.year()));
        холст.queue_draw();
    };
    обнови();
    glib::timeout_add_local(Duration::from_millis(250), move || {
        обнови();
        glib::ControlFlow::Continue
    });
    к.upcast()
}

fn рисуй_циферблат(с: &cairo::Context, ш: f64, в: f64, виджет: &gtk::DrawingArea) {
    let цвет = виджет.style_context().color(gtk::StateFlags::NORMAL);
    let (r, g, b) = (цвет.red(), цвет.green(), цвет.blue());
    let р = ш.min(в) / 2.0 - 6.0;
    с.translate(ш / 2.0, в / 2.0);
    с.set_source_rgba(r, g, b, 0.06);
    с.arc(0.0, 0.0, р, 0.0, 2.0 * PI);
    let _ = с.fill();
    for i in 0..60 {
        let а = i as f64 * PI / 30.0;
        let (д, т) = if i % 5 == 0 { (р * 0.86, 3.0) } else { (р * 0.93, 1.0) };
        с.set_source_rgba(r, g, b, if i % 5 == 0 { 0.8 } else { 0.35 });
        с.set_line_width(т);
        с.move_to(д * а.sin(), -д * а.cos());
        с.line_to(р * 0.98 * а.sin(), -р * 0.98 * а.cos());
        let _ = с.stroke();
    }
    let н = glib::DateTime::now_local().expect("время");
    let (ч, м, сек) = (н.hour() as f64 % 12.0, н.minute() as f64, н.seconds());
    let стрелка = |угол: f64, длина: f64, толщина: f64, rgba: (f64, f64, f64, f64)| {
        с.set_source_rgba(rgba.0, rgba.1, rgba.2, rgba.3);
        с.set_line_width(толщина);
        с.set_line_cap(cairo::LineCap::Round);
        с.move_to(-0.12 * длина * угол.sin(), 0.12 * длина * угол.cos());
        с.line_to(длина * угол.sin(), -длина * угол.cos());
        let _ = с.stroke();
    };
    стрелка((ч + м / 60.0) * PI / 6.0, р * 0.5, 6.0, (r, g, b, 0.95));
    стрелка((м + сек / 60.0) * PI / 30.0, р * 0.75, 4.0, (r, g, b, 0.95));
    стрелка(сек * PI / 30.0, р * 0.85, 1.6, (0.94, 0.27, 0.27, 1.0));
    с.set_source_rgb(0.94, 0.27, 0.27);
    с.arc(0.0, 0.0, 4.5, 0.0, 2.0 * PI);
    let _ = с.fill();
}

fn будильники(окно: &gtk::Window) -> gtk::Widget {
    let к = gtk::Box::new(gtk::Orientation::Vertical, 8);
    к.set_margin(16);
    let список = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let листание = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    листание.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    листание.set_vexpand(true);
    листание.add(&список);
    к.add(&листание);
    let добавить = кнопка("＋ Новый будильник", "Добавить будильник");
    добавить.style_context().add_class("accent");
    добавить.set_halign(gtk::Align::Center);
    к.add(&добавить);
    let заметка = gtk::Label::new(Some("Будильник звонит, даже если Часы закрыты."));
    заметка.style_context().add_class("muted");
    к.add(&заметка);

    let рисуй: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let р2 = рисуй.clone();
    *рисуй.borrow_mut() = Some(Box::new(move || {
        for р in список.children() {
            список.remove(&р);
        }
        let д = данные();
        let все = д["alarms"].as_array().cloned().unwrap_or_default();
        if все.is_empty() {
            let п = gtk::Label::new(Some("Будильников нет"));
            п.style_context().add_class("muted");
            п.set_margin_top(30);
            список.add(&п);
        }
        for (i, б) in все.iter().enumerate() {
            let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            ряд.style_context().add_class("row");
            let с = gtk::Box::new(gtk::Orientation::Vertical, 4);
            let время = gtk::Label::new(Some(б["time"].as_str().unwrap_or("07:00")));
            время.style_context().add_class("row-time");
            время.set_xalign(0.0);
            с.add(&время);
            let подпись = gtk::Entry::new();
            подпись.set_text(б["label"].as_str().unwrap_or(""));
            подпись.set_placeholder_text(Some("Подпись"));
            с.add(&подпись);
            let дни = gtk::Box::new(gtk::Orientation::Horizontal, 3);
            let выбраны: Vec<u64> = б["days"].as_array().map(|а| а.iter().filter_map(|х| х.as_u64()).collect()).unwrap_or_default();
            for (н, имя) in ДНИ.iter().enumerate() {
                let кн = gtk::Button::with_label(имя);
                кн.style_context().add_class("day");
                if выбраны.contains(&(н as u64)) {
                    кн.style_context().add_class("on");
                }
                let р3 = р2.clone();
                кн.connect_clicked(move |_| {
                    правь(i, |б| {
                        let mut д: Vec<u64> = б["days"].as_array().map(|а| а.iter().filter_map(|х| х.as_u64()).collect()).unwrap_or_default();
                        if let Some(п) = д.iter().position(|х| *х == н as u64) {
                            д.remove(п);
                        } else {
                            д.push(н as u64);
                            д.sort();
                        }
                        б["days"] = json!(д);
                    });
                    if let Some(р) = р3.borrow().as_ref() {
                        р();
                    }
                });
                дни.add(&кн);
            }
            с.add(&дни);
            с.set_hexpand(true);
            ряд.add(&с);
            let вкл = gtk::Switch::new();
            вкл.set_active(б["on"].as_bool() != Some(false));
            вкл.set_valign(gtk::Align::Center);
            вкл.connect_active_notify(move |в| {
                let а = в.is_active();
                правь(i, |б| б["on"] = json!(а));
            });
            ряд.add(&вкл);
            let удалить = кнопка("×", "Удалить будильник");
            удалить.set_valign(gtk::Align::Center);
            let р3 = р2.clone();
            удалить.connect_clicked(move |_| {
                let mut д = данные();
                if let Some(а) = д["alarms"].as_array_mut() {
                    if i < а.len() {
                        а.remove(i);
                    }
                }
                пиши_данные(ФАЙЛ, &д);
                if let Some(р) = р3.borrow().as_ref() {
                    р();
                }
            });
            ряд.add(&удалить);
            подпись.connect_changed(move |п| {
                let т = п.text().to_string();
                правь(i, |б| б["label"] = json!(т));
            });
            список.add(&ряд);
        }
        список.show_all();
    }));
    if let Some(р) = рисуй.borrow().as_ref() {
        р();
    }
    let окно = окно.clone();
    добавить.connect_clicked(move |_| {
        if let Some(время) = спроси_время(&окно) {
            let mut д = данные();
            if !д["alarms"].is_array() {
                д["alarms"] = json!([]);
            }
            if let Some(а) = д["alarms"].as_array_mut() {
                а.push(json!({ "time": время, "label": "", "days": [], "on": true }));
            }
            пиши_данные(ФАЙЛ, &д);
            if let Some(р) = рисуй.borrow().as_ref() {
                р();
            }
        }
    });
    к.upcast()
}

fn правь(i: usize, дело: impl FnOnce(&mut Value)) {
    let mut д = данные();
    if let Some(б) = д["alarms"].as_array_mut().and_then(|а| а.get_mut(i)) {
        дело(б);
        // Изменили — пусть сработает в новый срок, даже если сегодня уже звонил
        б["last"] = Value::Null;
    }
    пиши_данные(ФАЙЛ, &д);
}

fn спроси_время(окно: &gtk::Window) -> Option<String> {
    let д = gtk::Dialog::with_buttons(Some("Новый будильник"), Some(окно), gtk::DialogFlags::MODAL, &[("Отмена", gtk::ResponseType::Cancel), ("Готово", gtk::ResponseType::Accept)]);
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    ряд.set_margin(16);
    ряд.set_halign(gtk::Align::Center);
    let ч = gtk::SpinButton::with_range(0.0, 23.0, 1.0);
    ч.set_value(7.0);
    let м = gtk::SpinButton::with_range(0.0, 59.0, 5.0);
    м.set_value(0.0);
    for с in [&ч, &м] {
        с.set_wrap(true);
        с.connect_output(|с| {
            с.set_text(&format!("{:02}", с.value() as i64));
            glib::Propagation::Stop
        });
    }
    ряд.add(&ч);
    ряд.add(&gtk::Label::new(Some(":")));
    ряд.add(&м);
    д.content_area().add(&ряд);
    д.show_all();
    let ответ = д.run();
    let время = format!("{:02}:{:02}", ч.value() as i64, м.value() as i64);
    unsafe { д.destroy() };
    (ответ == gtk::ResponseType::Accept).then_some(время)
}

fn секундомер() -> gtk::Widget {
    let к = gtk::Box::new(gtk::Orientation::Vertical, 14);
    к.set_margin(24);
    let цифры = gtk::Label::new(Some("00:00.00"));
    цифры.style_context().add_class("big");
    к.add(&цифры);
    let кнопки = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    кнопки.set_halign(gtk::Align::Center);
    let пуск = кнопка("Старт", "Старт / пауза");
    пуск.style_context().add_class("accent");
    let круг = кнопка("Круг", "Отметить круг");
    let сброс = кнопка("Сброс", "Сбросить");
    кнопки.add(&пуск);
    кнопки.add(&круг);
    кнопки.add(&сброс);
    к.add(&кнопки);
    let круги = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let листание = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    листание.set_vexpand(true);
    листание.add(&круги);
    к.add(&листание);

    // Накоплено до последней паузы и когда пошли снова
    let накоплено = Rc::new(Cell::new(Duration::ZERO));
    let с_момента: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));
    let сколько = {
        let (н, с) = (накоплено.clone(), с_момента.clone());
        move || н.get() + с.get().map(|м| м.elapsed()).unwrap_or_default()
    };
    {
        let (цифры, сколько) = (цифры.clone(), сколько.clone());
        glib::timeout_add_local(Duration::from_millis(31), move || {
            цифры.set_text(&мс_строкой(сколько().as_millis()));
            glib::ControlFlow::Continue
        });
    }
    {
        let (н, с) = (накоплено.clone(), с_момента.clone());
        пуск.connect_clicked(move |б| match с.get() {
            Some(м) => {
                н.set(н.get() + м.elapsed());
                с.set(None);
                б.set_label("Старт");
            }
            None => {
                с.set(Some(Instant::now()));
                б.set_label("Пауза");
            }
        });
    }
    {
        let (круги, сколько) = (круги.clone(), сколько.clone());
        круг.connect_clicked(move |_| {
            let n = круги.children().len() + 1;
            let л = gtk::Label::new(Some(&format!("Круг {n}    {}", мс_строкой(сколько().as_millis()))));
            л.set_xalign(0.5);
            круги.pack_end(&л, false, false, 0);
            л.show();
        });
    }
    {
        let (н, с, пуск) = (накоплено.clone(), с_момента.clone(), пуск.clone());
        сброс.connect_clicked(move |_| {
            н.set(Duration::ZERO);
            с.set(None);
            пуск.set_label("Старт");
            for р in круги.children() {
                круги.remove(&р);
            }
        });
    }
    к.upcast()
}

fn таймер() -> gtk::Widget {
    let к = gtk::Box::new(gtk::Orientation::Vertical, 14);
    к.set_margin(24);
    let цифры = gtk::Label::new(Some("05:00"));
    цифры.style_context().add_class("big");
    к.add(&цифры);
    let готовые = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    готовые.set_halign(gtk::Align::Center);
    к.add(&готовые);
    let своё = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    своё.set_halign(gtk::Align::Center);
    let мин = gtk::SpinButton::with_range(0.0, 600.0, 1.0);
    мин.set_value(5.0);
    своё.add(&мин);
    своё.add(&gtk::Label::new(Some("мин")));
    к.add(&своё);
    let кнопки = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    кнопки.set_halign(gtk::Align::Center);
    let пуск = кнопка("Старт", "Старт / пауза");
    пуск.style_context().add_class("accent");
    let сброс = кнопка("Сброс", "Сбросить");
    кнопки.add(&пуск);
    кнопки.add(&сброс);
    к.add(&кнопки);

    let всего = Rc::new(Cell::new(Duration::from_secs(300)));
    let осталось_до_паузы = Rc::new(Cell::new(Duration::from_secs(300)));
    let пошёл: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));
    let осталось = {
        let (о, п) = (осталось_до_паузы.clone(), пошёл.clone());
        move || о.get().saturating_sub(п.get().map(|м| м.elapsed()).unwrap_or_default())
    };
    for минут in [1u64, 3, 5, 10, 25] {
        let б = кнопка(&format!("{минут} мин"), "");
        let (в, о, п, м, пуск) = (всего.clone(), осталось_до_паузы.clone(), пошёл.clone(), мин.clone(), пуск.clone());
        б.connect_clicked(move |_| {
            в.set(Duration::from_secs(минут * 60));
            о.set(Duration::from_secs(минут * 60));
            п.set(None);
            м.set_value(минут as f64);
            пуск.set_label("Старт");
        });
        готовые.add(&б);
    }
    {
        let (в, о, п) = (всего.clone(), осталось_до_паузы.clone(), пошёл.clone());
        мин.connect_value_changed(move |м| {
            if п.get().is_none() {
                в.set(Duration::from_secs(м.value() as u64 * 60));
                о.set(в.get());
            }
        });
    }
    {
        let (о, п) = (осталось_до_паузы.clone(), пошёл.clone());
        пуск.connect_clicked(move |б| match п.get() {
            Some(м) => {
                о.set(о.get().saturating_sub(м.elapsed()));
                п.set(None);
                б.set_label("Старт");
            }
            None if !о.get().is_zero() => {
                п.set(Some(Instant::now()));
                б.set_label("Пауза");
            }
            None => {}
        });
    }
    {
        let (в, о, п, пуск) = (всего.clone(), осталось_до_паузы.clone(), пошёл.clone(), пуск.clone());
        сброс.connect_clicked(move |_| {
            о.set(в.get());
            п.set(None);
            пуск.set_label("Старт");
        });
    }
    {
        let (о, п, пуск) = (осталось_до_паузы.clone(), пошёл.clone(), пуск.clone());
        glib::timeout_add_local(Duration::from_millis(200), move || {
            let с = осталось();
            цифры.set_text(&сек_строкой((с.as_millis() as u64 + 999) / 1000));
            if п.get().is_some() && с.is_zero() {
                п.set(None);
                о.set(Duration::ZERO);
                пуск.set_label("Старт");
                скажи("уведомление", json!({ "заголовок": "Таймер", "текст": "Время вышло", "значок": "⏰", "откуда": "Часы", "звук": true }));
            }
            glib::ControlFlow::Continue
        });
    }
    к.upcast()
}

fn мир(окно: &gtk::Window) -> gtk::Widget {
    let к = gtk::Box::new(gtk::Orientation::Vertical, 8);
    к.set_margin(16);
    let список = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let листание = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    листание.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    листание.set_vexpand(true);
    листание.add(&список);
    к.add(&листание);
    let добавить = кнопка("＋ Город", "Добавить город");
    добавить.set_halign(gtk::Align::Center);
    к.add(&добавить);

    let рисуй: Rc<dyn Fn()> = {
        let список = список.clone();
        Rc::new(move || {
            for р in список.children() {
                список.remove(&р);
            }
            let здесь = glib::DateTime::now_local().expect("время");
            for пояс in данные()["world"].as_array().cloned().unwrap_or_default() {
                let Some(пояс) = пояс.as_str().map(String::from) else { continue };
                let Ok(тз) = glib::TimeZone::from_identifier(Some(&пояс)).ok_or(()) else { continue };
                let Ok(там) = glib::DateTime::now(&тз) else { continue };
                let имя = ГОРОДА.iter().find(|(_, п)| *п == пояс).map(|(и, _)| и.to_string()).unwrap_or_else(|| пояс.rsplit('/').next().unwrap_or(&пояс).replace('_', " "));
                let разница = (там.utc_offset().as_seconds() - здесь.utc_offset().as_seconds()) / 3600;
                let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                ряд.style_context().add_class("row");
                let с = gtk::Box::new(gtk::Orientation::Vertical, 2);
                let и = gtk::Label::new(Some(&имя));
                и.set_xalign(0.0);
                с.add(&и);
                let р = gtk::Label::new(Some(&match разница {
                    0 => "как здесь".to_string(),
                    р if р > 0 => format!("на {р} ч впереди"),
                    р => format!("на {} ч позади", -р),
                }));
                р.style_context().add_class("muted");
                р.set_xalign(0.0);
                с.add(&р);
                с.set_hexpand(true);
                ряд.add(&с);
                let т = gtk::Label::new(Some(&там.format("%H:%M").map(|т| т.to_string()).unwrap_or_default()));
                т.style_context().add_class("row-time");
                ряд.add(&т);
                let у = кнопка("×", "Убрать город");
                у.set_valign(gtk::Align::Center);
                let (список2, пояс2) = (список.clone(), пояс.clone());
                у.connect_clicked(move |_| {
                    let mut д = данные();
                    if let Some(а) = д["world"].as_array_mut() {
                        а.retain(|х| х.as_str() != Some(&пояс2));
                    }
                    пиши_данные(ФАЙЛ, &д);
                    // перерисует таймер ниже
                    for р in список2.children() {
                        р.set_sensitive(false);
                    }
                });
                ряд.add(&у);
                список.add(&ряд);
            }
            список.show_all();
        })
    };
    рисуй();
    {
        let р = рисуй.clone();
        glib::timeout_add_local(Duration::from_secs(1), move || {
            // раз в секунду: и время идёт, и убранный город пропадает
            let н = glib::DateTime::now_local().map(|н| н.second()).unwrap_or(0);
            if н == 0 || список.children().iter().any(|р| !р.is_sensitive()) {
                р();
            }
            glib::ControlFlow::Continue
        });
    }
    let окно = окно.clone();
    добавить.connect_clicked(move |_| {
        let д = gtk::Dialog::with_buttons(Some("Добавить город"), Some(&окно), gtk::DialogFlags::MODAL, &[("Отмена", gtk::ResponseType::Cancel), ("Добавить", gtk::ResponseType::Accept)]);
        let выбор = gtk::ComboBoxText::new();
        for (имя, пояс) in ГОРОДА {
            выбор.append(Some(пояс), &crate::i18n::tr(имя));
        }
        выбор.set_active(Some(0));
        выбор.set_margin(14);
        д.content_area().add(&выбор);
        д.show_all();
        if д.run() == gtk::ResponseType::Accept {
            if let Some(пояс) = выбор.active_id() {
                let mut данн = данные();
                if !данн["world"].is_array() {
                    данн["world"] = json!([]);
                }
                if let Some(а) = данн["world"].as_array_mut() {
                    if !а.iter().any(|х| х.as_str() == Some(пояс.as_str())) {
                        а.push(json!(пояс.as_str()));
                    }
                }
                пиши_данные(ФАЙЛ, &данн);
                рисуй();
            }
        }
        unsafe { д.destroy() };
    });
    к.upcast()
}

#[cfg(test)]
mod проверки {
    use super::*;

    #[test]
    fn время_строкой() {
        assert_eq!(мс_строкой(61_230), "01:01.23");
        assert_eq!(сек_строкой(65), "01:05");
        assert_eq!(сек_строкой(3725), "1:02:05");
    }
}
