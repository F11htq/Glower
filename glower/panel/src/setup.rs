//! Мастер первого запуска и приветствие после него.
//!
//! Живёт в самой панели: всё, что он настраивает, хранит ядро, и
//! передавать выбранное через шину незачем. Мастер встаёт поверх всего,
//! панель и док на это время прячутся. Обои при выборе меняются сразу —
//! стол виден сквозь полупрозрачный фон мастера.
//!
//! После мастера — приветствие: знак системы собирается из пыли, вспышка,
//! «Привет, имя» по буквам, и всё сжимается в точку, из которой кругом
//! раскрывается рабочий стол. Щелчок или клавиша — сразу к финалу.

use crate::agent::{позови, скажи};
use crate::core;
use gtk::gdk_pixbuf::Pixbuf;
use gtk::glib;
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::f64::consts::PI;
use std::rc::Rc;
use std::time::Instant;

const СТИЛЬ: &str = r#"
window.setup, window.setup * { color: rgba(255, 255, 255, 0.95); font-family: sans-serif; text-shadow: none; }
.setup .card {
  background-color: rgba(18, 20, 30, 0.92);
  border: 1px solid rgba(255, 255, 255, 0.10);
  border-radius: 22px;
  padding: 30px 34px 24px 34px;
}
.setup .s-title { font-size: 26px; font-weight: 300; }
.setup .s-sub { color: rgba(255, 255, 255, 0.65); font-size: 13px; }
.setup .dots label { font-size: 9px; color: rgba(255, 255, 255, 0.25); }
.setup .dots label.on { color: #6c9bff; }
.setup .dots label.was { color: rgba(255, 255, 255, 0.6); }
.setup button {
  background-image: none;
  background-color: rgba(255, 255, 255, 0.08);
  border: none;
  border-radius: 10px;
  box-shadow: none;
  padding: 8px 16px;
}
.setup button:hover { background-color: rgba(255, 255, 255, 0.14); }
.setup button.next { background-color: #3a6df0; font-weight: bold; padding: 8px 26px; }
.setup button.next:hover { background-color: #4b7cff; }
.setup button.row { border-radius: 10px; padding: 8px 12px; }
.setup button.row.on { background-color: rgba(58, 109, 240, 0.35); box-shadow: inset 0 0 0 1px #6c9bff; }
.setup .code { font-weight: bold; color: rgba(255, 255, 255, 0.6); min-width: 34px; }
.setup .hint { color: rgba(255, 255, 255, 0.5); font-size: 12px; }
.setup .err { color: #ff8a8a; font-size: 12px; }
.setup entry {
  background-color: rgba(255, 255, 255, 0.07);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 10px;
  padding: 8px 12px;
  font-size: 15px;
  box-shadow: none;
}
.setup entry.emoji { font-size: 22px; }
.setup .brand { font-size: 30px; font-weight: 300; }
.setup button.acc { min-width: 34px; min-height: 34px; padding: 0; border-radius: 17px; }
.setup button.acc.on { box-shadow: 0 0 0 3px rgba(255, 255, 255, 0.9); }
.setup button.wall { min-width: 120px; min-height: 72px; padding: 0; border-radius: 12px; }
.setup button.wall.on { box-shadow: inset 0 0 0 3px white; }
.setup button.big { min-width: 180px; padding: 16px; border-radius: 14px; }
.setup button.big.on { background-color: rgba(58, 109, 240, 0.35); box-shadow: inset 0 0 0 1px #6c9bff; }
.setup .glyph { font-size: 22px; }
.setup list, .setup scrolledwindow, .setup viewport { background-color: transparent; }
.setup list row { padding: 2px 0; background-color: transparent; }
.setup checkbutton check { border-radius: 4px; }
"#;

#[derive(Clone)]
struct Выбор {
    язык: String,
    раскладки: Vec<String>,
    город: String,
    имя: String,
    знак: String,
    тема: String,
    акцент: u64,
    обои: String,
}

struct Мастер {
    окно: gtk::Window,
    точки: gtk::Box,
    заголовок: gtk::Label,
    пояснение: gtk::Label,
    место: gtk::Box,
    назад: gtk::Button,
    далее: gtk::Button,
    шаг: usize,
    выбор: Выбор,
    все_раскладки: Vec<Value>,
    ошибка: Option<gtk::Label>,
}

const ШАГИ: &[(&str, &str)] = &[
    ("Здравствуйте", "Настроим систему под вас. Это займёт минуту."),
    ("Язык системы", "Его можно будет сменить в Параметрах в любой момент."),
    ("Раскладки клавиатуры", "Выберите до четырёх. Переключение — Alt + Shift."),
    ("Ваш город", "Нужен только для погоды. Данные берутся из открытого сервиса Open-Meteo."),
    ("Как к вам обращаться", "Имя видно в меню и на экране входа."),
    ("Оформление", "Тему и цвет тоже можно менять когда угодно."),
    ("Обои", ""),
    ("Что система о вас знает", "Коротко и без мелкого шрифта."),
    ("Всё готово", "Осталось нажать кнопку."),
];

/// Знакомые имена раскладок: в базе XKB они по-английски
const РАСКЛАДКИ_ПО_РУССКИ: &[(&str, &str)] = &[
    ("us", "Английская (США)"),
    ("ru", "Русская"),
    ("ua", "Украинская"),
    ("by", "Белорусская"),
    ("kz", "Казахская"),
    ("de", "Немецкая"),
    ("fr", "Французская"),
    ("es", "Испанская"),
    ("it", "Итальянская"),
    ("pt", "Португальская"),
    ("pl", "Польская"),
    ("tr", "Турецкая"),
    ("gb", "Английская (Великобритания)"),
];

thread_local! {
    static МАСТЕР: RefCell<Option<Rc<RefCell<Мастер>>>> = const { RefCell::new(None) };
}

fn с_мастером<R>(дело: impl FnOnce(&mut Мастер) -> R) -> Option<R> {
    let м = МАСТЕР.with(|м| м.borrow().clone())?;
    let mut м = м.borrow_mut();
    Some(дело(&mut м))
}

fn стиль() {
    let с = gtk::CssProvider::new();
    if с.load_from_data(СТИЛЬ.as_bytes()).is_ok() {
        if let Some(э) = gtk::gdk::Screen::default() {
            gtk::StyleContext::add_provider_for_screen(&э, &с, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
        }
    }
}

fn поверх_всего(окно: &gtk::Window, имя: &str) {
    окно.set_decorated(false);
    окно.set_app_paintable(true);
    if let Some(в) = gtk::prelude::GtkWindowExt::screen(окно).and_then(|э| э.rgba_visual()) {
        окно.set_visual(Some(&в));
    }
    окно.init_layer_shell();
    окно.set_layer(Layer::Overlay);
    окно.set_namespace(имя);
    for край in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        окно.set_anchor(край, true);
    }
    окно.set_exclusive_zone(-1);
    окно.set_keyboard_mode(KeyboardMode::Exclusive);
}

/// Открыть мастер. Второй раз — просто поднять уже открытый
pub fn покажи() {
    if МАСТЕР.with(|м| м.borrow().is_some()) {
        return;
    }
    стиль();
    скажи("стол-занят", json!(true));

    let s = |k: &str| core::настройка(k);
    let выбор = Выбор {
        язык: core::язык_сейчас(),
        раскладки: vec!["us".into(), "ru".into()],
        город: s("city").as_str().unwrap_or("Москва").to_string(),
        имя: s("userName").as_str().unwrap_or("").to_string(),
        знак: s("userEmoji").as_str().unwrap_or("").to_string(),
        тема: s("theme").as_str().unwrap_or("dark").to_string(),
        акцент: s("accent").as_u64().unwrap_or(0),
        обои: s("wallpaper").as_str().unwrap_or("gora").to_string(),
    };

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.style_context().add_class("setup");
    поверх_всего(&окно, "glower-setup");
    // Стол виден сквозь фон: выбранные обои меняются прямо за мастером
    окно.connect_draw(|_, к| {
        к.set_operator(gtk::cairo::Operator::Source);
        к.set_source_rgba(0.02, 0.025, 0.05, 0.55);
        let _ = к.paint();
        glib::Propagation::Proceed
    });

    let карточка = gtk::Box::new(gtk::Orientation::Vertical, 10);
    карточка.style_context().add_class("card");
    карточка.set_halign(gtk::Align::Center);
    карточка.set_valign(gtk::Align::Center);
    карточка.set_size_request(600, 520);

    let точки = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    точки.style_context().add_class("dots");
    точки.set_halign(gtk::Align::Center);
    let заголовок = gtk::Label::new(None);
    заголовок.style_context().add_class("s-title");
    заголовок.set_xalign(0.0);
    let пояснение = gtk::Label::new(None);
    пояснение.style_context().add_class("s-sub");
    пояснение.set_xalign(0.0);
    пояснение.set_line_wrap(true);
    let место = gtk::Box::new(gtk::Orientation::Vertical, 10);
    место.set_vexpand(true);
    место.set_margin_top(10);

    let низ = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let назад = gtk::Button::with_label("Назад");
    let далее = gtk::Button::with_label("Далее");
    далее.style_context().add_class("next");
    низ.pack_start(&назад, false, false, 0);
    низ.pack_end(&далее, false, false, 0);
    назад.connect_clicked(|_| назад_шаг());
    далее.connect_clicked(|_| вперёд());

    карточка.add(&точки);
    карточка.add(&заголовок);
    карточка.add(&пояснение);
    карточка.add(&место);
    карточка.add(&низ);
    окно.add(&карточка);

    // Без мыши мастер тоже проходим: Ввод — дальше, Alt+← — назад
    окно.connect_key_press_event(|_, е| {
        use gtk::gdk::keys::constants as к;
        let кл = е.keyval();
        if кл == к::Return || кл == к::KP_Enter {
            вперёд();
            return glib::Propagation::Stop;
        }
        if кл == к::Left && е.state().contains(gtk::gdk::ModifierType::MOD1_MASK) {
            назад_шаг();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });

    let м = Rc::new(RefCell::new(Мастер {
        окно: окно.clone(),
        точки,
        заголовок,
        пояснение,
        место,
        назад,
        далее,
        шаг: 0,
        выбор,
        все_раскладки: Vec::new(),
        ошибка: None,
    }));
    МАСТЕР.with(|я| *я.borrow_mut() = Some(м));
    нарисуй();
    окно.show_all();

    // Что выбрано сейчас и что вообще есть — знает система
    позови("sys.kb.list", json!({}), |о| {
        if let Ok(о) = о {
            с_мастером(|м| {
                let выбрано: Vec<String> = о["выбрано"].as_array().map(|а| а.iter().filter_map(|х| х.as_str().map(String::from)).collect()).unwrap_or_default();
                if !выбрано.is_empty() {
                    м.выбор.раскладки = выбрано;
                }
                м.все_раскладки = о["список"].as_array().cloned().unwrap_or_default();
            });
            if с_мастером(|м| м.шаг == 2) == Some(true) {
                нарисуй();
            }
        }
    });
}

fn вперёд() {
    let Some((шаг, имя)) = с_мастером(|м| (м.шаг, м.выбор.имя.trim().to_string())) else { return };
    if шаг == 4 && имя.is_empty() {
        с_мастером(|м| {
            if let Some(о) = &м.ошибка {
                о.set_text("Без имени дальше нельзя — так система будет к вам обращаться");
            }
        });
        return;
    }
    if шаг + 1 < ШАГИ.len() {
        с_мастером(|м| м.шаг += 1);
        нарисуй();
    } else {
        готово();
    }
}

fn назад_шаг() {
    if с_мастером(|м| {
        if м.шаг > 0 {
            м.шаг -= 1;
            true
        } else {
            false
        }
    }) == Some(true)
    {
        нарисуй();
    }
}

fn очисти(к: &gtk::Box) {
    for р in к.children() {
        к.remove(&р);
    }
}

fn нарисуй() {
    let Some((шаг, место, точки, заголовок, пояснение, назад, далее)) =
        с_мастером(|м| (м.шаг, м.место.clone(), м.точки.clone(), м.заголовок.clone(), м.пояснение.clone(), м.назад.clone(), м.далее.clone()))
    else {
        return;
    };
    очисти(&точки);
    for i in 0..ШАГИ.len() {
        let т = gtk::Label::new(Some("●"));
        if i == шаг {
            т.style_context().add_class("on");
        } else if i < шаг {
            т.style_context().add_class("was");
        }
        точки.add(&т);
    }
    let (з, п) = ШАГИ[шаг];
    заголовок.set_text(з);
    пояснение.set_text(п);
    пояснение.set_visible(!п.is_empty());
    назад.set_opacity(if шаг > 0 { 1.0 } else { 0.0 });
    назад.set_sensitive(шаг > 0);
    далее.set_label(if шаг + 1 == ШАГИ.len() { "Начать работу" } else { "Далее" });
    очисти(&место);
    с_мастером(|м| м.ошибка = None);
    match шаг {
        0 => шаг_привет(&место),
        1 => шаг_язык(&место),
        2 => шаг_раскладки(&место),
        3 => шаг_город(&место),
        4 => шаг_имя(&место),
        5 => шаг_оформление(&место),
        6 => шаг_обои(&место),
        7 => шаг_тайна(&место),
        _ => шаг_итог(&место),
    }
    место.show_all();
    точки.show_all();
    пояснение.set_visible(!п.is_empty());
    if шаг != 3 && шаг != 4 {
        далее.grab_focus();
    }
}

/* ---------- шаги ---------- */

pub(crate) fn логотип(размер: i32) -> Option<Pixbuf> {
    let рядом = std::env::current_exe().ok().and_then(|п| п.parent().map(|р| р.join("../../../assets/logo.png")));
    [std::path::PathBuf::from("/usr/share/glower/ui/assets/logo.png"), std::path::PathBuf::from("assets/logo.png")]
        .into_iter()
        .chain(рядом)
        .find_map(|п| Pixbuf::from_file_at_scale(п, размер, размер, true).ok())
}

fn шаг_привет(место: &gtk::Box) {
    let к = gtk::Box::new(gtk::Orientation::Vertical, 10);
    к.set_valign(gtk::Align::Center);
    к.set_vexpand(true);
    if let Some(л) = логотип(110) {
        к.add(&gtk::Image::from_pixbuf(Some(&л)));
    }
    let имя = gtk::Label::new(Some("GlowerOS"));
    имя.style_context().add_class("brand");
    к.add(&имя);
    let в = gtk::Label::new(Some(&format!("Версия {}", core::версия())));
    в.style_context().add_class("hint");
    к.add(&в);
    место.add(&к);
}

fn строка_выбора(код: &str, имя: &str, подпись: &str, вкл: bool) -> gtk::Button {
    let б = gtk::Button::new();
    б.style_context().add_class("row");
    if вкл {
        б.style_context().add_class("on");
    }
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let к = gtk::Label::new(Some(код));
    к.style_context().add_class("code");
    к.set_xalign(0.0);
    ряд.add(&к);
    let и = gtk::Label::new(Some(имя));
    и.set_xalign(0.0);
    и.set_hexpand(true);
    ряд.add(&и);
    let п = gtk::Label::new(Some(подпись));
    п.style_context().add_class("hint");
    ряд.add(&п);
    let г = gtk::Label::new(Some(if вкл { "✓" } else { "" }));
    г.set_size_request(16, -1);
    ряд.add(&г);
    б.add(&ряд);
    б
}

fn листание(что: &impl IsA<gtk::Widget>, высота: i32) -> gtk::ScrolledWindow {
    let л = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    л.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    л.set_min_content_height(высота);
    л.set_vexpand(true);
    л.add(что);
    л
}

fn шаг_язык(место: &gtk::Box) {
    let язык = с_мастером(|м| м.выбор.язык.clone()).unwrap_or_default();
    let список = gtk::Box::new(gtk::Orientation::Vertical, 4);
    for (k, имя, локаль) in core::ЯЗЫКИ {
        let б = строка_выбора(&k.to_uppercase(), имя, локаль, язык == *k);
        let k = k.to_string();
        б.connect_clicked(move |_| {
            с_мастером(|м| м.выбор.язык = k.clone());
            нарисуй();
        });
        список.add(&б);
    }
    место.add(&листание(&список, 320));
}

fn имя_раскладки(id: &str, все: &[Value]) -> String {
    РАСКЛАДКИ_ПО_РУССКИ
        .iter()
        .find(|(к, _)| *к == id)
        .map(|(_, и)| и.to_string())
        .or_else(|| все.iter().find(|р| р["id"].as_str() == Some(id)).and_then(|р| р["имя"].as_str().map(String::from)))
        .unwrap_or_else(|| id.to_uppercase())
}

fn шаг_раскладки(место: &gtk::Box) {
    let (выбрано, все) = с_мастером(|м| (м.выбор.раскладки.clone(), м.все_раскладки.clone())).unwrap_or_default();
    let поиск = gtk::SearchEntry::new();
    поиск.set_placeholder_text(Some("Найти раскладку"));
    место.add(&поиск);
    let выбранные = gtk::Label::new(Some(&format!(
        "Выбрано: {}",
        выбрано.iter().map(|р| имя_раскладки(р, &все)).collect::<Vec<_>>().join(", ")
    )));
    выбранные.style_context().add_class("hint");
    выбранные.set_xalign(0.0);
    выбранные.set_line_wrap(true);
    место.add(&выбранные);

    // Сначала выбранные, потом знакомые, потом всё остальное
    let mut порядок: Vec<String> = выбрано.clone();
    for (к, _) in РАСКЛАДКИ_ПО_РУССКИ {
        if !порядок.iter().any(|р| р == к) {
            порядок.push(к.to_string());
        }
    }
    for р in &все {
        if let Some(id) = р["id"].as_str() {
            if !порядок.iter().any(|х| х == id) {
                порядок.push(id.to_string());
            }
        }
    }
    let список = gtk::ListBox::new();
    список.set_selection_mode(gtk::SelectionMode::None);
    for id in порядок {
        let имя = имя_раскладки(&id, &все);
        let г = gtk::CheckButton::with_label(&format!("{имя}  ·  {}", id.to_uppercase()));
        г.set_active(выбрано.contains(&id));
        let id2 = id.clone();
        г.connect_toggled(move |г| {
            let mut откатить = false;
            с_мастером(|м| {
                let с = &mut м.выбор.раскладки;
                if г.is_active() {
                    if с.len() >= 4 {
                        откатить = true;
                    } else if !с.contains(&id2) {
                        с.push(id2.clone());
                    }
                } else if с.len() > 1 {
                    с.retain(|х| х != &id2);
                } else {
                    // Хотя бы одна раскладка нужна: печатать чем-то надо
                    откатить = true;
                }
            });
            if откатить {
                let г = г.clone();
                glib::idle_add_local_once(move || г.set_active(!г.is_active()));
                return;
            }
            let (выбрано, все) = с_мастером(|м| (м.выбор.раскладки.clone(), м.все_раскладки.clone())).unwrap_or_default();
            if let Some(л) = г
                .ancestor(gtk::ScrolledWindow::static_type())
                .and_then(|п| п.parent())
                .and_then(|п| п.downcast::<gtk::Box>().ok())
                .and_then(|п| п.children().get(1).cloned())
                .and_then(|л| л.downcast::<gtk::Label>().ok())
            {
                л.set_text(&format!("Выбрано: {}", выбрано.iter().map(|р| имя_раскладки(р, &все)).collect::<Vec<_>>().join(", ")));
            }
        });
        let строка = gtk::ListBoxRow::new();
        строка.set_widget_name(&format!("{} {}", имя.to_lowercase(), id));
        строка.add(&г);
        список.add(&строка);
    }
    let строка_поиска = Rc::new(RefCell::new(String::new()));
    {
        let с = строка_поиска.clone();
        список.set_filter_func(Some(Box::new(move |р| {
            let что = с.borrow();
            что.is_empty() || р.widget_name().contains(что.as_str())
        })));
    }
    {
        let список = список.clone();
        поиск.connect_search_changed(move |п| {
            *строка_поиска.borrow_mut() = п.text().trim().to_lowercase();
            список.invalidate_filter();
        });
    }
    место.add(&листание(&список, 220));
}

fn поле(текст: &str, подсказка: &str) -> gtk::Entry {
    let п = gtk::Entry::new();
    п.set_text(текст);
    п.set_placeholder_text(Some(подсказка));
    п.set_activates_default(false);
    п.connect_activate(|_| вперёд());
    п
}

fn шаг_город(место: &gtk::Box) {
    let город = с_мастером(|м| м.выбор.город.clone()).unwrap_or_default();
    let п = поле(&город, "Москва");
    п.connect_changed(|п| {
        let т = п.text().to_string();
        с_мастером(|м| м.выбор.город = т);
    });
    место.add(&п);
    let п2 = п.clone();
    glib::idle_add_local_once(move || {
        п2.grab_focus();
        п2.set_position(-1);
    });
}

fn шаг_имя(место: &gtk::Box) {
    let (имя, знак) = с_мастером(|м| (м.выбор.имя.clone(), м.выбор.знак.clone())).unwrap_or_default();
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let з = поле(&знак, "🙂");
    з.style_context().add_class("emoji");
    з.set_max_length(2);
    з.set_width_chars(3);
    з.connect_changed(|п| {
        let т = п.text().to_string();
        с_мастером(|м| м.выбор.знак = т);
    });
    let и = поле(&имя, "Имя пользователя");
    и.set_max_length(24);
    и.set_hexpand(true);
    ряд.add(&з);
    ряд.add(&и);
    место.add(&ряд);
    let ошибка = gtk::Label::new(None);
    ошибка.style_context().add_class("err");
    ошибка.set_xalign(0.0);
    место.add(&ошибка);
    {
        let ошибка = ошибка.clone();
        и.connect_changed(move |п| {
            let т = п.text().to_string();
            с_мастером(|м| м.выбор.имя = т);
            ошибка.set_text("");
        });
    }
    с_мастером(|м| м.ошибка = Some(ошибка));
    glib::idle_add_local_once(move || и.grab_focus());
}

fn с_фоном(в: &impl IsA<gtk::Widget>, css: &str) {
    let с = gtk::CssProvider::new();
    if с.load_from_data(format!("* {{ background-image: {}; background-size: cover; }}", crate::desktop::цвета_для_gtk(css)).as_bytes()).is_ok() {
        в.style_context().add_provider(&с, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 3);
    }
}

fn данные() -> Value {
    serde_json::from_str(include_str!("shell-data.json")).unwrap_or_default()
}

fn шаг_оформление(место: &gtk::Box) {
    let (тема, акцент) = с_мастером(|м| (м.выбор.тема.clone(), м.выбор.акцент)).unwrap_or_default();
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    ряд.set_halign(gtk::Align::Center);
    for (v, имя, подпись, знак) in [("dark", "Тёмная", "тёмные поверхности", "●"), ("light", "Светлая", "светлые поверхности", "○")] {
        let б = gtk::Button::new();
        б.style_context().add_class("big");
        if тема == v {
            б.style_context().add_class("on");
        }
        let к = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let з = gtk::Label::new(Some(знак));
        з.style_context().add_class("glyph");
        к.add(&з);
        к.add(&gtk::Label::new(Some(имя)));
        let п = gtk::Label::new(Some(подпись));
        п.style_context().add_class("hint");
        к.add(&п);
        б.add(&к);
        б.connect_clicked(move |_| {
            с_мастером(|м| м.выбор.тема = v.to_string());
            нарисуй();
        });
        ряд.add(&б);
    }
    место.add(&ряд);
    let цвет = gtk::Label::new(Some("Цвет акцента"));
    цвет.style_context().add_class("hint");
    цвет.set_margin_top(14);
    место.add(&цвет);
    let цвета = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    цвета.set_halign(gtk::Align::Center);
    for (i, а) in данные()["акценты"].as_array().cloned().unwrap_or_default().iter().enumerate() {
        let б = gtk::Button::new();
        б.style_context().add_class("acc");
        if i as u64 == акцент {
            б.style_context().add_class("on");
        }
        б.set_tooltip_text(а["имя"].as_str());
        с_фоном(&б, &format!("linear-gradient(135deg, {}, {})", а["a"].as_str().unwrap_or("#3a86ff"), а["b"].as_str().unwrap_or("#3a86ff")));
        б.connect_clicked(move |_| {
            с_мастером(|м| м.выбор.акцент = i as u64);
            нарисуй();
        });
        цвета.add(&б);
    }
    место.add(&цвета);
}

fn шаг_обои(место: &gtk::Box) {
    let обои = с_мастером(|м| м.выбор.обои.clone()).unwrap_or_default();
    let сетка = gtk::FlowBox::new();
    сетка.set_selection_mode(gtk::SelectionMode::None);
    сетка.set_max_children_per_line(4);
    сетка.set_min_children_per_line(4);
    сетка.set_row_spacing(8);
    сетка.set_column_spacing(8);
    for в in данные()["обои"].as_array().cloned().unwrap_or_default() {
        let id = в["id"].as_str().unwrap_or("").to_string();
        let б = gtk::Button::new();
        б.style_context().add_class("wall");
        if id == обои {
            б.style_context().add_class("on");
        }
        б.set_tooltip_text(в["имя"].as_str());
        с_фоном(&б, в["css"].as_str().unwrap_or(""));
        б.connect_clicked(move |_| {
            с_мастером(|м| м.выбор.обои = id.clone());
            // Сразу на стол: за мастером видно, как это будет
            core::задай("wallpaper", json!(id));
            нарисуй();
        });
        сетка.add(&б);
    }
    место.add(&листание(&сетка, 300));
}

fn шаг_тайна(место: &gtk::Box) {
    for (знак, заголовок, текст) in [
        ("💾", "Всё остаётся здесь", "Настройки и файлы хранятся на этой машине. Никакой учётной записи и никакой отправки на сторону."),
        ("🌤", "Единственный запрос наружу", "Погода: город уходит в открытый сервис Open-Meteo. Откажетесь — погода просто не покажется."),
        ("🚫", "Чего здесь нет", "Ни сбора статистики, ни рекламного идентификатора, ни «улучшения качества продукта»."),
        ("🔍", "Проверяется", "Система открыта: весь код лежит рядом и читается глазами."),
    ] {
        let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        let з = gtk::Label::new(Some(знак));
        з.style_context().add_class("glyph");
        з.set_valign(gtk::Align::Start);
        ряд.add(&з);
        let к = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let з2 = gtk::Label::new(None);
        з2.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(заголовок)));
        з2.set_xalign(0.0);
        к.add(&з2);
        let т = gtk::Label::new(Some(текст));
        т.style_context().add_class("hint");
        т.set_xalign(0.0);
        т.set_line_wrap(true);
        т.set_max_width_chars(60);
        к.add(&т);
        ряд.add(&к);
        место.add(&ряд);
    }
}

fn шаг_итог(место: &gtk::Box) {
    let Some((в, все)) = с_мастером(|м| (м.выбор.clone(), м.все_раскладки.clone())) else { return };
    let язык = core::ЯЗЫКИ.iter().find(|(k, _, _)| *k == в.язык).map(|(_, и, _)| *и).unwrap_or("Русский");
    let раскладки = в.раскладки.iter().map(|р| имя_раскладки(р, &все)).collect::<Vec<_>>().join(", ");
    let город = if в.город.trim().is_empty() { "не указан".to_string() } else { в.город.trim().to_string() };
    let тема = if в.тема == "light" { "Светлая" } else { "Тёмная" };
    let сетка = gtk::Grid::new();
    сетка.set_row_spacing(10);
    сетка.set_column_spacing(14);
    for (i, (знак, ключ, что)) in [("🗣", "Язык", язык.to_string()), ("⌨️", "Раскладки", раскладки), ("🏙", "Город", город), ("👤", "Пользователь", в.имя.trim().to_string()), ("🎨", "Тема", тема.to_string())].into_iter().enumerate() {
        сетка.attach(&gtk::Label::new(Some(знак)), 0, i as i32, 1, 1);
        let к = gtk::Label::new(Some(ключ));
        к.style_context().add_class("hint");
        к.set_xalign(0.0);
        сетка.attach(&к, 1, i as i32, 1, 1);
        let ч = gtk::Label::new(Some(&что));
        ч.set_xalign(0.0);
        ч.set_line_wrap(true);
        ч.set_max_width_chars(40);
        сетка.attach(&ч, 2, i as i32, 1, 1);
    }
    место.add(&сетка);
}

/* ---------- применение ---------- */

fn готово() {
    let Some((в, окно)) = с_мастером(|м| (м.выбор.clone(), м.окно.clone())) else { return };
    core::задай("theme", json!(в.тема));
    core::задай("accent", json!(в.акцент));
    core::задай("wallpaper", json!(в.обои));
    core::задай("city", json!(if в.город.trim().is_empty() { "Москва" } else { в.город.trim() }));
    core::задай("userName", json!(в.имя.trim()));
    let знак = if в.знак.trim().is_empty() { в.имя.trim().chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "🙂".into()) } else { в.знак.trim().to_string() };
    core::задай("userEmoji", json!(знак));
    core::задай("setupDone", json!(true));
    позови("sys.kb.set", json!({ "раскладки": в.раскладки }), |_| {});
    if в.язык != core::язык_сейчас() {
        позови("sys.locale.set", json!({ "язык": в.язык }), |_| {});
    }
    core::обнови_погоду();
    окно.close();
    МАСТЕР.with(|м| *м.borrow_mut() = None);
    приветствие(в.имя.trim(), &в.язык);
}

/* ---------- приветствие ---------- */

/// Слова приветствия на языке системы. Незнакомый язык получает английские
fn тексты(язык: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    // привет, добро пожаловать в систему, без имени, пропустить
    match язык {
        "ru" => ("Привет", "Добро пожаловать в GlowerOS", "Добро пожаловать", "Щелчок — пропустить"),
        "uk" => ("Привіт", "Ласкаво просимо до GlowerOS", "Ласкаво просимо", "Клацніть, щоб пропустити"),
        "be" => ("Прывітанне", "Сардэчна запрашаем у GlowerOS", "Сардэчна запрашаем", "Пстрыкніце, каб прапусціць"),
        "kk" => ("Сәлем", "GlowerOS-қа қош келдіңіз", "Қош келдіңіз", "Өткізіп жіберу үшін басыңыз"),
        "de" => ("Hallo", "Willkommen bei GlowerOS", "Willkommen", "Klicken zum Überspringen"),
        "fr" => ("Bonjour", "Bienvenue dans GlowerOS", "Bienvenue", "Cliquez pour passer"),
        "es" => ("Hola", "Bienvenido a GlowerOS", "Bienvenido", "Haz clic para omitir"),
        "it" => ("Ciao", "Benvenuto in GlowerOS", "Benvenuto", "Fai clic per saltare"),
        "pt" => ("Olá", "Bem-vindo ao GlowerOS", "Bem-vindo", "Clique para pular"),
        "pl" => ("Cześć", "Witamy w GlowerOS", "Witamy", "Kliknij, aby pominąć"),
        "tr" => ("Merhaba", "GlowerOS’a hoş geldiniz", "Hoş geldiniz", "Geçmek için tıklayın"),
        _ => ("Hello", "Welcome to GlowerOS", "Welcome", "Click to skip"),
    }
}

struct Частица {
    // Куда прилетит — точка знака, от центра экрана
    цель_r: f64,
    цель_a: f64,
    // Откуда: радиус орбиты и сколько витков до места
    r0: f64,
    виток: f64,
    задержка: f64,
    цвет: (f64, f64, f64),
}

/// Простой генератор случайных чисел: красоте пыли криптография не нужна
struct Жребий(u64);
impl Жребий {
    fn дай(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn частицы(знак: Option<&Pixbuf>, размер: f64) -> Vec<Частица> {
    let mut ж = Жребий(glib::real_time() as u64 | 1);
    let mut точки: Vec<(f64, f64, (f64, f64, f64))> = Vec::new();
    if let Some(к) = знак {
        let (ш, в, шаг) = (к.width(), к.height(), к.rowstride());
        let каналы = к.n_channels();
        let байты = к.read_pixel_bytes();
        for y in (0..в).step_by(3) {
            for x in (0..ш).step_by(3) {
                let i = (y * шаг + x * каналы) as usize;
                if i + 3 >= байты.len() {
                    continue;
                }
                let прозрачность = if каналы == 4 { байты[i + 3] } else { 255 };
                if прозрачность > 140 {
                    точки.push((
                        x as f64 - ш as f64 / 2.0,
                        y as f64 - в as f64 / 2.0,
                        (байты[i] as f64 / 255.0, байты[i + 1] as f64 / 255.0, байты[i + 2] as f64 / 255.0),
                    ));
                }
            }
        }
    }
    if точки.is_empty() {
        // Знака нет — собираем кольцо
        for i in 0..900 {
            let а = i as f64 / 900.0 * 2.0 * PI;
            let r = размер * 0.4 + (ж.дай() - 0.5) * 12.0;
            точки.push((r * а.cos(), r * а.sin(), (0.45 + 0.3 * ж.дай(), 0.6, 1.0)));
        }
    }
    // Не больше двух тысяч: на слабой машине кадр должен успевать
    while точки.len() > 2000 {
        let i = (ж.дай() * точки.len() as f64) as usize % точки.len();
        точки.swap_remove(i);
    }
    точки
        .into_iter()
        .map(|(x, y, цвет)| Частица {
            цель_r: (x * x + y * y).sqrt(),
            цель_a: y.atan2(x),
            r0: размер * (1.6 + ж.дай() * 2.4),
            виток: 2.5 + ж.дай() * 4.0,
            задержка: ж.дай() * 0.35,
            цвет,
        })
        .collect()
}

fn плавно(p: f64) -> f64 {
    let p = p.clamp(0.0, 1.0);
    1.0 - (1.0 - p).powi(3)
}

/// Сцена: 0 — оси проявляются, 0.3–2.0 — сборка, 2.0 — вспышка,
/// 2.4 — слова по буквам, конец — всё в точку и стол раскрывается кругом
pub fn приветствие(имя: &str, язык: &str) {
    стиль();
    скажи("стол-занят", json!(true));
    let (привет, добро, без_имени, пропустить) = тексты(язык);
    let строка = if имя.is_empty() { без_имени.to_string() } else { format!("{привет}, {имя}") };
    let знак = логотип(180);
    let пыль = частицы(знак.as_ref(), 90.0);

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.style_context().add_class("setup");
    поверх_всего(&окно, "glower-welcome");
    let холст = gtk::DrawingArea::new();
    окно.add(&холст);

    let начало = Rc::new(Cell::new(Instant::now()));
    // Пропуск: сдвигаем начало так, будто сборка уже прошла
    let пропущено = Rc::new(Cell::new(false));
    const СЛОВА: f64 = 2.4;
    const ФИНАЛ: f64 = 5.0;
    const КОНЕЦ: f64 = 6.1;
    {
        let (начало, пропущено) = (начало.clone(), пропущено.clone());
        let пропусти = move || {
            if !пропущено.replace(true) {
                let прошло = начало.get().elapsed().as_secs_f64();
                if прошло < СЛОВА {
                    начало.set(Instant::now() - std::time::Duration::from_secs_f64(СЛОВА));
                }
            }
        };
        let п2 = пропусти.clone();
        окно.connect_button_press_event(move |_, _| {
            п2();
            glib::Propagation::Stop
        });
        окно.connect_key_press_event(move |_, _| {
            пропусти();
            glib::Propagation::Stop
        });
    }
    {
        let начало = начало.clone();
        холст.connect_draw(move |в, к| {
            let t = начало.get().elapsed().as_secs_f64();
            let (ш, вы) = (в.allocated_width() as f64, в.allocated_height() as f64);
            let (cx, cy) = (ш / 2.0, вы / 2.0 - 30.0);
            к.set_operator(gtk::cairo::Operator::Source);
            // Финал: тёмное небо уходит кругом от центра — под ним стол
            let раскрытие = ((t - ФИНАЛ - 0.35) / (КОНЕЦ - ФИНАЛ - 0.35)).clamp(0.0, 1.0);
            к.set_source_rgba(0.02, 0.024, 0.05, 1.0);
            if раскрытие > 0.0 {
                к.set_fill_rule(gtk::cairo::FillRule::EvenOdd);
                к.rectangle(0.0, 0.0, ш, вы);
                к.arc(ш / 2.0, вы / 2.0, плавно(раскрытие) * (ш * ш + вы * вы).sqrt() / 2.0 + 1.0, 0.0, 2.0 * PI);
                let _ = к.fill();
                // Остальное рисуем только поверх неба
                к.set_operator(gtk::cairo::Operator::Over);
                return glib::Propagation::Stop;
            }
            let _ = к.paint();
            к.set_operator(gtk::cairo::Operator::Over);

            // Перед раскрытием всё сжимается в точку
            let сжатие = 1.0 - плавно((t - ФИНАЛ) / 0.35);
            к.save().ok();
            к.translate(cx, cy);
            к.scale(сжатие.max(0.001), сжатие.max(0.001));

            // Орбиты: тонкие круги, видны во время сборки
            let оси = (t / 0.4).clamp(0.0, 1.0) * (1.0 - ((t - 1.6) / 0.6).clamp(0.0, 1.0));
            if оси > 0.0 {
                к.set_line_width(1.0);
                for i in 0..5 {
                    let r = 140.0 + i as f64 * 70.0;
                    к.set_source_rgba(0.55, 0.65, 1.0, 0.10 * оси);
                    к.arc(0.0, 0.0, r, 0.0, 2.0 * PI);
                    let _ = к.stroke();
                }
            }

            // Пыль летит по спирали к своей точке знака
            let собрано = ((t - 1.9) / 0.5).clamp(0.0, 1.0);
            if собрано < 1.0 {
                for ч in &пыль {
                    let p = ((t - 0.3 - ч.задержка) / 1.5).clamp(0.0, 1.0);
                    let e = плавно(p);
                    let r = ч.цель_r + (1.0 - e) * (ч.r0 - ч.цель_r);
                    let а = ч.цель_a + (1.0 - e) * ч.виток;
                    let (x, y) = (r * а.cos(), r * а.sin());
                    let видно = (p * 4.0).min(1.0) * (1.0 - собрано);
                    // Хвост: где частица была мгновение назад
                    let e2 = плавно((p - 0.04).max(0.0));
                    let r2 = ч.цель_r + (1.0 - e2) * (ч.r0 - ч.цель_r);
                    let а2 = ч.цель_a + (1.0 - e2) * ч.виток;
                    к.set_source_rgba(ч.цвет.0, ч.цвет.1, ч.цвет.2, видно * 0.35);
                    к.set_line_width(1.2);
                    к.move_to(r2 * а2.cos(), r2 * а2.sin());
                    к.line_to(x, y);
                    let _ = к.stroke();
                    к.set_source_rgba(ч.цвет.0, ч.цвет.1, ч.цвет.2, видно);
                    к.rectangle(x - 1.0, y - 1.0, 2.2, 2.2);
                    let _ = к.fill();
                }
            }
            // Чёткий знак проявляется сквозь пыль
            if let Some(з) = &знак {
                if собрано > 0.0 {
                    к.set_source_pixbuf(з, -(з.width() as f64) / 2.0, -(з.height() as f64) / 2.0);
                    let _ = к.paint_with_alpha(собрано);
                }
            }
            // Вспышка и комета вокруг собранного знака
            let вспышка = ((t - 1.95) / 0.6).clamp(0.0, 1.0);
            if вспышка > 0.0 && вспышка < 1.0 {
                let г = gtk::cairo::RadialGradient::new(0.0, 0.0, 0.0, 0.0, 0.0, 60.0 + вспышка * 260.0);
                г.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.5 * (1.0 - вспышка));
                г.add_color_stop_rgba(1.0, 0.5, 0.6, 1.0, 0.0);
                let _ = к.set_source(&г);
                к.arc(0.0, 0.0, 60.0 + вспышка * 260.0, 0.0, 2.0 * PI);
                let _ = к.fill();
                let а = -PI / 2.0 + вспышка * 2.0 * PI;
                for i in 0..16 {
                    let аа = а - i as f64 * 0.035;
                    к.set_source_rgba(1.0, 1.0, 1.0, (1.0 - i as f64 / 16.0) * 0.8 * (1.0 - вспышка));
                    к.arc(130.0 * аа.cos(), 130.0 * аа.sin(), 3.0 - i as f64 * 0.15, 0.0, 2.0 * PI);
                    let _ = к.fill();
                }
            }
            // Слова: имя по буквам, под ним — «Добро пожаловать»
            if t > СЛОВА {
                к.select_font_face("sans-serif", gtk::cairo::FontSlant::Normal, gtk::cairo::FontWeight::Normal);
                к.set_font_size(40.0);
                let ширина = |к: &gtk::cairo::Context, т: &str| к.text_extents(т).map(|е| е.x_advance()).unwrap_or(0.0);
                let полная = ширина(к, &строка);
                let букв = строка.chars().count();
                let идёт = (t - СЛОВА) / 0.05;
                let показано = (идёт.floor() as usize).min(букв);
                let часть: String = строка.chars().take(показано).collect();
                к.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                к.move_to(-полная / 2.0, 150.0);
                let _ = к.show_text(&часть);
                if показано < букв {
                    let след: String = строка.chars().skip(показано).take(1).collect();
                    к.set_source_rgba(1.0, 1.0, 1.0, идёт.fract());
                    к.move_to(-полная / 2.0 + ширина(к, &часть), 150.0);
                    let _ = к.show_text(&след);
                }
                let второе = ((t - СЛОВА - 0.5) / 0.6).clamp(0.0, 1.0);
                if второе > 0.0 {
                    к.set_font_size(17.0);
                    let ш2 = ширина(к, добро);
                    к.set_source_rgba(1.0, 1.0, 1.0, 0.7 * второе);
                    к.move_to(-ш2 / 2.0, 192.0 + (1.0 - второе) * 8.0);
                    let _ = к.show_text(добро);
                }
            }
            к.restore().ok();
            // Подсказка внизу, пока ещё есть что пропускать
            if t < СЛОВА {
                к.select_font_face("sans-serif", gtk::cairo::FontSlant::Normal, gtk::cairo::FontWeight::Normal);
                к.set_font_size(12.0);
                let шп = к.text_extents(пропустить).map(|е| е.x_advance()).unwrap_or(0.0);
                к.set_source_rgba(1.0, 1.0, 1.0, 0.35 * (t / 0.8).min(1.0));
                к.move_to((ш - шп) / 2.0, вы - 50.0);
                let _ = к.show_text(пропустить);
            }
            glib::Propagation::Stop
        });
    }
    {
        let окно2 = окно.clone();
        let панель_вернули = Cell::new(false);
        холст.add_tick_callback(move |в, _| {
            в.queue_draw();
            let t = начало.get().elapsed().as_secs_f64();
            // Панель и док — заранее: круг раскрывает стол уже с ними
            if t > ФИНАЛ + 0.35 && !панель_вернули.replace(true) {
                скажи("стол-занят", json!(false));
            }
            if t > КОНЕЦ {
                окно2.close();
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
    }
    окно.show_all();
}
