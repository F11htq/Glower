//! Параметры — нативные, настоящим окном (glower-settings).
//!
//! Системное — сеть, звук, экраны, носители, принтеры, Bluetooth, учётные
//! записи, обновления, прошивки — Параметры спрашивают у агента сами, как
//! и страничные спрашивали. А то, что живёт в оболочке, — обои, тема,
//! акцент, док, анимации, виджеты, раскладки — хранит страница рабочего
//! стола: она рассказывает о нём по шине («параметры») и выполняет просьбы
//! («параметры-дело»). Так настройка, сменённая здесь, сразу видна на столе,
//! а сменённая на столе — здесь.

use crate::agent::{позови, скажи, слушай};
use crate::apps::подготовь;
use gtk::gdk_pixbuf::Pixbuf;
use gtk::glib;
use gtk::prelude::*;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

const СТИЛЬ: &str = r#"
.settings .sidebar { background-color: @glower_bg2; border-right: 1px solid alpha(@glower_fg, 0.07); }
.settings .sidebar list { background-color: transparent; }
.settings .sidebar row { padding: 7px 12px; border-radius: 8px; margin: 1px 8px; }
.settings .sidebar row:selected { background-color: alpha(@glower_accent, 0.28); }
.settings .sidebar entry { margin: 10px 10px 6px; }
.settings .page { padding: 22px 28px 40px; }
.settings .h1 { font-size: 22px; font-weight: 600; margin-bottom: 6px; }
.settings .card { background-color: alpha(@glower_fg, 0.045); border-radius: 14px; padding: 6px 0; }
.settings .card-t { font-size: 12px; opacity: 0.6; margin: 14px 4px 6px; }
.settings .sub { font-size: 13px; font-weight: 600; opacity: 0.8; margin: 8px 18px 2px; }
.settings .row { padding: 9px 16px; }
.settings .row + .row { border-top: 1px solid alpha(@glower_fg, 0.05); }
.settings .emo { font-size: 18px; min-width: 28px; }
.settings .t { font-size: 14px; }
.settings .d { font-size: 12px; opacity: 0.6; }
.settings .note { font-size: 12px; opacity: 0.6; padding: 6px 18px; }
.settings .val { font-size: 12px; opacity: 0.75; }
.settings .hero { padding: 16px; }
.settings .ava { font-size: 28px; min-width: 56px; min-height: 56px; border-radius: 28px;
                 background-image: linear-gradient(140deg, @glower_accent, @glower_accent2); color: #ffffff; }
.settings .wall { min-width: 132px; min-height: 76px; border-radius: 10px; padding: 0; border: 2px solid transparent; }
.settings .wall.on { border-color: #ffffff; }
.settings .wall-name { font-size: 11px; opacity: 0.75; }
.settings .sw { min-width: 30px; min-height: 30px; border-radius: 15px; padding: 0; border: 2px solid transparent; }
.settings .sw.on { border-color: #ffffff; }
.settings button.on { background-image: linear-gradient(140deg, @glower_accent, @glower_accent2); }
.settings .seg button { border-radius: 0; }
.settings .seg button:first-child { border-radius: 9px 0 0 9px; }
.settings .seg button:last-child { border-radius: 0 9px 9px 0; }
.settings scale { min-width: 200px; }
.settings .logo { font-size: 24px; font-weight: 600; }
"#;

#[derive(Clone, Copy)]
struct Раздел {
    id: &'static str,
    имя: &'static str,
    знак: &'static str,
    слова: &'static str,
}

const РАЗДЕЛЫ: &[Раздел] = &[
    Раздел { id: "home", имя: "Главная", знак: "🏠", слова: "" },
    Раздел { id: "system", имя: "Система", знак: "🖥️", слова: "питание батарея многозадачность рабочие столы" },
    Раздел { id: "person", имя: "Персонализация", знак: "🎨", слова: "обои тема цвет акцент шрифт оформление тёмная светлая виджеты" },
    Раздел { id: "dock", имя: "Рабочий стол и док", знак: "🧱", слова: "док панель задач иконки значки" },
    Раздел { id: "sound", имя: "Звук", знак: "🔊", слова: "звук громкость" },
    Раздел { id: "display", имя: "Дисплей", знак: "💡", слова: "яркость масштаб ночной свет экран" },
    Раздел { id: "net", имя: "Сеть и Интернет", знак: "📶", слова: "wi-fi вайфай ipv6 погода сеть" },
    Раздел { id: "drives", имя: "Носители", знак: "💾", слова: "флешка носитель диск usb карта памяти" },
    Раздел { id: "print", имя: "Печать и сканер", знак: "🖨️", слова: "принтер печать cups сканер сканирование" },
    Раздел { id: "bt", имя: "Устройства", знак: "🎧", слова: "bluetooth блютуз наушники мышь камера оборудование" },
    Раздел { id: "notif", имя: "Уведомления и фокус", знак: "🔔", слова: "не беспокоить фокусировка" },
    Раздел { id: "apps", имя: "Приложения", знак: "📦", слова: "wine windows автозапуск по умолчанию" },
    Раздел { id: "acc", имя: "Учётные записи", знак: "👤", слова: "пароль пользователь люди блокировка" },
    Раздел { id: "time", имя: "Время и язык", знак: "🌍", слова: "часы пояс язык раскладки клавиатура" },
    Раздел { id: "keys", имя: "Клавиатура", знак: "⌨️", слова: "сочетания клавиши" },
    Раздел { id: "privacy", имя: "Конфиденциальность", знак: "🔐", слова: "брандмауэр стена пароли хранилище" },
    Раздел { id: "update", имя: "Обновления", знак: "⬆️", слова: "обновления обновить версия прошивка прошивки bios" },
    Раздел { id: "about", имя: "О системе", знак: "ℹ️", слова: "версия железо процессор память" },
];

struct Окно {
    окно: gtk::Window,
    список: gtk::ListBox,
    страница: gtk::Box,
    прокрутка: gtk::ScrolledWindow,
    видимые: RefCell<Vec<&'static str>>,
    текущий: Cell<&'static str>,
    /// Чей ответ ещё нужен: раздел сменили — старые ответы не рисуем
    поколение: Cell<u64>,
    /// Что рассказала страница
    сост: RefCell<Value>,
    /// Раздел нарисован без рассказа страницы — дорисовать, когда придёт
    ждёт_страницу: Cell<bool>,
    итог_раскладок: RefCell<Option<gtk::Label>>,
}

type П = Rc<Окно>;

/* ---------- общение ---------- */

fn страница(что: Value) {
    скажи("параметры-дело", что);
}

fn задай(п: &П, k: &str, v: Value) {
    if let Some(s) = п.сост.borrow_mut().get_mut("s") {
        s[k] = v.clone();
    }
    страница(json!({ "что": "set", "k": k, "v": v }));
}

fn задай_kv(п: &П, k: &str, v: Value) {
    if let Some(kv) = п.сост.borrow_mut().get_mut("kv") {
        kv[k] = v.clone();
    }
    страница(json!({ "что": "kv", "k": k, "v": v }));
}

fn s(п: &П, k: &str) -> Value {
    п.сост.borrow()["s"][k].clone()
}

fn kv(п: &П, k: &str) -> Value {
    п.сост.borrow()["kv"][k].clone()
}

fn есть_страница(п: &П) -> bool {
    !п.сост.borrow()["s"].is_null()
}

/// Вызов агента, ответ которого рисуется только в том разделе, где спросили
fn спроси<F>(п: &П, метод: &str, доводы: Value, дело: F)
where
    F: FnOnce(Result<Value, String>) + 'static,
{
    let поколение = п.поколение.get();
    let п2 = Rc::downgrade(п);
    позови(метод, доводы, move |о| {
        if п2.upgrade().is_some_and(|п| п.поколение.get() == поколение) {
            дело(о);
        }
    });
}

/* ---------- виджеты ---------- */

fn ярлык(текст: &str, класс: &str) -> gtk::Label {
    let л = gtk::Label::new(Some(текст));
    л.set_xalign(0.0);
    л.set_line_wrap(true);
    л.set_line_wrap_mode(gtk::pango::WrapMode::WordChar);
    if !класс.is_empty() {
        л.style_context().add_class(класс);
    }
    л
}

fn карта(п: &П, заголовок: &str) -> gtk::Box {
    if !заголовок.is_empty() {
        п.страница.add(&ярлык(заголовок, "card-t"));
    }
    let к = gtk::Box::new(gtk::Orientation::Vertical, 0);
    к.style_context().add_class("card");
    п.страница.add(&к);
    к
}

fn ряд(значок: &str, текст: &str, подпись: &str, управление: Option<&gtk::Widget>) -> gtk::Box {
    let р = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    р.style_context().add_class("row");
    let з = gtk::Label::new(Some(значок));
    з.style_context().add_class("emo");
    з.set_valign(gtk::Align::Center);
    р.add(&з);
    let текстовая = gtk::Box::new(gtk::Orientation::Vertical, 2);
    текстовая.set_hexpand(true);
    текстовая.set_valign(gtk::Align::Center);
    текстовая.add(&ярлык(текст, "t"));
    if !подпись.is_empty() {
        текстовая.add(&ярлык(подпись, "d"));
    }
    р.add(&текстовая);
    if let Some(у) = управление {
        у.set_valign(gtk::Align::Center);
        у.set_halign(gtk::Align::End);
        р.add(у);
    }
    р
}

fn добавь(к: &gtk::Box, значок: &str, текст: &str, подпись: &str, управление: Option<&gtk::Widget>) -> gtk::Box {
    let р = ряд(значок, текст, подпись, управление);
    к.add(&р);
    р
}

fn значение(текст: &str) -> gtk::Widget {
    let л = gtk::Label::new(Some(текст));
    л.style_context().add_class("val");
    л.set_line_wrap(true);
    л.set_max_width_chars(36);
    л.set_xalign(1.0);
    л.upcast()
}

fn заметка(к: &gtk::Box, текст: &str) -> gtk::Label {
    let л = ярлык(текст, "note");
    к.add(&л);
    л
}

fn очисти(к: &gtk::Box) {
    for р in к.children() {
        к.remove(&р);
    }
}

fn переключатель(вкл: bool, дело: impl Fn(bool) + 'static) -> gtk::Widget {
    let с = gtk::Switch::new();
    с.set_active(вкл);
    с.connect_active_notify(move |с| дело(с.is_active()));
    с.upcast()
}

fn ползунок(сейчас: f64, мин: f64, макс: f64, шаг: f64, вид: impl Fn(f64) -> String + 'static, дело: impl Fn(f64) + 'static) -> gtk::Widget {
    let п = gtk::Scale::with_range(gtk::Orientation::Horizontal, мин, макс, шаг);
    п.set_value(сейчас);
    п.set_draw_value(true);
    п.set_value_pos(gtk::PositionType::Right);
    п.connect_format_value(move |_, v| вид(v));
    // Двигают — не шлём каждый шаг: только когда рука остановилась
    let ждём = Rc::new(Cell::new(0u32));
    let дело = Rc::new(дело);
    п.connect_value_changed(move |п| {
        let номер = ждём.get() + 1;
        ждём.set(номер);
        let (ждём, дело, v) = (ждём.clone(), дело.clone(), п.value());
        glib::timeout_add_local_once(Duration::from_millis(220), move || {
            if ждём.get() == номер {
                дело(v);
            }
        });
    });
    п.upcast()
}

fn выбор(варианты: &[(String, String)], сейчас: &str, дело: impl Fn(String) + 'static) -> gtk::Widget {
    let в = gtk::ComboBoxText::new();
    for (id, имя) in варианты {
        в.append(Some(id), &crate::i18n::tr(имя));
    }
    в.set_active_id(Some(сейчас));
    в.connect_changed(move |в| {
        if let Some(id) = в.active_id() {
            дело(id.to_string());
        }
    });
    в.upcast()
}

/// Несколько кнопок, из которых нажата одна
fn сегменты(варианты: &[(&str, Value)], сейчас: &Value, дело: impl Fn(Value) + 'static) -> gtk::Widget {
    let коробка = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    коробка.style_context().add_class("seg");
    коробка.style_context().add_class("linked");
    let дело = Rc::new(дело);
    let кнопки: Rc<RefCell<Vec<(gtk::Button, Value)>>> = Rc::new(RefCell::new(Vec::new()));
    for (имя, v) in варианты {
        let б = gtk::Button::with_label(имя);
        if числа_равны(v, сейчас) {
            б.style_context().add_class("on");
        }
        let (кнопки2, дело, v2) = (кнопки.clone(), дело.clone(), v.clone());
        б.connect_clicked(move |б| {
            for (к, _) in кнопки2.borrow().iter() {
                к.style_context().remove_class("on");
            }
            б.style_context().add_class("on");
            дело(v2.clone());
        });
        коробка.add(&б);
        кнопки.borrow_mut().push((б, v.clone()));
    }
    коробка.upcast()
}

fn числа_равны(а: &Value, б: &Value) -> bool {
    match (а.as_f64(), б.as_f64()) {
        (Some(x), Some(y)) => (x - y).abs() < 1e-6,
        _ => а == б,
    }
}

fn кн(текст: &str, главная: bool, дело: impl Fn(&gtk::Button) + 'static) -> gtk::Button {
    let б = gtk::Button::with_label(текст);
    if главная {
        б.style_context().add_class("accent");
    }
    б.connect_clicked(move |б| дело(б));
    б
}

fn ряд_кнопок(кнопки: &[&gtk::Button]) -> gtk::Widget {
    let р = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    for б in кнопки {
        р.add(*б);
    }
    р.upcast()
}

fn строка(в: &Value, k: &str) -> String {
    match &в[k] {
        Value::String(с) => с.clone(),
        Value::Null => String::new(),
        другое => другое.to_string(),
    }
}

fn сообщи(п: &П, заголовок: &str, текст: &str) {
    let д = gtk::MessageDialog::new(Some(&п.окно), gtk::DialogFlags::MODAL, gtk::MessageType::Warning, gtk::ButtonsType::Close, заголовок);
    д.set_secondary_text(Some(текст));
    д.run();
    unsafe { д.destroy() };
}

fn спроси_да(п: &П, заголовок: &str, текст: &str, да: &str, нет: &str) -> bool {
    let д = gtk::MessageDialog::new(Some(&п.окно), gtk::DialogFlags::MODAL, gtk::MessageType::Question, gtk::ButtonsType::None, заголовок);
    д.set_secondary_text(Some(текст));
    д.add_button(нет, gtk::ResponseType::Cancel);
    д.add_button(да, gtk::ResponseType::Accept);
    let ответ = д.run();
    unsafe { д.destroy() };
    ответ == gtk::ResponseType::Accept
}

/// Спросить строку. None — отказались
fn спроси_строку(п: &П, заголовок: &str, текст: &str, тайно: bool) -> Option<String> {
    let д = gtk::Dialog::with_buttons(
        Some(заголовок),
        Some(&п.окно),
        gtk::DialogFlags::MODAL,
        &[("Отмена", gtk::ResponseType::Cancel), ("Готово", gtk::ResponseType::Accept)],
    );
    д.set_default_response(gtk::ResponseType::Accept);
    let коробка = gtk::Box::new(gtk::Orientation::Vertical, 8);
    коробка.set_margin(16);
    коробка.add(&ярлык(текст, ""));
    let поле = gtk::Entry::new();
    поле.set_visibility(!тайно);
    поле.set_activates_default(true);
    поле.set_width_chars(32);
    коробка.add(&поле);
    д.content_area().add(&коробка);
    д.show_all();
    let ответ = д.run();
    let т = поле.text().to_string();
    unsafe { д.destroy() };
    (ответ == gtk::ResponseType::Accept).then_some(т)
}

fn беда(к: &gtk::Box, заголовок: &str, е: &str) {
    очисти(к);
    добавь(к, "⚠️", заголовок, е, None);
    к.show_all();
}

fn ждём(к: &gtk::Box, текст: &str) {
    очисти(к);
    заметка(к, текст);
    к.show_all();
}

fn размер(б: f64) -> String {
    if б >= 1073741824.0 {
        format!("{:.1} ГБ", б / 1073741824.0).replace('.', ",")
    } else {
        format!("{} МБ", (б / 1048576.0).round())
    }
}

fn перейди(п: &П, id: &'static str) {
    п.текущий.set(id);
    if let Some(н) = п.видимые.borrow().iter().position(|х| *х == id) {
        п.список.select_row(п.список.row_at_index(н as i32).as_ref());
    }
    рисуй(п);
}

fn ссылка(п: &П, к: &gtk::Box, знак: &str, текст: &str, подпись: &str, куда: &'static str) {
    let б = gtk::Button::with_label("›");
    let п2 = п.clone();
    б.connect_clicked(move |_| перейди(&п2, куда));
    добавь(к, знак, текст, подпись, Some(б.upcast_ref()));
}

/// Разделу нужен рассказ страницы, а его ещё нет
fn нужна_страница(п: &П) -> bool {
    if есть_страница(п) {
        return false;
    }
    п.ждёт_страницу.set(true);
    let к = карта(п, "");
    заметка(&к, "Жду рабочий стол…");
    true
}

/* ---------- разделы ---------- */

fn рисуй(п: &П) {
    п.поколение.set(п.поколение.get() + 1);
    п.ждёт_страницу.set(false);
    *п.итог_раскладок.borrow_mut() = None;
    очисти(&п.страница);
    let id = п.текущий.get();
    let раздел = РАЗДЕЛЫ.iter().find(|р| р.id == id).copied().unwrap_or(РАЗДЕЛЫ[0]);
    п.окно.set_title(&format!("{} — Параметры", раздел.имя));
    п.страница.add(&ярлык(&format!("{} {}", раздел.знак, раздел.имя), "h1"));
    match id {
        "home" => главная(п),
        "system" => система(п),
        "person" => персонализация(п),
        "dock" => док(п),
        "sound" => звук(п),
        "display" => дисплей(п),
        "net" => сеть(п),
        "drives" => носители(п),
        "print" => печать(п),
        "bt" => устройства(п),
        "notif" => уведомления(п),
        "apps" => приложения(п),
        "acc" => учётки(п),
        "time" => время(п),
        "keys" => клавиатура(п),
        "privacy" => приватность(п),
        "update" => обновления(п),
        _ => о_системе(п),
    }
    п.страница.show_all();
    п.прокрутка.vadjustment().set_value(0.0);
}

fn главная(п: &П) {
    let профиль = п.сост.borrow()["профиль"].clone();
    let бренд = п.сост.borrow()["бренд"].clone();
    let имя = { let и = строка(&профиль, "имя"); if и.is_empty() { glib::real_name().to_string_lossy().to_string() } else { и } };
    let герой = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    герой.style_context().add_class("hero");
    let знак = { let з = строка(&профиль, "знак"); if з.is_empty() { имя.chars().next().map(|б| б.to_string()).unwrap_or_default() } else { з } };
    let ава = gtk::Label::new(Some(&знак));
    ава.style_context().add_class("ava");
    герой.add(&ава);
    let т = gtk::Box::new(gtk::Orientation::Vertical, 2);
    т.set_valign(gtk::Align::Center);
    т.add(&ярлык(&имя, "t"));
    т.add(&ярлык(&format!("Учётная запись · {}", { let ф = строка(&бренд, "полное"); if ф.is_empty() { "GlowerOS".into() } else { ф } }), "d"));
    герой.add(&т);
    let к0 = карта(п, "");
    к0.add(&герой);

    let к = карта(п, "Быстрые действия");
    ссылка(п, &к, "🎨", "Персонализация", "Обои, цвета, темы", "person");
    ссылка(п, &к, "📶", "Сеть и Интернет", "Wi-Fi и подключение", "net");
    ссылка(п, &к, "⬆️", "Обновления", "Система и приложения", "update");
    ссылка(п, &к, "👤", "Учётные записи", "Пароль и люди", "acc");
}

fn система(п: &П) {
    let к = карта(п, "");
    ссылка(п, &к, "💡", "Дисплей", "Яркость, ночной свет", "display");
    ссылка(п, &к, "🔊", "Звук", "Громкость и аудиоустройства", "sound");
    ссылка(п, &к, "🔔", "Уведомления", "Баннеры, фокусировка", "notif");

    if !есть_страница(п) {
        нужна_страница(п);
    } else {
        let м = карта(п, "Многозадачность");
        let столов = п.сост.borrow()["столов"].clone();
        добавь(&м, "🖥️", "Виртуальные рабочие столы", "Ctrl + Alt + ← / →",
            Some(&сегменты(&[("2", json!(2)), ("3", json!(3)), ("4", json!(4))], &столов, |v| страница(json!({ "что": "столы", "n": v })))));
    }

    let пит = карта(п, "Питание");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    пит.add(&место);
    батарея(п, &место);
}

fn батарея(п: &П, место: &gtk::Box) {
    let место = место.clone();
    спроси(п, "sys.battery", json!({}), move |о| match о {
        Ok(б) => {
            очисти(&место);
            if б["present"].as_bool() != Some(true) {
                let сеть = б["ac"]["online"].as_bool() == Some(true);
                добавь(&место, "🔌", if сеть { "Питание от сети" } else { "Батарея не обнаружена" },
                    if сеть { "Машина работает от розетки" } else { "В этой машине нет батареи" }, None);
            } else {
                let заряжается = б["charging"].as_bool() == Some(true);
                let уровень = б["battery"]["level"].as_f64().map(|у| format!("{у:.0}%")).unwrap_or_else(|| "—".into());
                добавь(&место, if заряжается { "🔌" } else { "🔋" }, "Батарея", if заряжается { "Заряжается" } else { "Работа от батареи" },
                    Some(&значение(&уровень)));
            }
            место.show_all();
        }
        Err(е) => беда(&место, "Не удалось опросить машину", &е),
    });
}

/// Плитка обоев: CSS-градиент страницы или картинка с диска
fn плитка_обоев(id: &str, css: Option<&str>, картинка: Option<&str>, выбрано: bool, все: &Rc<RefCell<Vec<gtk::Button>>>, дело: impl Fn() + 'static) -> gtk::Box {
    let б = gtk::Button::new();
    б.style_context().add_class("wall");
    if выбрано {
        б.style_context().add_class("on");
    }
    if let Some(css) = css {
        let стиль = gtk::CssProvider::new();
        let правило = format!("button {{ background-image: {}; background-size: cover; }}", crate::desktop::цвета_для_gtk(css));
        if стиль.load_from_data(правило.as_bytes()).is_ok() {
            б.style_context().add_provider(&стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2);
        }
    }
    if let Some(путь) = картинка.map(String::from) {
        let б2 = б.clone();
        // Снимки с диска — после показа окна, по одному: их бывает три десятка
        glib::idle_add_local_once(move || {
            if let Ok(м) = Pixbuf::from_file_at_scale(&путь, 132, 76, false) {
                б2.set_image(Some(&gtk::Image::from_pixbuf(Some(&м))));
                б2.set_always_show_image(true);
            }
        });
    }
    let (все2, _id) = (все.clone(), id.to_string());
    б.connect_clicked(move |б| {
        for х in все2.borrow().iter() {
            х.style_context().remove_class("on");
        }
        б.style_context().add_class("on");
        дело();
    });
    все.borrow_mut().push(б.clone());
    let коробка = gtk::Box::new(gtk::Orientation::Vertical, 4);
    коробка.add(&б);
    коробка
}

fn персонализация(п: &П) {
    if нужна_страница(п) {
        return;
    }
    let сейчас = строка(&s(п, "wallpaper"), "");
    let сейчас = if сейчас.is_empty() { s(п, "wallpaper").as_str().unwrap_or("").to_string() } else { сейчас };
    let плитки: Rc<RefCell<Vec<gtk::Button>>> = Rc::new(RefCell::new(Vec::new()));

    let о = карта(п, "Фон рабочего стола");
    let сетка = gtk::FlowBox::new();
    сетка.set_selection_mode(gtk::SelectionMode::None);
    сетка.set_max_children_per_line(5);
    сетка.set_row_spacing(10);
    сетка.set_column_spacing(10);
    сетка.set_margin(14);
    for в in п.сост.borrow()["обои"].as_array().cloned().unwrap_or_default() {
        let id = строка(&в, "id");
        let п1 = п.clone();
        let id2 = id.clone();
        let плитка = плитка_обоев(&id, в["css"].as_str(), None, id == сейчас, &плитки, move || задай(&п1, "wallpaper", json!(id2)));
        плитка.add(&ярлык(&строка(&в, "имя"), "wall-name"));
        сетка.add(&плитка);
    }
    о.add(&сетка);
    let п1 = п.clone();
    добавь(&о, "🔀", "Слайд-шоу", "Менять обои каждые 30 секунд",
        Some(&переключатель(s(п, "wallShuffle").as_bool().unwrap_or(false), move |v| задай(&п1, "wallShuffle", json!(v)))));
    let п1 = п.clone();
    let свой = кн("🖼 Выбрать файл…", true, move |_| {
        let д = gtk::FileChooserDialog::with_buttons(
            Some("Своё изображение"),
            Some(&п1.окно),
            gtk::FileChooserAction::Open,
            &[("Отмена", gtk::ResponseType::Cancel), ("Выбрать", gtk::ResponseType::Accept)],
        );
        let ф = gtk::FileFilter::new();
        ф.set_name(Some("Изображения"));
        ф.add_mime_type("image/*");
        д.add_filter(ф);
        if д.run() == gtk::ResponseType::Accept {
            if let Some(путь) = д.filename() {
                задай(&п1, "wallpaper", json!(format!("файл:{}", путь.display())));
            }
        }
        unsafe { д.destroy() };
    });
    добавь(&о, "📤", "Своё изображение",
        if сейчас.starts_with("файл:") { "Сейчас стоит картинка с диска" } else { "Любая картинка с диска" },
        Some(свой.upcast_ref()));

    // Обои, лежащие в системе: GNOME, KDE — что стоит на машине
    let сис = карта(п, "Обои системы");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    сис.add(&место);
    обои_системы(п, &место, &сейчас, &плитки);

    let т = карта(п, "Цвета и тема");
    let п1 = п.clone();
    добавь(&т, "🌗", "Режим", "", Some(&сегменты(&[("Тёмная", json!("dark")), ("Светлая", json!("light"))], &s(п, "theme"), move |v| задай(&п1, "theme", v))));
    let образцы = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let свой_цвет = s(п, "accentCustom");
    let акцент = s(п, "accent").as_u64().unwrap_or(0) as usize;
    let кружки: Rc<RefCell<Vec<gtk::Button>>> = Rc::new(RefCell::new(Vec::new()));
    for (i, а) in п.сост.borrow()["акценты"].as_array().cloned().unwrap_or_default().iter().enumerate() {
        let б = gtk::Button::new();
        б.style_context().add_class("sw");
        б.set_tooltip_text(Some(&строка(а, "имя")));
        if свой_цвет.is_null() && i == акцент {
            б.style_context().add_class("on");
        }
        let стиль = gtk::CssProvider::new();
        let _ = стиль.load_from_data(format!("button {{ background-image: linear-gradient(140deg, {}, {}); }}", строка(а, "a"), строка(а, "b")).as_bytes());
        б.style_context().add_provider(&стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2);
        let кружки2 = кружки.clone();
        let п1 = п.clone();
        б.connect_clicked(move |б| {
            for х in кружки2.borrow().iter() {
                х.style_context().remove_class("on");
            }
            б.style_context().add_class("on");
            if let Some(s) = п1.сост.borrow_mut().get_mut("s") {
                s["accentCustom"] = Value::Null;
                s["accent"] = json!(i);
            }
            страница(json!({ "что": "акцент", "i": i }));
        });
        кружки.borrow_mut().push(б.clone());
        образцы.add(&б);
    }
    добавь(&т, "🎯", "Цвет акцента", "", Some(образцы.upcast_ref()));
    let цвет = gtk::ColorButton::new();
    if let Ok(rgba) = свой_цвет.as_str().unwrap_or("#3a86ff").parse::<gtk::gdk::RGBA>() {
        цвет.set_rgba(&rgba);
    }
    let кружки2 = кружки.clone();
    цвет.connect_color_set(move |ц| {
        let c = ц.rgba();
        let hex = format!("#{:02x}{:02x}{:02x}", (c.red() * 255.0) as u8, (c.green() * 255.0) as u8, (c.blue() * 255.0) as u8);
        for х in кружки2.borrow().iter() {
            х.style_context().remove_class("on");
        }
        страница(json!({ "что": "акцент-свой", "цвет": hex }));
    });
    добавь(&т, "🖌️", "Свой цвет", "", Some(цвет.upcast_ref()));

    let в = карта(п, "Виджеты рабочего стола");
    let п1 = п.clone();
    добавь(&в, "🧩", "Показывать виджеты", "",
        Some(&переключатель(s(п, "showDeskWidgets").as_bool().unwrap_or(true), move |v| задай(&п1, "showDeskWidgets", json!(v)))));
    let ряд_в = gtk::FlowBox::new();
    ряд_в.set_selection_mode(gtk::SelectionMode::None);
    ряд_в.set_margin(10);
    let виджеты = п.сост.borrow()["виджеты"].as_array().cloned().unwrap_or_default();
    for вид in &виджеты {
        let t = строка(вид, "t");
        let п1 = п.clone();
        ряд_в.add(&кн(&format!("＋ {} {}", строка(вид, "знак"), строка(вид, "имя")), false, move |_| {
            страница(json!({ "что": "виджет+", "t": t }));
            let п2 = п1.clone();
            glib::timeout_add_local_once(Duration::from_millis(600), move || рисуй(&п2));
        }));
    }
    в.add(&ряд_в);
    for (i, w) in s(п, "deskWidgets").as_array().cloned().unwrap_or_default().iter().enumerate() {
        let t = строка(w, "t");
        let Some(вид) = виджеты.iter().find(|в| строка(в, "t") == t) else { continue };
        let п1 = п.clone();
        let убрать = кн("✖ Убрать", false, move |_| {
            страница(json!({ "что": "виджет-", "i": i }));
            let п2 = п1.clone();
            glib::timeout_add_local_once(Duration::from_millis(600), move || рисуй(&п2));
        });
        добавь(&в, &строка(вид, "знак"), &строка(вид, "имя"), "На рабочем столе", Some(убрать.upcast_ref()));
    }
}

fn обои_системы(п: &П, место: &gtk::Box, сейчас: &str, плитки: &Rc<RefCell<Vec<gtk::Button>>>) {
    ждём(место, "Смотрю, что есть на машине…");
    let (место2, п1, сейчас, плитки) = (место.clone(), п.clone(), сейчас.to_string(), плитки.clone());
    спроси(п, "sys.обои", json!({}), move |о| {
        let д = match о {
            Ok(д) => д,
            Err(е) => return беда(&место2, "Не удалось прочитать обои системы", &е),
        };
        очисти(&место2);
        let список = д["list"].as_array().cloned().unwrap_or_default();
        if список.is_empty() {
            заметка(&место2, "На машине пока нет ни одних обоев");
        }
        let сетка = gtk::FlowBox::new();
        сетка.set_selection_mode(gtk::SelectionMode::None);
        сетка.set_max_children_per_line(5);
        сетка.set_row_spacing(10);
        сетка.set_column_spacing(10);
        сетка.set_margin(14);
        for в in &список {
            let путь = строка(в, "путь");
            let id = format!("файл:{путь}");
            let превью = { let п = строка(в, "превью"); if п.is_empty() { путь.clone() } else { п } };
            let п2 = п1.clone();
            let id2 = id.clone();
            let плитка = плитка_обоев(&id, None, Some(&превью), id == сейчас, &плитки, move || задай(&п2, "wallpaper", json!(id2)));
            плитка.add(&ярлык(&format!("{} · {}", строка(в, "откуда"), строка(в, "имя")), "wall-name"));
            сетка.add(&плитка);
        }
        if !список.is_empty() {
            место2.add(&сетка);
        }
        let сколько = &д["сколько"];
        let низ = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        низ.set_margin(12);
        for (кто, подпись, пакет, вес) in [("GNOME", "Обои GNOME", "gnome-backgrounds", "30 МБ"), ("KDE", "Обои KDE", "plasma-workspace-wallpapers", "90 МБ")] {
            if сколько[кто].as_u64().unwrap_or(0) > 0 {
                continue;
            }
            let п2 = п1.clone();
            низ.add(&кн(&format!("⬇ {подпись} · {вес}"), false, move |б| поставь_пакет(&п2, б, пакет, подпись)));
        }
        место2.add(&низ);
        место2.show_all();
    });
}

/// Поставить пакет через агента и дождаться конца
fn поставь_пакет(п: &П, б: &gtk::Button, пакет: &str, подпись: &str) {
    б.set_sensitive(false);
    б.set_label("Ставлю…");
    let (п1, б1, подпись) = (п.clone(), б.clone(), подпись.to_string());
    позови("pkg.install", json!({ "name": пакет, "source": "apt" }), move |о| {
        if let Err(е) = о {
            б1.set_sensitive(true);
            б1.set_label(&format!("⬇ {подпись}"));
            return сообщи(&п1, "Не вышло поставить", &е);
        }
        следи_за_работой(&п1, move |п, итог| {
            if let Err(е) = итог {
                сообщи(п, "Не вышло поставить", &е);
            }
            рисуй(п);
        }, Some(б1));
    });
}

/// Ждать, пока агент закончит работу с пакетами. Шаги — в подпись кнопки.
/// Ушли из раздела — перестаём спрашивать: итог увидят, вернувшись
fn следи_за_работой(п: &П, конец: impl Fn(&П, Result<(), String>) + 'static, подпись: Option<gtk::Button>) {
    let поколение = п.поколение.get();
    let п1 = Rc::downgrade(п);
    let конец = Rc::new(конец);
    let кончено = Rc::new(Cell::new(false));
    glib::timeout_add_local(Duration::from_millis(1500), move || {
        let Some(п) = п1.upgrade() else { return glib::ControlFlow::Break };
        if кончено.get() || п.поколение.get() != поколение {
            return glib::ControlFlow::Break;
        }
        let (п2, конец, подпись, кончено2) = (п.clone(), конец.clone(), подпись.clone(), кончено.clone());
        позови("pkg.job", json!({}), move |о| {
            if кончено2.get() || п2.поколение.get() != поколение {
                return;
            }
            let j = о.unwrap_or(json!({ "running": false, "ok": true }));
            if j["running"].as_bool() == Some(true) {
                if let Some(б) = &подпись {
                    let шаг = строка(&j, "step");
                    б.set_label(&format!("{} · {}%", if шаг.is_empty() { "Работаю" } else { &шаг }, j["percent"].as_u64().unwrap_or(0)));
                }
                return;
            }
            кончено2.set(true);
            if j["ok"].as_bool() == Some(false) {
                let е = строка(&j, "error");
                конец(&п2, Err(if е.is_empty() { "система не сказала, что пошло не так".into() } else { е }));
            } else {
                конец(&п2, Ok(()));
            }
        });
        glib::ControlFlow::Continue
    });
}

fn док(п: &П) {
    if нужна_страница(п) {
        return;
    }
    let а = карта(п, "Приложения в доке");
    let коробка = gtk::FlowBox::new();
    коробка.set_selection_mode(gtk::SelectionMode::None);
    коробка.set_margin(10);
    let в_доке: Vec<String> = s(п, "dockApps").as_array().map(|с| с.iter().filter_map(|х| х.as_str().map(String::from)).collect()).unwrap_or_default();
    for пр in п.сост.borrow()["программы"].as_array().cloned().unwrap_or_default() {
        let id = строка(&пр, "id");
        let б = gtk::Button::with_label(&format!("{} {}", строка(&пр, "знак"), строка(&пр, "имя")));
        if в_доке.contains(&id) {
            б.style_context().add_class("on");
        }
        б.connect_clicked(move |б| {
            let с = б.style_context();
            if с.has_class("on") { с.remove_class("on") } else { с.add_class("on") }
            страница(json!({ "что": "список", "k": "dockApps", "id": id }));
        });
        коробка.add(&б);
    }
    а.add(&коробка);

    let д = карта(п, "Рабочий стол");
    let п1 = п.clone();
    добавь(&д, "🗂", "Значки на рабочем столе", "", Some(&переключатель(s(п, "showDesktopIcons").as_bool().unwrap_or(true), move |v| задай(&п1, "showDesktopIcons", json!(v)))));
    let п1 = п.clone();
    добавь(&д, "📊", "Виджеты на рабочем столе", "Часы, погода, заметка", Some(&переключатель(s(п, "showDeskWidgets").as_bool().unwrap_or(true), move |v| задай(&п1, "showDeskWidgets", json!(v)))));
}

fn звук(п: &П) {
    let к0 = карта(п, "Громкость");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    к0.add(&место);
    ждём(&место, "Спрашиваю систему…");
    let место2 = место.clone();
    спроси(п, "sys.volume.get", json!({}), move |о| {
        очисти(&место2);
        match о {
            Ok(д) => {
                let сейчас = д["volume"].as_f64().or(д["value"].as_f64()).unwrap_or(50.0);
                добавь(&место2, "🔊", "Общая громкость", "Звук всей машины",
                    Some(&ползунок(сейчас, 0.0, 100.0, 1.0, |v| format!("{v:.0}%"), |v| позови("sys.volume.set", json!({ "volume": v.round() }), |_| {}))));
                if let Some(без) = д["muted"].as_bool() {
                    добавь(&место2, "🔇", "Без звука", "", Some(&переключатель(без, |v| позови("sys.volume.set", json!({ "muted": v }), |_| {}))));
                }
            }
            Err(е) => {
                добавь(&место2, "🔊", "Громкостью системы управлять нечем", &е, None);
            }
        }
        место2.show_all();
    });
    if есть_страница(п) {
        let с = карта(п, "Звуки системы");
        let п1 = п.clone();
        добавь(&с, "🎚", "Звуки интерфейса", "Открытие и закрытие окон", Some(&переключатель(s(п, "sounds").as_bool().unwrap_or(false), move |v| задай(&п1, "sounds", json!(v)))));
        let п1 = п.clone();
        добавь(&с, "🔔", "Звук уведомлений", "", Some(&переключатель(s(п, "soundNotif").as_bool() != Some(false), move |v| задай(&п1, "soundNotif", json!(v)))));
        добавь(&с, "🎵", "Проверить вывод", "", Some(кн("▶ Проиграть", false, |_| страница(json!({ "что": "звук" }))).upcast_ref()));
    }
    let у = карта(п, "Аудиоустройства системы");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    у.add(&место);
    ждём(&место, "Опрос устройств…");
    let место2 = место.clone();
    спроси(п, "sys.devices", json!({}), move |о| {
        очисти(&место2);
        match о {
            Ok(д) => {
                let звук = д["sound"].as_array().cloned().unwrap_or_default();
                if звук.is_empty() {
                    добавь(&место2, "🚫", "Звуковых карт не найдено", "Ядро не сообщает ни об одной звуковой карте", None);
                }
                for з in звук {
                    добавь(&место2, "🔈", з.as_str().unwrap_or(""), "Звуковая карта системы", None);
                }
                место2.show_all();
            }
            Err(е) => беда(&место2, "Не удалось опросить машину", &е),
        }
    });
}

fn дисплей(п: &П) {
    let э = карта(п, "Экран машины");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    э.add(&место);
    экраны(п, &место);

    // Свой оконный сервер — пока проба: включается со следующего входа, и
    // если не поднимется, сеанс сам вернётся к labwc
    let ос = карта(п, "Оконный сервер");
    let п1 = п.clone();
    let сейчас_свой = std::env::var_os("GLOWER_WM_PID").is_some();
    добавь(&ос, "✨", "Новый оконный сервер (проба)",
        if сейчас_свой { "Работает сейчас: размытие под меню и панелью, анимации окон. Выключение — со следующего входа" }
        else { "Размытие под меню и панелью, анимации окон. Включится со следующего входа; если не запустится — вернётся прежний" },
        Some(&переключатель(s(п, "wm").as_str() == Some("glower"), move |v| задай(&п1, "wm", json!(if v { "glower" } else { "labwc" })))));

    let я = карта(п, "Подсветка");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    я.add(&место);
    let место2 = место.clone();
    спроси(п, "sys.brightness.get", json!({}), move |о| {
        очисти(&место2);
        match о {
            Ok(д) if !д["value"].is_null() || !д["brightness"].is_null() => {
                let сейчас = д["value"].as_f64().or(д["brightness"].as_f64()).unwrap_or(100.0);
                добавь(&место2, "☀️", "Яркость экрана", "",
                    Some(&ползунок(сейчас, 5.0, 100.0, 1.0, |v| format!("{v:.0}%"), |v| позови("sys.brightness.set", json!({ "value": v.round() }), |_| {}))));
            }
            Ok(д) => {
                добавь(&место2, "☀️", "Подсветкой управлять нечем", &{ let п = строка(&д, "reason"); if п.is_empty() { "У этого экрана нет регулируемой подсветки".into() } else { п } }, None);
            }
            Err(е) => {
                добавь(&место2, "☀️", "Подсветкой управлять нечем", &е, None);
            }
        }
        место2.show_all();
    });

    if есть_страница(п) {
        let ц = карта(п, "Цвет и тема");
        let п1 = п.clone();
        добавь(&ц, "🌙", "Ночной свет", "", Some(&переключатель(s(п, "nightLight").as_bool().unwrap_or(false), move |v| задай(&п1, "nightLight", json!(v)))));
    }
}

fn экраны(п: &П, место: &gtk::Box) {
    ждём(место, "Спрашиваю систему об экранах…");
    let (место2, п1) = (место.clone(), п.clone());
    спроси(п, "sys.screens", json!({}), move |о| {
        let д = match о {
            Ok(д) => д,
            Err(е) => return беда(&место2, "Система не ответила про экраны", &е),
        };
        очисти(&место2);
        if д["есть"].as_bool() != Some(true) {
            добавь(&место2, "🖥️", "Масштабом системы управлять нечем", &строка(&д, "почему"), None);
        }
        for экр in д["list"].as_array().cloned().unwrap_or_default() {
            let имя = строка(&экр, "имя");
            let масштаб = экр["масштаб"].as_f64().unwrap_or(1.0);
            let п2 = п1.clone();
            let имя2 = имя.clone();
            let место3 = место2.clone();
            let выбор = сегменты(
                &[("100%", json!(1)), ("125%", json!(1.25)), ("150%", json!(1.5)), ("175%", json!(1.75)), ("200%", json!(2))],
                &json!(масштаб),
                move |v| {
                    let (п3, место4) = (п2.clone(), место3.clone());
                    позови("sys.screen.scale", json!({ "экран": имя2, "масштаб": v }), move |о| match о {
                        Ok(_) => {
                            let (п4, место5) = (п3.clone(), место4.clone());
                            glib::timeout_add_local_once(Duration::from_millis(1200), move || экраны(&п4, &место5));
                        }
                        Err(е) => сообщи(&п3, "Не вышло изменить масштаб", &е),
                    });
                },
            );
            let описание = { let о = строка(&экр, "описание"); if о.is_empty() { имя.clone() } else { о } };
            let mut подпись = Vec::new();
            if !экр["ширина"].is_null() {
                подпись.push(format!("{}×{}", экр["ширина"], экр["высота"]));
            }
            if !экр["частота"].is_null() {
                подпись.push(format!("{} Гц", строка(&экр, "частота")));
            }
            подпись.push(format!("масштаб {}%", (масштаб * 100.0).round()));
            добавь(&место2, "🖥️", &описание, &подпись.join(" · "), Some(&выбор));
        }
        место2.show_all();
    });
}

fn сеть(п: &П) {
    let в = карта(п, "Wi-Fi");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    в.add(&место);
    вайфай(п, &место);

    let у = карта(п, "Сетевые устройства");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    у.add(&место);
    ждём(&место, "Опрашиваю сетевые устройства…");
    let место2 = место.clone();
    спроси(п, "sys.net", json!({}), move |о| match о {
        Ok(р) => {
            очисти(&место2);
            let устройства = р["devices"].as_array().cloned().unwrap_or_default();
            if устройства.is_empty() {
                добавь(&место2, "🚫", "Сетевых устройств не видно", &строка(&р, "reason"), None);
            }
            for д in устройства {
                let сост = строка(&д, "state");
                let подпись: Vec<String> = [строка(&д, "type"), строка(&д, "connection")].into_iter().filter(|с| !с.is_empty()).collect();
                добавь(&место2, if сост == "connected" || сост == "up" { "🔗" } else { "🔌" }, &строка(&д, "device"),
                    &if подпись.is_empty() { "сетевое устройство".into() } else { подпись.join(" · ") }, Some(&значение(&сост)));
            }
            место2.show_all();
        }
        Err(е) => беда(&место2, "Не удалось опросить машину", &е),
    });

    let ш = карта(п, "IPv6");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    ш.add(&место);
    ipv6(п, &место);

    if есть_страница(п) {
        let пг = карта(п, "Погода");
        let поле = gtk::Entry::new();
        поле.set_text(s(п, "city").as_str().unwrap_or("Москва"));
        поле.set_width_chars(16);
        let п1 = п.clone();
        поле.connect_activate(move |п2| задай(&п1, "city", json!(п2.text().to_string())));
        let п1 = п.clone();
        поле.connect_focus_out_event(move |п2, _| {
            if s(&п1, "city").as_str() != Some(п2.text().as_str()) {
                задай(&п1, "city", json!(п2.text().to_string()));
            }
            glib::Propagation::Proceed
        });
        добавь(&пг, "🏙", "Город", "Погода в виджете и в панели", Some(поле.upcast_ref()));
        добавь(&пг, "♻️", "Обновить прогноз", "", Some(кн("🔄 Обновить", false, |_| страница(json!({ "что": "погода" }))).upcast_ref()));
    }
}

fn полоски(с: u64) -> &'static str {
    match с {
        75.. => "▂▄▆█",
        50.. => "▂▄▆_",
        25.. => "▂▄__",
        _ => "▂___",
    }
}

fn вайфай(п: &П, место: &gtk::Box) {
    ждём(место, "Спрашиваю систему…");
    let (место2, п1) = (место.clone(), п.clone());
    спроси(п, "sys.wifi.state", json!({}), move |о| {
        let ст = match о {
            Ok(с) => с,
            Err(е) => return беда(&место2, "Wi-Fi недоступен", &е),
        };
        очисти(&место2);
        if ст["supported"].as_bool() != Some(true) {
            let рубильник = строка(&ст, "рубильник");
            let железо = &ст["железо"];
            if рубильник == "мягкая" {
                let (п2, место3) = (п1.clone(), место2.clone());
                let вкл = кн("Включить", true, move |б| {
                    б.set_sensitive(false);
                    let (п3, место4) = (п2.clone(), место3.clone());
                    позови("sys.wifi.radio", json!({ "on": true }), move |о| {
                        if let Err(е) = о {
                            сообщи(&п3, "Wi-Fi", &е);
                        }
                        вайфай(&п3, &место4);
                    });
                });
                добавь(&место2, "📴", "Wi-Fi выключен", "Он выключен программно — скорее всего, клавишей на клавиатуре. Карта на месте.", Some(вкл.upcast_ref()));
            } else if рубильник == "жёсткая" {
                добавь(&место2, "📴", "Wi-Fi выключен переключателем", "Его выключили на корпусе или в BIOS. Из системы включить нельзя — это сделано железом.", None);
            } else if !железо.is_null() && строка(железо, "драйвер").is_empty() {
                добавь(&место2, "🧩", "Карта есть, но система не знает, чем её включить",
                    &format!("{}. К ней нужен отдельный драйвер: у части карт он закрытый и в ядро не входит. Подключитесь кабелем и загляните в Обновления — система попробует его найти.", строка(железо, "имя")), None);
            } else if !железо.is_null() {
                добавь(&место2, "🧩", "Карта найдена, но не работает",
                    &format!("{} · драйвер: {}. Система видит её на шине, но управлять ей не может.", строка(железо, "имя"), строка(железо, "драйвер")), None);
            } else {
                добавь(&место2, "📶", "Беспроводных устройств нет",
                    &{ let п = строка(&ст, "reason"); if п.is_empty() { "На этой машине не найдено ни одного адаптера Wi-Fi".into() } else { п } }, None);
            }
            место2.show_all();
            return;
        }
        let радио = ст["radio"].as_bool() != Some(false);
        let (п2, место3) = (п1.clone(), место2.clone());
        добавь(&место2, "📶", "Wi-Fi", if радио { "Включён" } else { "Выключен" }, Some(&переключатель(радио, move |v| {
            let (п3, место4) = (п2.clone(), место3.clone());
            позови("sys.wifi.radio", json!({ "on": v }), move |о| {
                if let Err(е) = о {
                    сообщи(&п3, "Wi-Fi", &е);
                }
                вайфай(&п3, &место4);
            });
        })));
        if let Some(сейчас) = ст["active"].as_array().and_then(|а| а.iter().find(|а| строка(а, "type").contains("wireless")).cloned()) {
            let устройство = строка(&сейчас, "device");
            let (п2, место3) = (п1.clone(), место2.clone());
            let откл = кн("Отключиться", false, move |_| {
                let (п3, место4) = (п2.clone(), место3.clone());
                позови("sys.wifi.disconnect", json!({ "device": устройство }), move |о| {
                    if let Err(е) = о {
                        сообщи(&п3, "Wi-Fi", &е);
                    }
                    вайфай(&п3, &место4);
                });
            });
            добавь(&место2, "✅", &строка(&сейчас, "name"), &format!("Подключено · {}", строка(&сейчас, "device")), Some(откл.upcast_ref()));
        }
        if !радио {
            место2.show_all();
            return;
        }
        let список = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let (п2, список2) = (п1.clone(), список.clone());
        добавь(&место2, "🔎", "Доступные сети", "", Some(кн("🔄 Искать сети", false, move |_| сети(&п2, &список2, true)).upcast_ref()));
        место2.add(&список);
        сети(&п1, &список, false);
        место2.show_all();
    });
}

fn сети(п: &П, список: &gtk::Box, заново: bool) {
    ждём(список, if заново { "ищу сети…" } else { "читаю список…" });
    let (список2, п1) = (список.clone(), п.clone());
    спроси(п, "sys.wifi.scan", json!({ "rescan": заново }), move |о| {
        let р = match о {
            Ok(р) => р,
            Err(е) => return беда(&список2, "Не удалось получить список", &е),
        };
        очисти(&список2);
        let найдено = р["list"].as_array().cloned().unwrap_or_default();
        if найдено.is_empty() {
            добавь(&список2, "📭", "Сетей не найдено", "", None);
        }
        for х in найдено {
            let ssid = строка(&х, "ssid");
            let активна = х["active"].as_bool() == Some(true);
            let защищена = х["secure"].as_bool() == Some(true);
            let (п2, список3) = (п1.clone(), список2.clone());
            let ssid2 = ssid.clone();
            let б = кн(if активна { "Подключено" } else { "Подключиться" }, !активна, move |б| {
                let mut пароль = String::new();
                if защищена {
                    match спроси_строку(&п2, &format!("Подключение к {ssid2}"), "Введите пароль сети", true) {
                        Some(п) => пароль = п,
                        None => return,
                    }
                }
                б.set_label("подключаюсь…");
                б.set_sensitive(false);
                let (п3, список4, б2) = (п2.clone(), список3.clone(), б.clone());
                позови("sys.wifi.connect", json!({ "ssid": ssid2, "password": пароль }), move |о| match о {
                    Ok(_) => сети(&п3, &список4, false),
                    Err(е) => {
                        б2.set_label("Подключиться");
                        б2.set_sensitive(true);
                        сообщи(&п3, "Не удалось подключиться", &е);
                    }
                });
            });
            б.set_sensitive(!активна);
            let сигнал = х["signal"].as_u64().unwrap_or(0);
            let mut подпись = format!("{} {}% · {}", полоски(сигнал), сигнал, строка(&х, "band"));
            let защита = строка(&х, "security");
            if !защита.is_empty() {
                подпись.push_str(&format!(" · {защита}"));
            }
            добавь(&список2, if защищена { "🔒" } else { "📶" }, &ssid, &подпись, Some(б.upcast_ref()));
        }
        список2.show_all();
    });
}

fn ipv6(п: &П, место: &gtk::Box) {
    ждём(место, "Спрашиваю систему…");
    let (место2, п1) = (место.clone(), п.clone());
    спроси(п, "sys.ipv6", json!({}), move |о| {
        очисти(&место2);
        let д = о.unwrap_or(Value::Null);
        if д["есть"].as_bool() != Some(true) {
            добавь(&место2, "🚫", "IPv6 в этом ядре нет", "Переключать нечего.", None);
            место2.show_all();
            return;
        }
        let вкл = д["включён"].as_bool() == Some(true);
        let (п2, место3) = (п1.clone(), место2.clone());
        добавь(&место2, "🌐", if вкл { "IPv6 включён" } else { "IPv6 выключен" },
            if вкл { "Обычное состояние: система пользуется и старой адресацией, и новой." } else { "Система пользуется только IPv4. Настройка сохранена и переживёт перезагрузку." },
            Some(&переключатель(вкл, move |v| {
                let (п3, место4) = (п2.clone(), место3.clone());
                позови("sys.ipv6.set", json!({ "включить": v }), move |о| {
                    if let Err(е) = о {
                        сообщи(&п3, "Не вышло переключить IPv6", &е);
                    }
                    ipv6(&п3, &место4);
                });
            })));
        добавь(&место2, "ℹ️", "Зачем это может понадобиться",
            "Многие VPN пропускают через себя только IPv4. Пока IPv6 включён, часть трафика идёт мимо туннеля: такие сайты не открываются, а ваш настоящий адрес виден тому, к кому вы пришли. Выключение IPv6 это лечит — но без VPN его лучше не трогать.", None);
        if !вкл && д["закреплено"].as_bool() != Some(true) {
            добавь(&место2, "⚠️", "Настройка не закреплена", "Сейчас IPv6 выключен, но после перезагрузки вернётся: файла настройки нет.", None);
        }
        место2.show_all();
    });
}

fn носители(п: &П) {
    let к0 = карта(п, "Съёмные носители");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    к0.add(&место);
    список_носителей(п, &место);
}

fn список_носителей(п: &П, место: &gtk::Box) {
    ждём(место, "Спрашиваю систему…");
    let (место2, п1) = (место.clone(), п.clone());
    спроси(п, "sys.drives", json!({}), move |о| {
        let д = match о {
            Ok(д) => д,
            Err(е) => return беда(&место2, "Система не ответила про носители", &е),
        };
        очисти(&место2);
        if д["есть"].as_bool() != Some(true) {
            добавь(&место2, "💾", "Система не смогла перечислить носители", &строка(&д, "почему"), None);
        }
        let список = д["list"].as_array().cloned().unwrap_or_default();
        if список.is_empty() && д["есть"].as_bool() == Some(true) {
            добавь(&место2, "💾", "Съёмных носителей нет", "Воткните флешку или карту памяти — она появится здесь сама", None);
        }
        for н in список {
            let dev = строка(&н, "dev");
            let где = строка(&н, "где");
            let (п2, место3, dev2) = (п1.clone(), место2.clone(), dev.clone());
            let открыть = кн(if где.is_empty() { "Подключить и открыть" } else { "Открыть" }, true, move |_| {
                let (п3, место4) = (п2.clone(), место3.clone());
                позови("sys.drive", json!({ "action": "open", "dev": dev2 }), move |о| match о {
                    Ok(_) => список_носителей(&п3, &место4),
                    Err(е) => сообщи(&п3, "Не удалось открыть", &е),
                });
            });
            let mut кнопки = vec![открыть];
            if !где.is_empty() {
                let (п2, место3, dev2) = (п1.clone(), место2.clone(), dev.clone());
                кнопки.push(кн("Отключить", false, move |_| {
                    if !спроси_да(&п2, "Отключить носитель?", "Система допишет всё начатое, и флешку можно будет вынимать.", "Отключить", "Отмена") {
                        return;
                    }
                    let (п3, место4) = (п2.clone(), место3.clone());
                    позови("sys.drive", json!({ "action": "unmount", "dev": dev2 }), move |о| match о {
                        Ok(_) => {
                            сообщи(&п3, "Можно вынимать", "Носитель отключён.");
                            список_носителей(&п3, &место4);
                        }
                        Err(е) => сообщи(&п3, "Не удалось отключить", &е),
                    });
                }));
            }
            let заголовок = [строка(&н, "метка"), строка(&н, "модель"), dev.clone()].into_iter().find(|с| !с.is_empty()).unwrap_or_default();
            let фс = { let ф = строка(&н, "файловая"); if ф.is_empty() { "без файловой системы".into() } else { ф } };
            let подпись = format!("{} · {} · {}", размер(н["размер"].as_f64().unwrap_or(0.0)), фс, if где.is_empty() { "не подключён".into() } else { format!("подключён: {где}") });
            let ссылки: Vec<&gtk::Button> = кнопки.iter().collect();
            добавь(&место2, "💾", &заголовок, &подпись, Some(&ряд_кнопок(&ссылки)));
        }
        место2.show_all();
    });
}

fn печать(п: &П) {
    let ск = карта(п, "Сканирование");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    ск.add(&место);
    ждём(&место, "Ищу сканеры…");
    let (место2, п1) = (место.clone(), п.clone());
    спроси(п, "sys.scanners", json!({}), move |о| {
        let д = match о {
            Ok(д) => д,
            Err(е) => return беда(&место2, "Система не ответила про сканеры", &е),
        };
        очисти(&место2);
        if д["есть"].as_bool() != Some(true) {
            добавь(&место2, "🖼️", "Сканировать нечем", &{ let п = строка(&д, "почему"); if п.is_empty() { "в этой сборке нет sane".into() } else { п } }, None);
            место2.show_all();
            return;
        }
        let п2 = п1.clone();
        let открыть = кн("Открыть сканирование", true, move |_| {
            let п3 = п2.clone();
            позови("sys.scan.open", json!({}), move |о| {
                if let Err(е) = о {
                    сообщи(&п3, "Не вышло открыть", &е);
                }
            });
        });
        let список = д["list"].as_array().cloned().unwrap_or_default();
        if список.is_empty() {
            добавь(&место2, "🖼️", "Сканеров не найдено", "Ни по кабелю, ни в сети. Проверьте, включён ли он, и что он в той же сети.", Some(открыть.upcast_ref()));
        } else {
            let имена: Vec<String> = список.iter().map(|у| строка(у, "имя")).collect();
            добавь(&место2, "🖼️", &format!("Сканеров найдено: {}", список.len()), &имена.join(" · "), Some(открыть.upcast_ref()));
        }
        место2.show_all();
    });

    let пч = карта(п, "Печать");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    пч.add(&место);
    принтеры(п, &место);
}

fn принтеры(п: &П, место: &gtk::Box) {
    ждём(место, "Спрашиваю систему о принтерах…");
    let (место2, п1) = (место.clone(), п.clone());
    спроси(п, "sys.printers", json!({}), move |о| {
        let д = match о {
            Ok(д) => д,
            Err(е) => return беда(&место2, "Система не ответила про принтеры", &е),
        };
        очисти(&место2);
        if д["есть"].as_bool() != Some(true) {
            добавь(&место2, "🖨️", "Печать в системе не настроена", &строка(&д, "почему"), None);
            место2.show_all();
            return;
        }
        let дело = |п: &П, место: &gtk::Box, доводы: Value, беда_текст: &'static str, потом: bool| {
            let (п2, место2) = (п.clone(), место.clone());
            позови("sys.printer", доводы, move |о| match о {
                Ok(_) => {
                    if потом {
                        let (п3, место3) = (п2.clone(), место2.clone());
                        glib::timeout_add_local_once(Duration::from_millis(900), move || принтеры(&п3, &место3));
                    }
                }
                Err(е) => сообщи(&п2, беда_текст, &е),
            });
        };
        let (п2, место3) = (п1.clone(), место2.clone());
        добавь(&место2, "➕", "Добавить или настроить принтер", "Откроется страница настройки CUPS — та же, что во всех Linux",
            Some(кн("Добавить принтер", true, move |_| дело(&п2, &место3, json!({ "action": "настройка" }), "Не вышло открыть настройку", false)).upcast_ref()));
        let список = д["list"].as_array().cloned().unwrap_or_default();
        if список.is_empty() {
            добавь(&место2, "🖨️", "Принтеров пока нет", "Подключите принтер кабелем или добавьте сетевой кнопкой выше", None);
        }
        let главный = строка(&д, "поумолчанию");
        for пр in список {
            let имя = строка(&пр, "имя");
            let (п2, место3, имя2) = (п1.clone(), место2.clone(), имя.clone());
            let проба = кн("Пробная страница", false, move |_| дело(&п2, &место3, json!({ "action": "проба", "принтер": имя2 }), "Не вышло напечатать", true));
            let mut кнопки = vec![проба];
            if главный != имя {
                let (п2, место3, имя2) = (п1.clone(), место2.clone(), имя.clone());
                кнопки.push(кн("По умолчанию", true, move |_| дело(&п2, &место3, json!({ "action": "умолчание", "принтер": имя2 }), "Не вышло", true)));
            }
            let описание = строка(&пр, "описание");
            let подпись: Vec<String> = [
                строка(&пр, "состояние"),
                if описание != имя { описание } else { String::new() },
                строка(&пр, "где"),
                if главный == имя { "основной".into() } else { String::new() },
            ]
            .into_iter()
            .filter(|с| !с.is_empty())
            .collect();
            let ссылки: Vec<&gtk::Button> = кнопки.iter().collect();
            добавь(&место2, if пр["готов"].as_bool() == Some(true) { "🖨️" } else { "⚠️" }, &имя, &подпись.join(" · "), Some(&ряд_кнопок(&ссылки)));
        }
        for з in д["задания"].as_array().cloned().unwrap_or_default() {
            let номер = з["задание"].clone();
            let (п2, место3) = (п1.clone(), место2.clone());
            let снять = кн("Снять", false, move |_| дело(&п2, &место3, json!({ "action": "снять", "задание": номер }), "Не вышло снять", true));
            let подпись: Vec<String> = [строка(&з, "кто"), строка(&з, "размер")].into_iter().filter(|с| !с.is_empty()).collect();
            добавь(&место2, "📄", &format!("В очереди: {}", строка(&з, "задание")), &подпись.join(" · "), Some(снять.upcast_ref()));
        }
        место2.show_all();
    });
}

fn устройства(п: &П) {
    let бт = карта(п, "Bluetooth");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    бт.add(&место);
    блютус(п, &место, false);

    let м = карта(п, "Камеры и звук");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    м.add(&место);
    let место2 = место.clone();
    спроси(п, "sys.devices", json!({}), move |о| match о {
        Ok(д) => {
            очисти(&место2);
            let камеры = д["cameras"].as_array().cloned().unwrap_or_default();
            if камеры.is_empty() {
                добавь(&место2, "📷", "Камеры", "В системе не найдено ни одной", Some(&значение("0")));
            }
            for к2 in камеры {
                добавь(&место2, "📷", &строка(&к2, "name"), &format!("Камера · {}", строка(&к2, "id")), None);
            }
            let звук = д["sound"].as_array().cloned().unwrap_or_default();
            if звук.is_empty() {
                добавь(&место2, "🔈", "Звуковые карты", "В системе не найдено ни одной", Some(&значение("0")));
            }
            for з in звук {
                добавь(&место2, "🔈", з.as_str().unwrap_or(""), "Звуковая карта", None);
            }
            место2.show_all();
        }
        Err(е) => беда(&место2, "Не удалось опросить машину", &е),
    });

    let пит = карта(п, "Питание");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    пит.add(&место);
    батарея(п, &место);

    let ж = карта(п, "Оборудование");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    ж.add(&место);
    железо(п, &место);
}

fn блютус(п: &П, место: &gtk::Box, искать: bool) {
    ждём(место, if искать { "Ищу устройства вокруг…" } else { "Спрашиваю систему…" });
    let (место2, п1) = (место.clone(), п.clone());
    спроси(п, if искать { "sys.bt.scan" } else { "sys.bt" }, if искать { json!({ "секунд": 8 }) } else { json!({}) }, move |о| {
        let д = match о {
            Ok(д) => д,
            Err(е) => return беда(&место2, "Система не ответила про Bluetooth", &е),
        };
        очисти(&место2);
        if д["есть"].as_bool() != Some(true) {
            let подробно = &д["подробно"];
            let блок = &подробно["блокировка"];
            let почему = строка(&д, "почему");
            // Кнопку — только там, где она может помочь
            let слово = if блок["рубильником"].as_bool() == Some(true) || почему.contains("нет bluetoothctl") || почему.contains("адаптера") && почему.contains("не видно") {
                None
            } else if блок["программно"].as_bool() == Some(true) {
                Some("Включить")
            } else {
                Some("Попробовать включить")
            };
            let кнопка_вкл = слово.map(|слово| {
                let (п2, место3) = (п1.clone(), место2.clone());
                кн(слово, true, move |б| {
                    б.set_sensitive(false);
                    б.set_label("Включаю…");
                    let (п3, место4) = (п2.clone(), место3.clone());
                    позови("sys.bt.power", json!({ "включить": true }), move |о| {
                        if let Err(е) = о {
                            сообщи(&п3, "Не вышло включить Bluetooth", &е);
                        }
                        let (п4, место5) = (п3.clone(), место4.clone());
                        glib::timeout_add_local_once(Duration::from_millis(900), move || блютус(&п4, &место5, false));
                    });
                })
            });
            добавь(&место2, "🎧", "Bluetooth выключен", &if почему.is_empty() { "система не сказала причину".into() } else { почему }, кнопка_вкл.as_ref().map(|б| б.upcast_ref()));
            let файл = строка(подробно, "файл");
            let жел = строка(подробно, "железо");
            if !файл.is_empty() || !жел.is_empty() {
                let код = строка(подробно, "код");
                заметка(&место2, &format!("Адаптер: {}{}", if жел.is_empty() { "неизвестен" } else { &жел }, if код.is_empty() { String::new() } else { format!(" · код {код}") }));
                if !файл.is_empty() {
                    заметка(&место2, &format!("Ядру не хватает файла {файл}. Такие файлы пишет производитель железа и не разрешает их распространять, поэтому в системе его нет. Если найдёте файл с этим именем — положите его сюда: /lib/firmware/{файл} — и перезагрузитесь."));
                }
            }
            место2.show_all();
            return;
        }
        let вкл = д["включён"].as_bool() == Some(true);
        let (п2, место3) = (п1.clone(), место2.clone());
        добавь(&место2, "🔵", &{ let и = строка(&д, "имя"); if и.is_empty() { "Адаптер".into() } else { и } }, if вкл { "включён" } else { "выключен" },
            Some(&переключатель(вкл, move |v| {
                let (п3, место4) = (п2.clone(), место3.clone());
                позови("sys.bt.power", json!({ "включить": v }), move |о| {
                    if let Err(е) = о {
                        сообщи(&п3, "Не вышло", &е);
                    }
                    let (п4, место5) = (п3.clone(), место4.clone());
                    glib::timeout_add_local_once(Duration::from_millis(600), move || блютус(&п4, &место5, false));
                });
            })));
        let (п2, место3) = (п1.clone(), место2.clone());
        добавь(&место2, "📡", "Поиск устройств", "Включите на наушниках режим соединения и нажмите",
            Some(кн("🔎 Искать устройства", true, move |_| блютус(&п2, &место3, true)).upcast_ref()));
        for у in д["list"].as_array().cloned().unwrap_or_default() {
            let адрес = строка(&у, "адрес");
            let соединено = у["соединено"].as_bool() == Some(true);
            let своё = у["своё"].as_bool() == Some(true);
            let действие = if соединено { "disconnect" } else if своё { "connect" } else { "pair" };
            let (п2, место3, адрес2) = (п1.clone(), место2.clone(), адрес.clone());
            let главная = кн(if соединено { "Отключить" } else if своё { "Соединить" } else { "Подключить" }, !соединено, move |б| {
                б.set_sensitive(false);
                let (п3, место4, б2) = (п2.clone(), место3.clone(), б.clone());
                позови("sys.bt.device", json!({ "action": действие, "адрес": адрес2 }), move |о| match о {
                    Ok(_) => блютус(&п3, &место4, false),
                    Err(е) => {
                        б2.set_sensitive(true);
                        сообщи(&п3, "Не получилось", &е);
                    }
                });
            });
            let mut кнопки = vec![главная];
            if своё {
                let (п2, место3, адрес2) = (п1.clone(), место2.clone(), адрес.clone());
                кнопки.push(кн("Забыть", false, move |_| {
                    let (п3, место4) = (п2.clone(), место3.clone());
                    позови("sys.bt.device", json!({ "action": "remove", "адрес": адрес2 }), move |о| match о {
                        Ok(_) => блютус(&п3, &место4, false),
                        Err(е) => сообщи(&п3, "Не получилось", &е),
                    });
                }));
            }
            let подпись: Vec<String> = [адрес.clone(), if своё { "своё устройство".into() } else { "рядом".into() }, if соединено { "соединено".into() } else { String::new() }]
                .into_iter()
                .filter(|с| !с.is_empty())
                .collect();
            let ссылки: Vec<&gtk::Button> = кнопки.iter().collect();
            добавь(&место2, if соединено { "🎧" } else { "🔈" }, &{ let и = строка(&у, "имя"); if и.is_empty() { адрес.clone() } else { и } }, &подпись.join(" · "), Some(&ряд_кнопок(&ссылки)));
        }
        место2.show_all();
    });
}

fn склонение(n: u64, одно: &str, два: &str, много: &str) -> String {
    let (а, б) = (n % 10, n % 100);
    let слово = if а == 1 && б != 11 { одно } else if (2..=4).contains(&а) && !(12..=14).contains(&б) { два } else { много };
    format!("{n} {слово}")
}

/// Железо машины словами. Чего машина не сообщает — не пишем
fn железо(п: &П, место: &gtk::Box) {
    ждём(место, "Спрашиваю систему…");
    let место2 = место.clone();
    спроси(п, "sys.hardware", json!({}), move |о| {
        let hw = match о {
            Ok(hw) => hw,
            Err(е) => return беда(&место2, "Не удалось опросить машину", &е),
        };
        очисти(&место2);
        let гб = |б: f64| {
            let г = б / 1073741824.0;
            if г >= 1000.0 { format!("{:.1} ТБ", г / 1024.0).replace('.', ",") } else if г >= 10.0 { format!("{} ГБ", г.round()) } else { format!("{г:.1} ГБ").replace('.', ",") }
        };
        let точно = |б: f64| format!("{:.1} ГБ", б / 1073741824.0).replace('.', ",");
        let ц = &hw["cpu"];
        let mut модель = { let м = строка(ц, "model"); if м.is_empty() { "название не сообщается".into() } else { м } };
        for лишнее in ["(R)", "(TM)", "(tm)"] {
            модель = модель.replace(лишнее, "");
        }
        if let Some(i) = модель.find(" CPU @") {
            модель.truncate(i);
        }
        if let Some(i) = модель.find(" @ ") {
            модель.truncate(i);
        }
        let модель = модель.split_whitespace().collect::<Vec<_>>().join(" ");
        let mut про = Vec::new();
        if let Some(я) = ц["ядер"].as_u64() {
            про.push(склонение(я, "ядро", "ядра", "ядер"));
        }
        if let Some(п) = ц["cores"].as_u64() {
            про.push(склонение(п, "поток", "потока", "потоков"));
        }
        if let Some(ч) = ц["частота"].as_f64() {
            про.push(format!("до {:.1} ГГц", ч / 1e9).replace('.', ","));
        }
        let кэш = строка(ц, "кэш");
        if !кэш.is_empty() {
            про.push(format!("кэш {}", кэш.replace(" KB", " КБ")));
        }
        let арх = строка(ц, "arch");
        if !арх.is_empty() {
            про.push(арх);
        }
        let стр = |з: &str, к2: &str, в: &str, под: &str| {
            добавь(&место2, з, к2, под, Some(&значение(в)));
        };
        стр("⚙️", "Процессор", &модель, &про.join(" · "));
        let видео = hw["видео"].as_array().cloned().unwrap_or_default();
        for (i, в) in видео.iter().enumerate() {
            let др = строка(в, "драйвер");
            стр("🎮", &if видео.len() > 1 { format!("Видеокарта {}", i + 1) } else { "Видеокарта".into() }, &строка(в, "имя"),
                &if др.is_empty() { "драйвер не загружен".into() } else { format!("драйвер {др}") });
        }
        let пам = &hw["mem"];
        let подкачка = пам["подкачка"].as_f64().filter(|п| *п > 0.0).map(|п| format!(" · подкачка {}", точно(п))).unwrap_or_default();
        стр("🧠", "Оперативная память", &точно(пам["total"].as_f64().unwrap_or(0.0)), &format!("свободно {}{подкачка}", точно(пам["free"].as_f64().unwrap_or(0.0))));
        for д in hw["диски"].as_array().cloned().unwrap_or_default() {
            стр("💽", "Накопитель", &строка(&д, "имя"), &format!("{} · {}", гб(д["размер"].as_f64().unwrap_or(0.0)), строка(&д, "вид")));
        }
        let плата = строка(&hw, "плата");
        let машина = строка(&hw, "machine");
        if !плата.is_empty() || !машина.is_empty() {
            стр("🧩", "Материнская плата", if плата.is_empty() { &машина } else { &плата }, if плата.is_empty() { "" } else { &машина });
        }
        let биос = строка(&hw, "биос");
        if !биос.is_empty() {
            let uefi = hw["uefi"].as_bool() == Some(true);
            стр("🔧", if uefi { "UEFI" } else { "BIOS" }, &биос, if uefi { "загрузка UEFI" } else { "загрузка Legacy BIOS" });
        }
        for с in hw["сеть"].as_array().cloned().unwrap_or_default() {
            let др = строка(&с, "драйвер");
            стр("📶", "Сетевой адаптер", &строка(&с, "имя"), &if др.is_empty() { String::new() } else { format!("драйвер {др}") });
        }
        for з in hw["звук"].as_array().cloned().unwrap_or_default() {
            let др = строка(&з, "драйвер");
            стр("🔊", "Звук", &строка(&з, "имя"), &if др.is_empty() { String::new() } else { format!("драйвер {др}") });
        }
        стр("🖥", "Система", "GlowerOS", &format!("ядро Linux {}", строка(&hw, "kernel")));
        стр("🏷", "Имя машины", &строка(&hw, "host"), "");
        let работа = hw["uptime"].as_f64().unwrap_or(0.0);
        стр("⏱", "Время работы", &format!("{} ч {} мин", (работа / 3600.0).floor(), ((работа % 3600.0) / 60.0).round()), "с момента включения");
        место2.show_all();
    });
}

fn уведомления(п: &П) {
    if нужна_страница(п) {
        return;
    }
    let к0 = карта(п, "Уведомления");
    let п1 = п.clone();
    добавь(&к0, "🔔", "Уведомления", "Выключенные баннеры не появляются, но остаются в центре уведомлений",
        Some(&переключатель(kv(п, "notif").as_bool() != Some(false), move |v| задай_kv(&п1, "notif", json!(v)))));
    let п1 = п.clone();
    добавь(&к0, "🌙", "Не беспокоить", "", Some(&переключатель(s(п, "dnd").as_bool().unwrap_or(false), move |v| задай(&п1, "dnd", json!(v)))));

    let ф = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let длина = kv(п, "focusLen").as_str().unwrap_or("25").to_string();
    let п1 = п.clone();
    let варианты: Vec<(String, String)> = [("25", "25 минут"), ("50", "50 минут"), ("90", "90 минут")].iter().map(|(а, б)| (а.to_string(), б.to_string())).collect();
    let сколько = выбор(&варианты, &длина, move |v| задай_kv(&п1, "focusLen", json!(v)));
    ф.add(&сколько);
    let осталось = п.сост.borrow()["фокус"].as_u64().unwrap_or(0);
    let п1 = п.clone();
    let начать = кн(if осталось > 0 { "Прервать" } else { "Начать" }, true, move |б| {
        let идёт = б.label().map(|л| л == "Прервать").unwrap_or(false);
        let минут: u64 = if идёт { 0 } else { kv(&п1, "focusLen").as_str().unwrap_or("25").parse().unwrap_or(25) };
        страница(json!({ "что": "фокус", "минут": минут }));
        б.set_label(if идёт { "Начать" } else { "Прервать" });
    });
    ф.add(&начать);
    if осталось > 0 {
        ф.add(&значение(&format!("ещё {} мин", осталось.div_ceil(60))));
    }
    добавь(&к0, "🎯", "Фокусировка", "«Не беспокоить» на заданное время", Some(ф.upcast_ref()));
    добавь(&к0, "🧪", "Проверить", "", Some(кн("Показать", false, |_| страница(json!({ "что": "уведомление" }))).upcast_ref()));
}

fn приложения(п: &П) {
    let w = карта(п, "Программы Windows");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    w.add(&место);
    вайн(п, &место);

    if нужна_страница(п) {
        return;
    }
    let ав = карта(п, "Автозапуск");
    добавь(&ав, "🚀", "Запускать при входе", "", None);
    let коробка = gtk::FlowBox::new();
    коробка.set_selection_mode(gtk::SelectionMode::None);
    коробка.set_margin(10);
    let сами: Vec<String> = s(п, "autostart").as_array().map(|с| с.iter().filter_map(|х| х.as_str().map(String::from)).collect()).unwrap_or_default();
    for пр in п.сост.borrow()["программы"].as_array().cloned().unwrap_or_default() {
        let id = строка(&пр, "id");
        let б = gtk::Button::with_label(&format!("{} {}", строка(&пр, "знак"), строка(&пр, "имя")));
        if сами.contains(&id) {
            б.style_context().add_class("on");
        }
        б.connect_clicked(move |б| {
            let с = б.style_context();
            if с.has_class("on") { с.remove_class("on") } else { с.add_class("on") }
            страница(json!({ "что": "список", "k": "autostart", "id": id }));
        });
        коробка.add(&б);
    }
    ав.add(&коробка);

}

fn вайн(п: &П, место: &gtk::Box) {
    ждём(место, "Проверяю…");
    let (место2, п1) = (место.clone(), п.clone());
    спроси(п, "pkg.windows", json!({}), move |о| {
        let в = match о {
            Ok(в) => в,
            Err(_) => return беда(&место2, "Wine", "Агент не ответил — состояние неизвестно"),
        };
        очисти(&место2);
        let полный = в["полный"].as_bool() == Some(true);
        let wine64 = в["wine64"].as_bool() == Some(true);
        if полный {
            добавь(&место2, "🪟", "Wine", "Стоит. Программы .exe и .msi открываются двойным щелчком, а поставленные появляются в меню приложений", Some(&значение("✓ готово")));
        } else {
            let подпись = if wine64 {
                "Стоит только 64-битная половина — большинство установщиков 32-битные и не запустятся"
            } else {
                "Не стоит. Система поставит его сама при подключении к сети — или сейчас, кнопкой"
            };
            let кн = в["можно"].as_bool().filter(|м| *м).map(|_| {
                let п2 = п1.clone();
                кн(if wine64 { "Доставить" } else { "Поставить" }, true, move |б| {
                    б.set_sensitive(false);
                    let (п3, б2) = (п2.clone(), б.clone());
                    позови("pkg.windows.install", json!({}), move |о| match о {
                        Ok(_) => следи_за_работой(&п3, |п, итог| {
                            if let Err(е) = итог {
                                сообщи(п, "Не удалось поставить Wine", &е);
                            }
                            рисуй(п);
                        }, Some(б2)),
                        Err(е) => {
                            б2.set_sensitive(true);
                            сообщи(&п3, "Не удалось начать установку", &е);
                        }
                    });
                })
            });
            добавь(&место2, "🪟", "Wine", подпись, кн.as_ref().map(|б| б.upcast_ref()));
        }
        if полный || wine64 {
            программы_windows(&п1, &место2);
        }
        место2.show_all();
    });
}

fn программы_windows(п: &П, место: &gtk::Box) {
    let (место2, п1) = (место.clone(), п.clone());
    спроси(п, "pkg.windows.list", json!({}), move |о| {
        let список = о.ok().and_then(|д| д["список"].as_array().cloned()).unwrap_or_default();
        for з in &список {
            let ключ = строка(з, "ключ");
            let имя = строка(з, "имя");
            let (п2, имя2) = (п1.clone(), имя.clone());
            let кн = кн("Удалить", false, move |б| {
                if !спроси_да(&п2, &format!("Удалить «{имя2}»?"), "Откроется окно удаления самой программы — как в Windows. Следуйте ему.", "Открыть удаление", "Отмена") {
                    return;
                }
                б.set_sensitive(false);
                б.set_label("Удаляю…");
                let (п3, б2, ключ2) = (п2.clone(), б.clone(), ключ.clone());
                позови("pkg.windows.remove", json!({ "ключ": ключ2 }), move |о| {
                    if let Err(е) = о {
                        б2.set_sensitive(true);
                        б2.set_label("Удалить");
                        return сообщи(&п3, "Не удалось удалить", &е);
                    }
                    // Удаляет сама программа своим окном — ждём, пока закончит
                    let п4 = Rc::downgrade(&п3);
                    let ключ3 = ключ2.clone();
                    glib::timeout_add_local(Duration::from_secs(3), move || {
                        let Some(п5) = п4.upgrade() else { return glib::ControlFlow::Break };
                        let п6 = п5.clone();
                        let идёт = Rc::new(Cell::new(true));
                        let идёт2 = идёт.clone();
                        позови("pkg.uninstall.windows", json!({ "id": format!("windows:{ключ3}") }), move |о| {
                            if let Ok(с) = о {
                                if с["идёт"].as_bool() != Some(true) {
                                    идёт2.set(false);
                                    if п6.текущий.get() == "apps" {
                                        рисуй(&п6);
                                    }
                                }
                            }
                        });
                        let _ = идёт;
                        if п5.текущий.get() != "apps" { glib::ControlFlow::Break } else { glib::ControlFlow::Continue }
                    });
                });
            });
            добавь(&место2, "▫️", &имя, "Программа Windows", Some(кн.upcast_ref()));
        }
        let п2 = п1.clone();
        let сброс = кн("Удалить все", false, move |_| {
            if !спроси_да(&п2, "Удалить все программы Windows?", "Будет удалена папка Windows (~/.wine) целиком — со всеми поставленными программами, их настройками и файлами, которые они там хранили, — и все их ярлыки. Сам Wine останется: следующий .exe заведёт папку заново.", "Удалить всё", "Отмена") {
                return;
            }
            let п3 = п2.clone();
            позови("pkg.windows.reset", json!({}), move |о| match о {
                Ok(_) => рисуй(&п3),
                Err(е) => сообщи(&п3, "Не удалось удалить", &е),
            });
        });
        добавь(&место2, "🧹", "Удалить все программы Windows",
            if список.is_empty() { "Поставленных установщиком программ нет — здесь можно убрать и всё остальное" } else { "Если деинсталлятор программы не работает или нужно начать с чистого листа" },
            Some(сброс.upcast_ref()));
        место2.show_all();
    });
}

fn учётки(п: &П) {
    let к0 = карта(п, "Учётная запись машины");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    к0.add(&место);
    ждём(&место, "Спрашиваю систему…");
    let (место2, п1) = (место.clone(), п.clone());
    спроси(п, "sys.me", json!({}), move |о| {
        let я = match о {
            Ok(я) => я,
            Err(е) => return беда(&место2, "Система не ответила про учётную запись", &е),
        };
        очисти(&место2);
        let группы: Vec<String> = я["группы"].as_array().map(|г| г.iter().filter_map(|х| х.as_str().map(String::from)).collect()).unwrap_or_default();
        let mut подпись = Vec::new();
        let полное = строка(&я, "полное");
        if !полное.is_empty() {
            подпись.push(полное);
        }
        подпись.push(format!("дом: {}", строка(&я, "дом")));
        подпись.push(format!("групп: {}", группы.len()));
        добавь(&место2, "👤", &{ let и = строка(&я, "имя"); if и.is_empty() { "пользователь".into() } else { и } }, &подпись.join(" · "), None);
        let пароль = строка(&я, "пароль");
        let состояние = match пароль.as_str() {
            "задан" => "Пароль задан",
            "нет" => "Пароля нет — вход без спроса",
            _ => "Состояние пароля системе неизвестно",
        };
        let п2 = п1.clone();
        let задан = пароль == "задан";
        let сменить = кн(if задан { "Сменить" } else { "Задать" }, true, move |_| {
            let старый = if задан {
                match спроси_строку(&п2, "Смена пароля", "Введите нынешний пароль", true) {
                    Some(с) => с,
                    None => return,
                }
            } else {
                String::new()
            };
            let Some(новый) = спроси_строку(&п2, "Смена пароля", "Новый пароль — не короче четырёх знаков", true).filter(|н| !н.is_empty()) else { return };
            if спроси_строку(&п2, "Смена пароля", "Повторите новый пароль", true).as_deref() != Some(новый.as_str()) {
                return сообщи(&п2, "Пароли не совпали", "Введённые пароли различаются — система ничего не меняла.");
            }
            let п3 = п2.clone();
            позови("sys.passwd", json!({ "старый": старый, "новый": новый }), move |о| match о {
                Ok(_) => {
                    сообщи(&п3, "Пароль изменён", "Новым паролем система будет спрашивать при входе и на запертом экране.");
                    рисуй(&п3);
                }
                Err(е) => сообщи(&п3, "Не вышло сменить пароль", &е),
            });
        });
        добавь(&место2, "🔑", состояние, "Этим паролем система спрашивает вас при входе и когда экран заперт", Some(сменить.upcast_ref()));
        if !группы.is_empty() {
            добавь(&место2, "👥", "Группы", &группы.join(", "), None);
        }
        место2.show_all();
    });

    let л = карта(п, "Люди в системе");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    л.add(&место);
    люди(п, &место);

    if нужна_страница(п) {
        return;
    }
    let пр = карта(п, "Профиль");
    let профиль = п.сост.borrow()["профиль"].clone();
    let имя = gtk::Entry::new();
    имя.set_text(&строка(&профиль, "имя"));
    имя.set_width_chars(16);
    имя.connect_activate(|п2| страница(json!({ "что": "имя", "имя": п2.text().to_string() })));
    имя.connect_focus_out_event(|п2, _| {
        страница(json!({ "что": "имя", "имя": п2.text().to_string() }));
        glib::Propagation::Proceed
    });
    добавь(&пр, "👤", "Имя", "Как система обращается к вам", Some(имя.upcast_ref()));
    let знак = gtk::Entry::new();
    знак.set_text(&строка(&профиль, "знак"));
    знак.set_max_length(2);
    знак.set_width_chars(3);
    знак.connect_changed(|п2| страница(json!({ "что": "имя", "знак": п2.text().to_string() })));
    добавь(&пр, "🙂", "Значок", "Буква или эмодзи", Some(знак.upcast_ref()));
    let п1 = п.clone();
    let варианты: Vec<(String, String)> = [("0", "Никогда"), ("1", "1 минута"), ("5", "5 минут"), ("10", "10 минут"), ("30", "30 минут")].iter().map(|(а, б)| (а.to_string(), б.to_string())).collect();
    добавь(&пр, "🔒", "Блокировать при бездействии", "", Some(&выбор(&варианты, &kv(п, "autoLock").as_u64().unwrap_or(0).to_string(), move |v| задай_kv(&п1, "autoLock", json!(v.parse::<u64>().unwrap_or(0))))));
    let п1 = п.clone();
    добавь(&пр, "🌙", "Пароль после пробуждения", "", Some(&переключатель(s(п, "lockOnWake").as_bool() != Some(false), move |v| задай(&п1, "lockOnWake", json!(v)))));
}

fn люди(п: &П, место: &gtk::Box) {
    ждём(место, "Спрашиваю систему…");
    let (место2, п1) = (место.clone(), п.clone());
    спроси(п, "sys.users", json!({}), move |о| {
        let д = match о {
            Ok(д) => д,
            Err(е) => return беда(&место2, "Система не ответила про людей", &е),
        };
        очисти(&место2);
        let дело = |п: &П, место: &gtk::Box, доводы: Value, беда_текст: &'static str| {
            let (п2, место2) = (п.clone(), место.clone());
            позови("sys.user", доводы, move |о| match о {
                Ok(_) => люди(&п2, &место2),
                Err(е) => сообщи(&п2, беда_текст, &е),
            });
        };
        let (п2, место3) = (п1.clone(), место2.clone());
        let завести = кн("Добавить", true, move |_| {
            let Some(имя) = спроси_строку(&п2, "Новый человек", "Короткое имя для входа: маленькие латинские буквы, цифры, дефис", false).filter(|и| !и.is_empty()) else { return };
            let Some(полное) = спроси_строку(&п2, "Новый человек", "Как его зовут? Это имя увидят на экране входа", false) else { return };
            let Some(пароль) = спроси_строку(&п2, "Новый человек", "Пароль — не короче четырёх знаков", true).filter(|п| !п.is_empty()) else { return };
            let хозяин = спроси_да(&п2, "Права хозяина?", "Хозяин машины может ставить программы и менять настройки системы. Обычному человеку это не нужно.", "Сделать хозяином", "Обычный человек");
            дело(&п2, &место3, json!({ "action": "завести", "имя": имя, "пароль": пароль, "полное": полное, "хозяин": хозяин }), "Не вышло завести");
        });
        добавь(&место2, "➕", "Добавить человека", "У каждого будут свои файлы, свои настройки и свой пароль", Some(завести.upcast_ref()));
        for ч in д["list"].as_array().cloned().unwrap_or_default() {
            let имя = строка(&ч, "имя");
            let хозяин = ч["хозяин"].as_bool() == Some(true);
            let это_я = ч["это_я"].as_bool() == Some(true);
            let (п2, место3, имя2) = (п1.clone(), место2.clone(), имя.clone());
            let пароль = кн("Пароль", false, move |_| {
                let Some(н) = спроси_строку(&п2, &format!("Пароль для {имя2}"), "Новый пароль — не короче четырёх знаков", true).filter(|н| !н.is_empty()) else { return };
                дело(&п2, &место3, json!({ "action": "пароль", "имя": имя2, "пароль": н }), "Не вышло");
            });
            let (п2, место3, имя2) = (п1.clone(), место2.clone(), имя.clone());
            let права = кн(if хозяин { "Снять права" } else { "Сделать хозяином" }, false, move |_| {
                дело(&п2, &место3, json!({ "action": "хозяин", "имя": имя2, "хозяин": !хозяин }), "Не вышло");
            });
            let mut кнопки = vec![пароль, права];
            if !это_я {
                let (п2, место3, имя2, полное) = (п1.clone(), место2.clone(), имя.clone(), строка(&ч, "полное"));
                кнопки.push(кн("Удалить", false, move |_| {
                    if !спроси_да(&п2, &format!("Удалить {имя2}?"), &format!("Учётная запись «{}» будет удалена из системы.", if полное.is_empty() { &имя2 } else { &полное }), "Удалить", "Отмена") {
                        return;
                    }
                    let с_файлами = спроси_да(&п2, "Что делать с файлами?", "Удалить и домашнюю папку этого человека вместе со всем, что в ней? Если нет — папка останется на диске.", "Удалить файлы", "Оставить файлы");
                    дело(&п2, &место3, json!({ "action": "удалить", "имя": имя2, "сФайлами": с_файлами }), "Не вышло удалить");
                }));
            }
            let полное = строка(&ч, "полное");
            let заголовок = format!("{}{}", if полное.is_empty() { &имя } else { &полное }, if это_я { " — это вы" } else { "" });
            let пароль_слово = match строка(&ч, "пароль").as_str() {
                "задан" => "пароль задан",
                "нет" => "без пароля",
                _ => "",
            };
            let подпись: Vec<&str> = [имя.as_str(), if хозяин { "хозяин машины" } else { "обычный человек" }, пароль_слово].into_iter().filter(|с| !с.is_empty()).collect();
            let ссылки: Vec<&gtk::Button> = кнопки.iter().collect();
            добавь(&место2, if хозяин { "🛡️" } else { "👤" }, &заголовок, &подпись.join(" · "), Some(&ряд_кнопок(&ссылки)));
        }
        место2.show_all();
    });
}

/// Все часовые пояса, которые знает система
fn пояса() -> Vec<String> {
    let mut список: Vec<String> = std::fs::read_to_string("/usr/share/zoneinfo/zone1970.tab")
        .or_else(|_| std::fs::read_to_string("/usr/share/zoneinfo/zone.tab"))
        .unwrap_or_default()
        .lines()
        .filter(|с| !с.starts_with('#'))
        .filter_map(|с| с.split('\t').nth(2).map(String::from))
        .collect();
    список.sort();
    список.dedup();
    список.push("UTC".into());
    список
}

fn сдвиг(пояс: &str) -> String {
    glib::TimeZone::from_identifier(Some(пояс))
        .and_then(|т| glib::DateTime::now(&т).ok())
        .and_then(|д| д.format("%:z").ok())
        .map(|с| format!("UTC{с}"))
        .unwrap_or_default()
}

fn время(п: &П) {
    if нужна_страница(п) {
        return;
    }
    let к0 = карта(п, "Дата и время");
    let п1 = п.clone();
    добавь(&к0, "🕐", "24-часовой формат", "", Some(&переключатель(s(п, "clock24").as_bool() != Some(false), move |v| задай(&п1, "clock24", json!(v)))));
    let п1 = п.clone();
    добавь(&к0, "⏱", "Показывать секунды", "", Some(&переключатель(s(п, "showSeconds").as_bool().unwrap_or(false), move |v| задай(&п1, "showSeconds", json!(v)))));
    let здесь = std::fs::read_to_string("/etc/timezone").map(|т| т.trim().to_string()).unwrap_or_else(|_| "UTC".into());
    let mut варианты = vec![(String::new(), format!("Как на машине · {здесь}"))];
    варианты.extend(пояса().into_iter().map(|п2| { let с = сдвиг(&п2); (п2.clone(), format!("{} · {с}", п2.replace('_', " "))) }));
    let п1 = п.clone();
    добавь(&к0, "🌍", "Часовой пояс", "", Some(&выбор(&варианты, s(п, "tz").as_str().unwrap_or(""), move |v| задай(&п1, "tz", json!(v)))));

    let я = карта(п, "Язык и регион");
    let языки: Vec<(String, String)> = п.сост.borrow()["языки"].as_array().cloned().unwrap_or_default().iter().map(|я| (строка(я, "k"), строка(я, "имя"))).collect();
    let язык = строка(&п.сост.borrow(), "язык");
    добавь(&я, "🗣", "Язык интерфейса", if язык == "ru" { "Русский — исходный язык системы" } else { "Всё, чего нет в словаре, остаётся на русском" },
        Some(&выбор(&языки, &язык, |v| страница(json!({ "что": "язык", "k": v })))));
    let п1 = п.clone();
    let город = gtk::Entry::new();
    город.set_text(s(п, "city").as_str().unwrap_or(""));
    город.connect_activate(move |п2| задай(&п1, "city", json!(п2.text().to_string())));
    добавь(&я, "🏙", "Город для погоды", "", Some(город.upcast_ref()));

    // Раскладки: весь список системы с поиском, не больше четырёх сразу
    let кл = карта(п, "Раскладки клавиатуры");
    let выбрано: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(
        п.сост.borrow()["раскладки"].as_array().map(|с| с.iter().filter_map(|х| х.as_str().map(String::from)).collect()).unwrap_or_default(),
    ));
    let все: Vec<(String, String)> = п.сост.borrow()["все_раскладки"].as_array().cloned().unwrap_or_default().iter().map(|р| (строка(р, "id"), строка(р, "имя"))).collect();
    let имя_раскладки = { let все = все.clone(); move |id: &str| все.iter().find(|(i, _)| i == id).map(|(_, и)| и.clone()).unwrap_or_else(|| id.to_string()) };
    let сейчас = ярлык(&выбрано.borrow().iter().map(|id| имя_раскладки(id)).collect::<Vec<_>>().join(", "), "t");
    сейчас.set_margin_start(18);
    сейчас.set_margin_top(8);
    кл.add(&сейчас);
    let поиск = gtk::SearchEntry::new();
    поиск.set_margin_start(16);
    поиск.set_margin_end(16);
    поиск.set_margin_top(8);
    кл.add(&поиск);
    let список = gtk::ListBox::new();
    список.set_selection_mode(gtk::SelectionMode::None);
    let прокрутка = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    прокрутка.set_min_content_height(220);
    прокрутка.set_margin(10);
    прокрутка.add(&список);
    кл.add(&прокрутка);
    let отметки: Rc<RefCell<Vec<gtk::CheckButton>>> = Rc::new(RefCell::new(Vec::new()));
    // Выбранные — первыми
    let mut порядок = все.clone();
    порядок.sort_by_key(|(id, _)| !выбрано.borrow().contains(id));
    for (id, имя) in порядок {
        let о = gtk::CheckButton::with_label(&имя);
        о.set_active(выбрано.borrow().contains(&id));
        let (выбрано2, отметки2, сейчас2, имя_р) = (выбрано.clone(), отметки.clone(), сейчас.clone(), имя_раскладки.clone());
        let id2 = id.clone();
        о.connect_toggled(move |о| {
            let mut в = выбрано2.borrow_mut();
            if о.is_active() {
                if !в.contains(&id2) {
                    в.push(id2.clone());
                }
            } else {
                в.retain(|х| *х != id2);
            }
            let полно = в.len() >= 4;
            for х in отметки2.borrow().iter() {
                if !х.is_active() {
                    х.set_sensitive(!полно);
                }
            }
            сейчас2.set_text(&в.iter().map(|id| имя_р(id)).collect::<Vec<_>>().join(", "));
        });
        unsafe { о.set_data("поиск", format!("{} {}", id, имя.to_lowercase())) };
        отметки.borrow_mut().push(о.clone());
        список.add(&о);
    }
    {
        let полно = выбрано.borrow().len() >= 4;
        for х in отметки.borrow().iter() {
            if !х.is_active() {
                х.set_sensitive(!полно);
            }
        }
    }
    список.set_filter_func(Some(Box::new({
        let поиск = поиск.clone();
        move |р| {
            let т = поиск.text().to_lowercase();
            if т.is_empty() {
                return true;
            }
            р.child()
                .and_then(|д| unsafe { д.data::<String>("поиск").map(|у| у.as_ref().clone()) })
                .is_some_and(|с| с.contains(&т))
        }
    })));
    let список2 = список.clone();
    поиск.connect_search_changed(move |_| список2.invalidate_filter());
    let итог = ярлык("", "note");
    let выбрано2 = выбрано.clone();
    let итог2 = итог.clone();
    кл.add(&ряд("⌨️", "Применить", "Раскладки передаются системе; переключаются Alt + Shift", Some(кн("Применить", true, move |_| {
        итог2.set_text("Передаю системе…");
        страница(json!({ "что": "раскладки", "список": *выбрано2.borrow() }));
    }).upcast_ref())));
    кл.add(&итог);
    *п.итог_раскладок.borrow_mut() = Some(итог);
}

fn клавиатура(п: &П) {
    let к0 = карта(п, "Сочетания клавиш системы");
    for (з, клавиши, что) in [
        ("🪟", "Win", "Меню программ"),
        ("🗂", "Win + Tab", "Просмотр задач"),
        ("↔️", "Alt + Tab", "Переключение между окнами"),
        ("✖️", "Alt + F4", "Закрыть окно"),
        ("🧲", "Win + ← / →", "Прилипание окна"),
        ("⬆️", "Win + ↑ / ↓", "Развернуть или свернуть"),
        ("🖥", "Win + D", "Свернуть всё"),
        ("🖇", "Ctrl + Alt + ← / →", "Виртуальные рабочие столы"),
        ("📁", "Win + E", "Проводник"),
        ("📊", "Ctrl + Shift + Esc", "Диспетчер задач"),
        ("✂️", "Win + Shift + S", "Снимок области экрана"),
        ("🔒", "Win + L", "Заблокировать экран"),
        ("⌨️", "Alt + Shift", "Сменить раскладку"),
    ] {
        добавь(&к0, з, клавиши, что, None);
    }
}

fn приватность(п: &П) {
    let ст = карта(п, "Сетевая защита");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    ст.add(&место);
    стена(п, &место);

    let хр = карта(п, "Хранилище паролей");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    хр.add(&место);
    let (место2, п1) = (место.clone(), п.clone());
    спроси(п, "sys.keyring", json!({}), move |о| {
        очисти(&место2);
        let д = о.unwrap_or(Value::Null);
        if д["есть"].as_bool() != Some(true) {
            добавь(&место2, "🔐", "Хранилища нет", "Программам негде держать свои ключи — каждая заводит собственный тайник.", None);
        } else if д["работает"].as_bool() != Some(true) {
            добавь(&место2, "🔐", "Хранилище не запущено", "Служба есть, но не поднялась. Пароли программ сохранить не выйдет.", None);
        } else {
            let заперто = д["отперто"].as_bool() == Some(false);
            добавь(&место2, "🔐", if заперто { "Хранилище заперто" } else { "Хранилище работает" },
                if заперто { "Оно отпирается паролем входа. Если вы вошли без пароля, отпереть его нечем." } else { "Пароли программ — почты, мессенджеров, браузера — лежат здесь под ключом, а не открытым текстом на диске. Отпирается вашим паролем входа." },
                None);
            if заперто && д["файлВхода"].as_bool() == Some(true) {
                let п2 = п1.clone();
                let чинить = кн("Завести заново", false, move |_| {
                    if !спроси_да(&п2, "Завести хранилище заново", "Старое хранилище открыть нечем: оно заперто паролем, которого не знает ни система, ни вы. Оно будет отложено в сторону, а вход заведёт новое — уже с вашим паролем. Всё, что программы успели в нём сохранить, останется в отложенном файле и в новом не появится.", "Завести заново", "Отмена") {
                        return;
                    }
                    let п3 = п2.clone();
                    позови("sys.keyring.reset", json!({}), move |о| match о {
                        Ok(о) => сообщи(&п3, "Готово", &{ let д = строка(&о, "дальше"); if д.is_empty() { "Выйдите и войдите снова.".into() } else { д } }),
                        Err(е) => сообщи(&п3, "Не вышло", &е),
                    });
                });
                добавь(&место2, "🛠", "Хранилище не открывается вашим паролем", "Так бывает, если его завела прежняя версия системы. Пароли программ сохраняться не будут, пока это не починить.", Some(чинить.upcast_ref()));
            }
        }
        место2.show_all();
    });
}

fn стена(п: &П, место: &gtk::Box) {
    ждём(место, "Спрашиваю систему…");
    let (место2, п1) = (место.clone(), п.clone());
    спроси(п, "sys.firewall", json!({}), move |о| {
        let д = match о {
            Ok(д) => д,
            Err(е) => return беда(&место2, "Брандмауэр", &format!("Система не ответила: {е}")),
        };
        очисти(&место2);
        if д["есть"].as_bool() != Some(true) {
            добавь(&место2, "🧱", "Брандмауэра нет", "В этой сборке нет ufw — входящие соединения ничем не отсекаются.", None);
            место2.show_all();
            return;
        }
        let вкл = д["включён"].as_bool() == Some(true);
        let (п2, место3) = (п1.clone(), место2.clone());
        добавь(&место2, "🧱", if вкл { "Брандмауэр включён" } else { "Брандмауэр выключен" },
            if вкл { "Входящие соединения не пускаются внутрь, исходящие идут свободно." } else { "Машина принимает любые входящие соединения. В чужой сети это опасно." },
            Some(&переключатель(вкл, move |v| {
                let (п3, место4) = (п2.clone(), место3.clone());
                позови("sys.firewall.set", json!({ "включить": v }), move |о| {
                    if let Err(е) = о {
                        сообщи(&п3, "Брандмауэр", &е);
                    }
                    стена(&п3, &место4);
                });
            })));
        let правила: Vec<String> = д["правила"].as_array().map(|п| п.iter().filter_map(|х| х.as_str().map(String::from)).collect()).unwrap_or_default();
        if !правила.is_empty() {
            добавь(&место2, "📄", "Открытые порты", &правила.join(" · "), None);
        }
        место2.show_all();
    });
}

fn обновления(п: &П) {
    let о = карта(п, "Обновления системы");
    let п1 = п.clone();
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let место_для = место.clone();
    добавь(&о, "🔄", "Обновить списки программ", "Система спросит репозитории Ubuntu, что нового появилось", Some(кн("Обновить списки", false, move |б| {
        б.set_sensitive(false);
        let (п2, место2, б2) = (п1.clone(), место_для.clone(), б.clone());
        позови("pkg.update", json!({}), move |о| match о {
            Ok(_) => работа_обновлений(&п2, &место2),
            Err(е) => {
                б2.set_sensitive(true);
                сообщи(&п2, "Не вышло обновить списки", &е);
            }
        });
    }).upcast_ref()));
    о.add(&место);
    что_обновить(п, &место);

    let пк = карта(п, "Прошивки устройств");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let (п1, место_для) = (п.clone(), место.clone());
    добавь(&пк, "🌐", "Обновить список прошивок", "Система сходит в общую базу производителей и спросит, что вышло для этой машины", Some(кн("Спросить производителей", false, move |б| {
        б.set_sensitive(false);
        б.set_label("Спрашиваю…");
        let (п2, место2, б2) = (п1.clone(), место_для.clone(), б.clone());
        позови("sys.firmware.refresh", json!({}), move |о| {
            б2.set_sensitive(true);
            б2.set_label("Спросить производителей");
            match о {
                Ok(о) if о["ok"].as_bool() == Some(false) => сообщи(&п2, "Прошивки", &строка(&о, "почему")),
                Err(е) => сообщи(&п2, "Не вышло спросить", &е),
                _ => {}
            }
            прошивки(&п2, &место2);
        });
    }).upcast_ref()));
    пк.add(&место);
    прошивки(п, &место);
}

fn работа_обновлений(п: &П, место: &gtk::Box) {
    ждём(место, "Идёт работа…");
    let шаг = gtk::Button::with_label("Идёт работа…");
    шаг.set_sensitive(false);
    шаг.set_relief(gtk::ReliefStyle::None);
    место.add(&шаг);
    место.show_all();
    let место2 = место.clone();
    следи_за_работой(п, move |п, итог| match итог {
        Ok(()) => рисуй(п),
        Err(е) => беда(&место2, "Не вышло", &е),
    }, Some(шаг));
}

fn что_обновить(п: &П, место: &gtk::Box) {
    ждём(место, "Спрашиваю систему об обновлениях…");
    let (место2, п1) = (место.clone(), п.clone());
    спроси(п, "pkg.upgrade.check", json!({}), move |о| {
        let д = match о {
            Ok(д) => д,
            Err(е) => return беда(&место2, "Система не ответила про обновления", &е),
        };
        очисти(&место2);
        let всего = д["всего"].as_u64().unwrap_or(0);
        if let Some(когда) = д["когда"].as_i64() {
            if let Ok(т) = glib::DateTime::from_unix_local(когда / 1000).and_then(|т| т.format("%d.%m.%Y %H:%M")) {
                заметка(&место2, &format!("Списки программ читались {т}"));
            }
        }
        let держим: Vec<String> = д["удержано"].as_array().map(|а| а.iter().filter_map(|х| х.as_str().map(String::from)).collect()).unwrap_or_default();
        if !держим.is_empty() {
            добавь(&место2, "⏸", &format!("Отложено системой: {}", держим.join(", ")),
                "Этим обновлениям нужны пакеты, которых на машине ещё нет. Помогает «Обновить списки», а если не помогло — полная установка новой версии.", None);
        }
        let список = д["list"].as_array().cloned().unwrap_or_default();
        let наш = список.iter().find(|х| строка(х, "name") == "glower").cloned();
        место2.add(&ярлык("Система", "sub"));
        if всего == 0 {
            добавь(&место2, "✅", "Система обновлена", if держим.is_empty() { "Ничего нового в репозиториях для неё нет" } else { "Всё остальное обновлено" }, None);
        } else {
            let перезагрузить = кн("Обновить и перезагрузить", true, |_| страница(json!({ "что": "питание", "action": "update-reboot" })));
            let выключить = кн("Обновить и выключить", false, |_| страница(json!({ "что": "питание", "action": "update-poweroff" })));
            добавь(&место2, "⬆️", &format!("Обновлений системы: {всего}"),
                if наш.is_some() { "Среди них — новая версия GlowerOS" } else { "Ubuntu: ядро, библиотеки и программы" },
                Some(&ряд_кнопок(&[&перезагрузить, &выключить])));
            if let Some(н) = &наш {
                добавь(&место2, "✨", &format!("GlowerOS {} → {}", строка(н, "было"), строка(н, "станет")), "Оболочка, панель и внутренности системы", None);
            }
            let ядро: Vec<String> = список.iter().map(|х| строка(х, "name")).filter(|и| ["linux-image", "linux-generic", "linux-modules"].iter().any(|н| и.starts_with(н))).collect();
            if !ядро.is_empty() {
                добавь(&место2, "🧩", "Новое ядро Linux", &ядро.iter().take(3).cloned().collect::<Vec<_>>().join(", "), None);
            }
            let прочие: Vec<&Value> = список.iter().filter(|х| { let и = строка(х, "name"); и != "glower" && !["linux-image", "linux-generic", "linux-modules", "linux-headers"].iter().any(|н| и.starts_with(н)) }).collect();
            for х in прочие.iter().take(12) {
                добавь(&место2, "📦", &строка(х, "name"), &format!("{} → {}", строка(х, "было"), строка(х, "станет")), None);
            }
            if прочие.len() > 12 {
                заметка(&место2, &format!("И ещё {} пакетов Ubuntu.", прочие.len() - 12));
            }
        }
        let flathub = д["flathub"].as_array().cloned().unwrap_or_default();
        if !flathub.is_empty() {
            место2.add(&ярлык("Приложения", "sub"));
            let (п2, место3) = (п1.clone(), место2.clone());
            let сейчас = кн("Обновить сейчас", true, move |_| {
                let (п3, место4) = (п2.clone(), место3.clone());
                позови("pkg.upgrade.run", json!({ "source": "flatpak" }), move |о| match о {
                    Ok(_) => работа_обновлений(&п3, &место4),
                    Err(е) => сообщи(&п3, "Не вышло начать", &е),
                });
            });
            добавь(&место2, "🫙", &format!("Приложений с обновлениями: {}", flathub.len()), "Без перезагрузки — как в Магазине", Some(сейчас.upcast_ref()));
            for х in flathub {
                let имя = { let и = строка(&х, "имя"); if и.is_empty() { строка(&х, "name") } else { и } };
                let станет = строка(&х, "станет");
                добавь(&место2, "🫙", &имя, &if станет.is_empty() { строка(&х, "name") } else { format!("будет {станет}") }, None);
            }
        }
        место2.show_all();
    });
}

fn прошивки(п: &П, место: &gtk::Box) {
    ждём(место, "Спрашиваю систему об устройствах…");
    let (место2, п1) = (место.clone(), п.clone());
    спроси(п, "sys.firmware", json!({}), move |о| {
        let д = match о {
            Ok(д) => д,
            Err(е) => return беда(&место2, "Система не ответила про прошивки", &е),
        };
        очисти(&место2);
        if д["есть"].as_bool() != Some(true) {
            добавь(&место2, "🧩", "Обновлять прошивки нечем", &{ let п = строка(&д, "почему"); if п.is_empty() { "в этой сборке нет fwupd".into() } else { п } }, None);
            место2.show_all();
            return;
        }
        let новое = д["новое"].as_array().cloned().unwrap_or_default();
        if новое.is_empty() {
            добавь(&место2, "✅", "Новых прошивок нет", "Производители ничего не выпустили для этой машины — или список ещё не читался.", None);
        }
        for у in новое {
            let заголовок = format!("{}: {} → {}", строка(&у, "имя"), строка(&у, "было"), строка(&у, "станет"));
            let (п2, место3, ид, заголовок2) = (п1.clone(), место2.clone(), у["ид"].clone(), заголовок.clone());
            let б = кн("Обновить", true, move |б| {
                if !спроси_да(&п2, "Обновить прошивку?", &format!("{заголовок2}. Прошивка меняет то, что живёт в самом железе. Машину нельзя выключать, пока идёт запись."), "Обновить", "Отмена") {
                    return;
                }
                б.set_sensitive(false);
                б.set_label("Пишу…");
                let (п3, место4) = (п2.clone(), место3.clone());
                позови("sys.firmware.update", json!({ "ид": ид }), move |о| {
                    match о {
                        Ok(_) => сообщи(&п3, "Прошивка обновлена", "Некоторые прошивки встают только после перезагрузки."),
                        Err(е) => сообщи(&п3, "Не вышло обновить прошивку", &е),
                    }
                    прошивки(&п3, &место4);
                });
            });
            добавь(&место2, "🧩", &заголовок, &{ let з = строка(&у, "зачем"); if з.is_empty() { "Производитель не объяснил, что изменилось".into() } else { з } }, Some(б.upcast_ref()));
        }
        let список = д["list"].as_array().cloned().unwrap_or_default();
        if !список.is_empty() {
            let имена: Vec<String> = список.iter().take(6).map(|у| строка(у, "имя")).collect();
            заметка(&место2, &format!("Устройств, о которых знает система: {} · {}{}", список.len(), имена.join(" · "), if список.len() > 6 { " и другие" } else { "" }));
        }
        место2.show_all();
    });
}

fn логотип() -> Option<Pixbuf> {
    let рядом = std::env::current_exe().ok().and_then(|п| п.parent().map(|р| р.join("../../../assets/logo.png")));
    ["/usr/share/glower/ui/assets/logo.png".into()]
        .into_iter()
        .chain(рядом)
        .find_map(|п: std::path::PathBuf| Pixbuf::from_file_at_scale(п, 96, 96, true).ok())
}

fn о_системе(п: &П) {
    let бренд = п.сост.borrow()["бренд"].clone();
    let шапка = gtk::Box::new(gtk::Orientation::Vertical, 6);
    шапка.set_margin(20);
    if let Some(л) = логотип() {
        шапка.add(&gtk::Image::from_pixbuf(Some(&л)));
    }
    let имя = gtk::Label::new(Some(&{ let ф = строка(&бренд, "полное"); if ф.is_empty() { "GlowerOS".into() } else { ф } }));
    имя.style_context().add_class("logo");
    шапка.add(&имя);
    let версия = строка(&бренд, "версия");
    if !версия.is_empty() {
        let в = gtk::Label::new(Some(&версия));
        в.style_context().add_class("d");
        шапка.add(&в);
    }
    let к0 = карта(п, "");
    к0.add(&шапка);

    let ж = карта(п, "Характеристики устройства");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    ж.add(&место);
    железо(п, &место);

    if есть_страница(п) {
        let н = карта(п, "Начальная настройка");
        let п1 = п.clone();
        добавь(&н, "✨", "Пройти настройку заново", "Язык, раскладки, имя, тема и обои", Some(кн("Пройти заново", false, move |_| {
            if спроси_да(&п1, "Начальная настройка", "Система снова спросит язык, раскладки, имя и оформление — как при первом запуске. Файлы останутся на месте.", "Пройти заново", "Отмена") {
                страница(json!({ "что": "настройка" }));
            }
        }).upcast_ref()));
        let с = карта(п, "Сброс");
        let п1 = п.clone();
        добавь(&с, "♻️", "Настройки по умолчанию", "Файлы останутся на месте", Some(кн("Сбросить настройки", false, move |_| {
            if спроси_да(&п1, "Настройки по умолчанию", "Внешний вид, звук и поведение вернутся к исходным. Файлы останутся на месте.", "Вернуть", "Отмена") {
                страница(json!({ "что": "сброс" }));
            }
        }).upcast_ref()));
    }
}

/* ---------- окно ---------- */

fn заполни_список(п: &П, что: &str) {
    for р in п.список.children() {
        п.список.remove(&р);
    }
    let что = что.to_lowercase();
    let mut видимые = Vec::new();
    for р in РАЗДЕЛЫ {
        if !что.is_empty() && !р.имя.to_lowercase().contains(&что) && !р.слова.contains(&что) {
            continue;
        }
        let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        ряд.add(&gtk::Label::new(Some(р.знак)));
        let л = gtk::Label::new(Some(р.имя));
        л.set_xalign(0.0);
        ряд.add(&л);
        п.список.add(&ряд);
        видимые.push(р.id);
    }
    if let Some(н) = видимые.iter().position(|х| *х == п.текущий.get()) {
        п.список.select_row(п.список.row_at_index(н as i32).as_ref());
    }
    *п.видимые.borrow_mut() = видимые;
    п.список.show_all();
}

pub fn запусти(доводы: Vec<String>) -> ! {
    подготовь("glower-settings", "Параметры");
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль параметров");
    gtk::StyleContext::add_provider_for_screen(&gtk::gdk::Screen::default().expect("экран"), &стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_default_size(980, 680);
    окно.set_icon_name(Some("preferences-system"));
    окно.style_context().add_class("glower-app");
    окно.style_context().add_class("settings");

    let середина = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    окно.add(&середина);
    let бок = gtk::Box::new(gtk::Orientation::Vertical, 0);
    бок.style_context().add_class("sidebar");
    бок.set_size_request(240, -1);
    let поиск = gtk::SearchEntry::new();
    поиск.set_placeholder_text(Some("Найти параметр"));
    бок.add(&поиск);
    let список = gtk::ListBox::new();
    let прокрутка_бока = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    прокрутка_бока.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    прокрутка_бока.set_vexpand(true);
    прокрутка_бока.add(&список);
    бок.add(&прокрутка_бока);
    середина.add(&бок);

    let страница_ = gtk::Box::new(gtk::Orientation::Vertical, 0);
    страница_.style_context().add_class("page");
    страница_.set_hexpand(true);
    let прокрутка = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    прокрутка.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    прокрутка.set_hexpand(true);
    прокрутка.add(&страница_);
    середина.add(&прокрутка);

    // Раздел из доводов: glower-settings net
    let первый = доводы
        .first()
        .and_then(|д| РАЗДЕЛЫ.iter().find(|р| р.id == д.trim_start_matches("--")))
        .map(|р| р.id)
        .unwrap_or("home");

    let п: П = Rc::new(Окно {
        окно: окно.clone(),
        список: список.clone(),
        страница: страница_,
        прокрутка,
        видимые: RefCell::new(Vec::new()),
        текущий: Cell::new(первый),
        поколение: Cell::new(0),
        сост: RefCell::new(Value::Null),
        ждёт_страницу: Cell::new(false),
        итог_раскладок: RefCell::new(None),
    });
    заполни_список(&п, "");
    {
        let п1 = п.clone();
        список.connect_row_activated(move |_, р| {
            let id = п1.видимые.borrow().get(р.index() as usize).copied();
            if let Some(id) = id {
                if id != п1.текущий.get() {
                    п1.текущий.set(id);
                    рисуй(&п1);
                }
            }
        });
        let п1 = п.clone();
        поиск.connect_search_changed(move |п2| заполни_список(&п1, п2.text().trim()));
        let п1 = п.clone();
        поиск.connect_activate(move |_| {
            let первый = п1.видимые.borrow().first().copied();
            if let Some(id) = первый {
                перейди(&п1, id);
            }
        });
    }

    // Что рассказывает страница
    {
        let вести = слушай(&["параметры"], &["параметры-итог", "параметры-раздел"]);
        let п1 = Rc::downgrade(&п);
        glib::MainContext::default().spawn_local(async move {
            while let Ok(в) = вести.recv().await {
                let Some(п) = п1.upgrade() else { break };
                match в.тема.as_str() {
                    "параметры" => {
                        let первый_раз = !есть_страница(&п);
                        *п.сост.borrow_mut() = в.что;
                        // Раздел ждал рассказа — дорисовать; иначе не трогаем
                        // то, что человек сейчас крутит
                        if первый_раз && (п.ждёт_страницу.get() || matches!(п.текущий.get(), "home" | "about")) {
                            рисуй(&п);
                        }
                    }
                    // Просили открыть Параметры, а они уже открыты — на нужный раздел
                    "параметры-раздел" => {
                        if let Some(р) = в.что.as_str().and_then(|id| РАЗДЕЛЫ.iter().find(|р| р.id == id)) {
                            перейди(&п, р.id);
                            п.окно.present();
                        }
                    }
                    "параметры-итог" => {
                        if let Some(л) = п.итог_раскладок.borrow().as_ref() {
                            let ошибка = строка(&в.что, "ошибка");
                            л.set_text(&if !ошибка.is_empty() {
                                format!("Не удалось передать системе: {ошибка}")
                            } else if в.что["сразу"].as_bool() == Some(true) {
                                "Раскладки применены.".into()
                            } else {
                                "Раскладки сохранены — заработают со следующего входа в систему.".into()
                            });
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    окно.connect_key_press_event({
        let поиск = поиск.clone();
        move |_, е| {
            let ctrl = е.state().contains(gtk::gdk::ModifierType::CONTROL_MASK);
            if ctrl && crate::notepad::латинская(е) == "f" {
                поиск.grab_focus();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        }
    });
    окно.connect_delete_event(|_, _| {
        gtk::main_quit();
        glib::Propagation::Proceed
    });
    окно.show_all();
    рисуй(&п);
    gtk::main();
    std::process::exit(0);
}

#[cfg(test)]
mod проверки {
    use super::*;

    #[test]
    fn склоняет_по_русски() {
        assert_eq!(склонение(1, "ядро", "ядра", "ядер"), "1 ядро");
        assert_eq!(склонение(4, "ядро", "ядра", "ядер"), "4 ядра");
        assert_eq!(склонение(12, "ядро", "ядра", "ядер"), "12 ядер");
        assert_eq!(склонение(22, "поток", "потока", "потоков"), "22 потока");
    }

    #[test]
    fn у_каждого_раздела_есть_рисунок() {
        let ids: Vec<&str> = РАЗДЕЛЫ.iter().map(|р| р.id).collect();
        assert_eq!(ids.len(), 18);
        assert!(ids.contains(&"about") && ids.contains(&"net") && ids.contains(&"update"));
    }

    #[test]
    fn числа_сравниваются_как_числа() {
        assert!(числа_равны(&json!(1), &json!(1.0)));
        assert!(!числа_равны(&json!(1.25), &json!(1.5)));
        assert!(числа_равны(&json!("dark"), &json!("dark")));
    }
}
