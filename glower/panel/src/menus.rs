//! Что открывают кнопки полосы: календарь, быстрые настройки, батарея и
//! питание. Слова и поля — те же, что у страничной панели (js/панель.js):
//! имена полей берём у агента, а не придумываем, — это уже стоило нам
//! панели, которая честно показывала ноль громкости.

use crate::agent::{позови, скажи};
use crate::popup::{self, Где};
use gtk::glib;
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

/// Что панель знает со слов стола.
#[derive(Default)]
pub struct Знаю {
    pub обновлений: u64,
    pub dnd: bool,
    pub ночной: bool,
    pub эконом: bool,
    pub раскладка: String,
}

thread_local! {
    pub static ЗНАЮ: RefCell<Знаю> = RefCell::new(Знаю { раскладка: "ENG".into(), ..Default::default() });
}

/// Окошко ещё на экране? Ответ агента может прийти, когда его уже закрыли.
fn жив(меню: &gtk::Box) -> bool {
    меню.toplevel().is_some_and(|о| о.is_visible())
}

// ---------------------------------------------------------------- часы

pub(crate) const МЕСЯЦЫ_ИМ: [&str; 12] = ["Январь", "Февраль", "Март", "Апрель", "Май", "Июнь", "Июль", "Август", "Сентябрь", "Октябрь", "Ноябрь", "Декабрь"];
pub(crate) const МЕСЯЦЫ: [&str; 12] = [
    "января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа", "сентября",
    "октября", "ноября", "декабря",
];
pub(crate) const ДНИ: [&str; 7] = ["понедельник", "вторник", "среда", "четверг", "пятница", "суббота", "воскресенье"];

/// Часовой пояс и вид часов — настройка стола; пока он не прислал своё,
/// показываем время машины.
#[derive(Clone, Default)]
pub struct Пояс {
    pub где: String,
    pub ч12: bool,
}

thread_local! {
    pub static ПОЯС: RefCell<Пояс> = RefCell::new(Пояс::default());
}

pub fn сейчас() -> glib::DateTime {
    let где = ПОЯС.with(|п| п.borrow().где.clone());
    if !где.is_empty() {
        if let Some(тз) = glib::TimeZone::from_identifier(Some(&где)) {
            if let Ok(т) = glib::DateTime::now(&тз) {
                return т;
            }
        }
    }
    glib::DateTime::now_local().expect("часы машины")
}

pub fn время_и_дата() -> (String, String) {
    let т = сейчас();
    let ч12 = ПОЯС.with(|п| п.borrow().ч12);
    let время = if ч12 {
        let ч = т.hour() % 12;
        format!("{:02}:{:02} {}", if ч == 0 { 12 } else { ч }, т.minute(), if т.hour() < 12 { "AM" } else { "PM" })
    } else {
        format!("{:02}:{:02}", т.hour(), т.minute())
    };
    let дата = format!("{} {}", т.day_of_month(), МЕСЯЦЫ[(т.month() as usize).clamp(1, 12) - 1]);
    (время, дата)
}

pub fn календарь(кнопка: &gtk::Button) {
    popup::открой(кнопка, "часы", Где::Посередине, 280, |меню| {
        let т = сейчас();
        let заголовок = gtk::Label::new(Some(&format!(
            "{}, {} {} {}",
            ДНИ[(т.day_of_week() as usize).clamp(1, 7) - 1],
            т.day_of_month(),
            МЕСЯЦЫ[(т.month() as usize).clamp(1, 12) - 1],
            т.year()
        )));
        заголовок.style_context().add_class("title");
        заголовок.set_xalign(0.0);
        меню.add(&заголовок);
        let к = gtk::Calendar::new();
        // Месяц в календаре GTK считается с нуля
        к.select_month((т.month() - 1) as u32, т.year() as u32);
        к.select_day(т.day_of_month() as u32);
        меню.add(&к);
    });
}

// ------------------------------------------------------- быстрые настройки

/// Плитка быстрых настроек. С `ещё` — двойная, как в Windows 11: левая
/// часть включает и выключает, правая «›» открывает список (сети, устройства)
fn плитка(ряд: &gtk::Grid, место: i32, значок: &str, имя: &str, под: &str, вкл: bool, можно: bool, ещё: Option<Box<dyn Fn()>>) -> gtk::Button {
    let ящик = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    ящик.style_context().add_class("tilebox");
    let б = gtk::Button::new();
    б.style_context().add_class("tile");
    if вкл {
        ящик.style_context().add_class("on");
        б.style_context().add_class("on");
    }
    // Значок в кружке слева — как в быстрых настройках Windows 11 и GNOME:
    // плитку узнают по значку раньше, чем прочтут подпись
    let строка = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let кружок = gtk::Image::from_icon_name(Some(значок), gtk::IconSize::Button);
    кружок.set_pixel_size(16);
    кружок.style_context().add_class("tile-icon");
    кружок.set_valign(gtk::Align::Center);
    строка.add(&кружок);
    let к = gtk::Box::new(gtk::Orientation::Vertical, 1);
    к.set_valign(gtk::Align::Center);
    let и = gtk::Label::new(Some(имя));
    и.style_context().add_class("tile-name");
    и.set_xalign(0.0);
    let п = gtk::Label::new(Some(под));
    п.style_context().add_class("tile-sub");
    п.set_xalign(0.0);
    п.set_ellipsize(gtk::pango::EllipsizeMode::End);
    п.set_max_width_chars(12);
    к.add(&и);
    к.add(&п);
    строка.add(&к);
    б.add(&строка);
    б.set_hexpand(true);
    ящик.add(&б);
    if let Some(ещё) = ещё {
        б.style_context().add_class("split");
        let с = gtk::Button::from_icon_name(Some("go-next-symbolic"), gtk::IconSize::Button);
        с.style_context().add_class("tile-more");
        с.connect_clicked(move |_| ещё());
        ящик.add(&с);
    }
    ящик.set_sensitive(можно);
    ящик.set_hexpand(true);
    ряд.attach(&ящик, место % 2, место / 2, 1, 1);
    б
}

/// Наши переключатели живут в памяти стола: меняем — говорим ему.
fn наш_переключатель(ключ: &'static str) -> bool {
    let стало = ЗНАЮ.with(|з| {
        let mut з = з.borrow_mut();
        let поле = match ключ {
            "dnd" => &mut з.dnd,
            "nightLight" => &mut з.ночной,
            _ => &mut з.эконом,
        };
        *поле = !*поле;
        *поле
    });
    скажи("настройка", json!({ "ключ": ключ, "что": стало }));
    стало
}

/// Ползунок, который меняет машину не на каждое движение, а раз в сотню
/// миллисекунд: иначе звук дёргался бы десятками вызовов подряд.
fn ползунок(меню: &gtk::Box, значок: &str, имя: &str, взять: &'static str, поле: &'static str, поставить: &'static str) {
    // Значок слева и ползунок во всю ширину — как в Windows 11; имя — в
    // подсказке, значок громкости и солнца понятен и без слов
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    ряд.style_context().add_class("slider");
    let л = gtk::Image::from_icon_name(Some(значок), gtk::IconSize::Button);
    л.set_pixel_size(16);
    л.style_context().add_class("slider-icon");
    ряд.set_tooltip_text(Some(имя));
    let ш = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    ш.set_draw_value(false);
    ш.set_sensitive(false);
    ш.set_hexpand(true);
    ряд.add(&л);
    ряд.add(&ш);
    меню.add(&ряд);

    let ждёт = Rc::new(Cell::new(false));
    let готов = Rc::new(Cell::new(false));
    {
        let ждёт = ждёт.clone();
        let готов = готов.clone();
        ш.connect_value_changed(move |ш| {
            if !готов.get() || ждёт.get() {
                return;
            }
            ждёт.set(true);
            let ждёт = ждёт.clone();
            let ш = ш.clone();
            glib::timeout_add_local_once(Duration::from_millis(120), move || {
                ждёт.set(false);
                позови(поставить, json!({ поле: ш.value().round() as i64 }), |_| {});
            });
        });
    }
    let ряд_ = ряд.clone();
    позови(взять, json!({}), move |о| match о.ok().and_then(|о| о[поле].as_f64()) {
        Some(з) => {
            ш.set_value(з);
            ш.set_sensitive(true);
            готов.set(true);
        }
        // Управлять нечем — ползунка и не показываем: тот, что двигается и
        // ничего не меняет, хуже отсутствующего
        None => ряд_.hide(),
    });
}

/// Заряд словами — тот же разбор, что Разбор.батарея у страничной панели.
pub fn заряд_словами(о: &Value) -> String {
    let есть = о["present"].as_bool() != Some(false);
    match о["battery"]["level"].as_f64() {
        Some(у) if есть => format!(
            "{}% · {}",
            у.round(),
            if о["charging"].as_bool() == Some(true) { "заряжается" } else { "от батареи" }
        ),
        _ => {
            if о["ac"]["online"].as_bool() == Some(true) {
                "от сети".into()
            } else {
                "нет батареи".into()
            }
        }
    }
}

struct WiFi {
    нет: bool,
    радио: bool,
    под: String,
}

fn разбери_wifi(о: Result<Value, String>) -> WiFi {
    let о = match о {
        Ok(о) if о["supported"].as_bool() != Some(false) && !о.is_null() => о,
        _ => return WiFi { нет: true, радио: false, под: "нет на машине".into() },
    };
    let радио = о["radio"].as_bool() == Some(true);
    let активные = о["active"].as_array().cloned().unwrap_or_default();
    let сеть = активные
        .iter()
        .find(|а| {
            let т = а["type"].as_str().or(а["тип"].as_str()).unwrap_or("");
            т.contains("wifi") || т.contains("wireless")
        })
        .or(активные.first());
    let под = if !радио {
        "выключен".to_string()
    } else if let Some(с) = сеть {
        с["name"].as_str().or(с["имя"].as_str()).unwrap_or("подключено").to_string()
    } else {
        "включён".to_string()
    };
    WiFi { нет: false, радио, под }
}

struct Bt {
    можно: bool,
    вкл: bool,
    под: String,
}

fn разбери_bt(о: Result<Value, String>) -> Bt {
    let о = match о {
        Ok(о) => о,
        Err(_) => return Bt { можно: false, вкл: false, под: "нет".into() },
    };
    if о["есть"].as_bool() == Some(false) {
        let п = &о["подробно"];
        let блок = &п["блокировка"];
        let рубильник = блок["рубильником"].as_bool() == Some(true);
        let под = if рубильник {
            "выключен кнопкой"
        } else if блок["программно"].as_bool() == Some(true) {
            "выключен"
        } else if !п["железо"].is_null() && п["железо"].as_bool() != Some(false) {
            "нет прошивки"
        } else {
            "нет на машине"
        };
        // Выключенный — не то же, что отсутствующий: программно выключенный
        // включается одним нажатием, и гасить плитку нельзя
        let почему = о["почему"].as_str().unwrap_or("");
        let можно = !рубильник && !почему.contains("нет bluetoothctl") && !почему.contains("не видно");
        return Bt { можно, вкл: false, под: под.into() };
    }
    let вкл = о["включён"].as_bool() == Some(true);
    Bt { можно: true, вкл, под: if вкл { "включён" } else { "выключен" }.into() }
}

pub fn быстрые(кнопка: &gtk::Button) {
    popup::открой(кнопка, "быстрые", Где::Справа, 340, |меню| наполни_быстрые(меню));
}

fn наполни_быстрые(меню: &gtk::Box) {
    popup::очисти(меню);
    let плитки = gtk::Grid::new();
    плитки.set_column_spacing(6);
    плитки.set_row_spacing(6);
    плитки.set_column_homogeneous(true);
    меню.add(&плитки);

    // Wi-Fi и Bluetooth спрашиваем у машины: плитка, которая щёлкает и
    // ничего не делает, хуже её отсутствия
    let место_wifi = gtk::Box::new(gtk::Orientation::Vertical, 0);
    плитки.attach(&место_wifi, 0, 0, 1, 1);
    let место_bt = gtk::Box::new(gtk::Orientation::Vertical, 0);
    плитки.attach(&место_bt, 1, 0, 1, 1);
    let сети = gtk::Box::new(gtk::Orientation::Vertical, 0);

    {
        let (меню, плитки, место_wifi, сети) = (меню.clone(), плитки.clone(), место_wifi.clone(), сети.clone());
        позови("sys.wifi.state", json!({}), move |о| {
            if !жив(&меню) {
                return;
            }
            let w = разбери_wifi(о);
            плитки.remove(&место_wifi);
            let м0 = меню.clone();
            let б = плитка(&плитки, 0, if w.радио { "network-wireless-symbolic" } else { "network-wireless-disabled-symbolic" }, "Wi-Fi", &w.под, w.радио, !w.нет, Some(Box::new(move || сети_wifi(&м0))));
            let радио = w.радио;
            let м = меню.clone();
            б.connect_clicked(move |_| {
                let м = м.clone();
                позови("sys.wifi.radio", json!({ "on": !радио, "включить": !радио }), move |_| {
                    let м = м.clone();
                    glib::timeout_add_local_once(Duration::from_millis(700), move || {
                        if жив(&м) {
                            наполни_быстрые(&м)
                        }
                    });
                });
            });
            плитки.show_all();
            сети.show_all();
        });
    }
    {
        let (меню, плитки, место_bt) = (меню.clone(), плитки.clone(), место_bt.clone());
        позови("sys.bt", json!({}), move |о| {
            if !жив(&меню) {
                return;
            }
            let b = разбери_bt(о);
            плитки.remove(&место_bt);
            let м0 = меню.clone();
            let б = плитка(&плитки, 1, if b.вкл { "bluetooth-active-symbolic" } else { "bluetooth-disabled-symbolic" }, "Bluetooth", &b.под, b.вкл, b.можно, Some(Box::new(move || устройства_bt(&м0))));
            let вкл = b.вкл;
            let м = меню.clone();
            б.connect_clicked(move |_| {
                let м = м.clone();
                позови("sys.bt.power", json!({ "включить": !вкл, "on": !вкл }), move |о| {
                    if let Err(е) = о {
                        // Отказ проговариваем: молчащий выключатель — худшее,
                        // что с ним может быть
                        popup::очисти(&м);
                        popup::тихо(&м, &format!("Не вышло включить Bluetooth: {е}"));
                        м.show_all();
                        return;
                    }
                    let м = м.clone();
                    glib::timeout_add_local_once(Duration::from_millis(900), move || {
                        if жив(&м) {
                            наполни_быстрые(&м)
                        }
                    });
                });
            });
            плитки.show_all();
        });
    }

    let (dnd, ночной) = ЗНАЮ.with(|з| {
        let з = з.borrow();
        (з.dnd, з.ночной)
    });
    for (место, ключ, значок, имя, вкл, да, нет) in [
        (2, "dnd", "notifications-disabled-symbolic", "Не беспокоить", dnd, "включено", "выключено"),
        (3, "nightLight", "night-light-symbolic", "Ночной свет", ночной, "тёплые тона", "выключен"),
    ] {
        let б = плитка(&плитки, место, значок, имя, if вкл { да } else { нет }, вкл, true, None);
        let м = меню.clone();
        б.connect_clicked(move |_| {
            наш_переключатель(ключ);
            наполни_быстрые(&м);
            м.show_all();
        });
    }
    меню.add(&сети);

    // Что играет — у любого проигрывателя, по стандарту MPRIS
    if let Some(и) = crate::media::что_играет() {
        let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        ряд.style_context().add_class("now-playing");
        let текст = gtk::Box::new(gtk::Orientation::Vertical, 0);
        текст.set_hexpand(true);
        let н = gtk::Label::new(Some(if и.название.is_empty() { "Без названия" } else { &и.название }));
        н.set_xalign(0.0);
        н.set_ellipsize(gtk::pango::EllipsizeMode::End);
        н.set_max_width_chars(24);
        текст.add(&н);
        if !и.исполнитель.is_empty() {
            let а = gtk::Label::new(Some(&и.исполнитель));
            а.style_context().add_class("small");
            а.set_xalign(0.0);
            а.set_ellipsize(gtk::pango::EllipsizeMode::End);
            а.set_max_width_chars(24);
            текст.add(&а);
        }
        ряд.add(&текст);
        for (знак, метод) in [("⏮", "Previous"), (if и.статус == "Playing" { "⏸" } else { "▶" }, "PlayPause"), ("⏭", "Next")] {
            let б = crate::apps::значок_кнопка(знак);
            let м = меню.clone();
            б.connect_clicked(move |_| {
                crate::media::нажми(метод);
                let м = м.clone();
                glib::timeout_add_local_once(Duration::from_millis(300), move || {
                    if жив(&м) {
                        наполни_быстрые(&м);
                        м.show_all();
                    }
                });
            });
            ряд.add(&б);
        }
        меню.add(&ряд);
    }

    ползунок(меню, "audio-volume-high-symbolic", "Громкость", "sys.volume.get", "volume", "sys.volume.set");
    ползунок(меню, "display-brightness-symbolic", "Яркость", "sys.brightness.get", "value", "sys.brightness.set");

    // Раскладки переключает оконный сервер (Alt+Shift) — всем программам
    // разом; здесь — какие включены и куда идти, чтобы их сменить
    let раскладка = ЗНАЮ.with(|з| з.borrow().раскладка.clone());
    let р = popup::строка(меню, &format!("{раскладка}  ·  Alt+Shift"), false, || {
        скажи("покажи", json!("раскладка"));
        popup::закрой();
    });
    со_значком(&р, "input-keyboard-symbolic");
    р.set_tooltip_text(Some("Раскладки"));

    // Нижняя строка — как в Windows 11: заряд слева, замок и параметры
    // справа круглыми кнопками
    let низ = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    низ.style_context().add_class("qs-foot");
    let батарея = gtk::Image::from_icon_name(Some("battery-good-symbolic"), gtk::IconSize::Button);
    батарея.set_pixel_size(16);
    let заряд = gtk::Label::new(Some("…"));
    заряд.set_xalign(0.0);
    заряд.set_hexpand(true);
    заряд.set_ellipsize(gtk::pango::EllipsizeMode::End);
    низ.add(&батарея);
    низ.add(&заряд);
    {
        let батарея = батарея.clone();
        позови("sys.battery", json!({}), move |о| {
            let есть = о.as_ref().map(|о| о["present"].as_bool() != Some(false) && о["battery"]["level"].is_number()).unwrap_or(false);
            let заряжается = о.as_ref().map(|о| о["charging"].as_bool() == Some(true)).unwrap_or(false);
            заряд.set_text(&о.map(|о| заряд_словами(&о)).unwrap_or_else(|_| "—".into()));
            батарея.set_from_icon_name(
                Some(if !есть { "ac-adapter-symbolic" } else if заряжается { "battery-full-charging-symbolic" } else { "battery-good-symbolic" }),
                gtk::IconSize::Button,
            );
        });
    }
    for (значок, подсказка, что) in [("system-lock-screen-symbolic", "Заблокировать", 0), ("emblem-system-symbolic", "Параметры", 1)] {
        let б = gtk::Button::from_icon_name(Some(значок), gtk::IconSize::Button);
        б.style_context().add_class("round");
        б.set_tooltip_text(Some(подсказка));
        б.connect_clicked(move |_| {
            if что == 0 {
                позови("sys.power", json!({ "action": "lock" }), |_| {});
            } else {
                скажи("открой", json!({ "вид": "приложение", "id": "settings" }));
            }
            popup::закрой();
        });
        низ.add(&б);
    }
    меню.add(&gtk::Separator::new(gtk::Orientation::Horizontal));
    меню.add(&низ);
    меню.show_all();
}

/// Значок слева от подписи кнопки-строки
fn со_значком(б: &gtk::Button, значок: &str) {
    let Some(подпись) = б.child() else { return };
    б.remove(&подпись);
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let к = gtk::Image::from_icon_name(Some(значок), gtk::IconSize::Button);
    к.set_pixel_size(16);
    ряд.add(&к);
    ряд.add(&подпись);
    б.add(&ряд);
}

// ------------------------------------------------- списки Wi-Fi и Bluetooth
//
// Как в Windows 11: стрелка «›» на плитке открывает список прямо в быстрых
// настройках. Сеть или устройство раскрывается щелчком, и под ним — кнопки:
// подключиться, отключиться, забыть; у незнакомой защищённой сети — поле
// для пароля. В Параметры ходить незачем.

thread_local! {
    /// Что раскрыто в списке: имя сети или адрес устройства
    static РАСКРЫТО: RefCell<String> = const { RefCell::new(String::new()) };
}

fn раскрыто(что: &str) -> bool {
    РАСКРЫТО.with(|р| *р.borrow() == что)
}

fn раскрой(что: &str) {
    РАСКРЫТО.with(|р| {
        let mut р = р.borrow_mut();
        *р = if *р == что { String::new() } else { что.to_string() };
    });
}

/// Шапка списка: «‹», имя, выключатель справа
fn шапка_списка(меню: &gtk::Box, имя: &str, вкл: Option<bool>, переключи: impl Fn(bool) + 'static) {
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    ряд.style_context().add_class("qs-head");
    let назад = gtk::Button::from_icon_name(Some("go-previous-symbolic"), gtk::IconSize::Button);
    назад.style_context().add_class("round");
    назад.set_tooltip_text(Some("Назад"));
    let м = меню.clone();
    назад.connect_clicked(move |_| {
        РАСКРЫТО.with(|р| р.borrow_mut().clear());
        наполни_быстрые(&м);
    });
    ряд.add(&назад);
    let з = gtk::Label::new(Some(имя));
    з.style_context().add_class("qs-title");
    з.set_xalign(0.0);
    з.set_hexpand(true);
    ряд.add(&з);
    if let Some(вкл) = вкл {
        let в = gtk::Switch::new();
        в.set_active(вкл);
        в.set_valign(gtk::Align::Center);
        в.connect_state_set(move |_, да| {
            переключи(да);
            glib::Propagation::Proceed
        });
        ряд.add(&в);
    }
    меню.add(&ряд);
}

/// Листание для длинного списка: растёт по содержимому, но не выше экрана
fn листание_списка(список: &gtk::Box) -> gtk::ScrolledWindow {
    let л = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    л.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    л.set_propagate_natural_height(true);
    л.set_max_content_height(360);
    л.add(список);
    л
}

/// Низ списка: ссылка в Параметры и «обновить»
fn низ_списка(меню: &gtk::Box, подпись: &str, раздел: &'static str, обнови: impl Fn() + 'static) {
    меню.add(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    ряд.style_context().add_class("qs-foot");
    let ссылка = gtk::Button::with_label(подпись);
    ссылка.style_context().add_class("qs-link");
    ссылка.set_hexpand(true);
    if let Some(п) = ссылка.child().and_then(|п| п.downcast::<gtk::Label>().ok()) {
        п.set_xalign(0.0);
    }
    ссылка.connect_clicked(move |_| {
        скажи("открой", json!({ "вид": "приложение", "id": "settings", "довод": { "section": раздел } }));
        popup::закрой();
    });
    ряд.add(&ссылка);
    let з = gtk::Button::from_icon_name(Some("view-refresh-symbolic"), gtk::IconSize::Button);
    з.style_context().add_class("round");
    з.set_tooltip_text(Some("Обновить"));
    з.connect_clicked(move |_| обнови());
    ряд.add(&з);
    меню.add(&ряд);
}

/// Строка списка: значок, имя, подпись; щелчок раскрывает
fn строка_списка(список: &gtk::Box, ключ: &str, значок: &str, имя: &str, под: &str, вкл: bool, перерисуй: Rc<dyn Fn()>) -> (gtk::Box, bool) {
    let ящик = gtk::Box::new(gtk::Orientation::Vertical, 6);
    ящик.style_context().add_class("qs-item");
    let открыто = раскрыто(ключ);
    if вкл {
        ящик.style_context().add_class("on");
    }
    if открыто {
        ящик.style_context().add_class("open");
    }
    let б = gtk::Button::new();
    б.style_context().add_class("qs-row");
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let з = gtk::Image::from_icon_name(Some(значок), gtk::IconSize::Button);
    з.set_pixel_size(18);
    ряд.add(&з);
    let т = gtk::Box::new(gtk::Orientation::Vertical, 1);
    т.set_valign(gtk::Align::Center);
    let и = gtk::Label::new(Some(имя));
    и.set_xalign(0.0);
    и.set_ellipsize(gtk::pango::EllipsizeMode::End);
    и.set_max_width_chars(26);
    т.add(&и);
    if !под.is_empty() {
        let п = gtk::Label::new(Some(под));
        п.style_context().add_class("qs-sub");
        п.set_xalign(0.0);
        т.add(&п);
    }
    ряд.add(&т);
    б.add(&ряд);
    let ключ = ключ.to_string();
    б.connect_clicked(move |_| {
        раскрой(&ключ);
        перерисуй();
    });
    ящик.add(&б);
    список.add(&ящик);
    (ящик, открыто)
}

fn кнопка_действия(текст: &str, главная: bool) -> gtk::Button {
    let б = gtk::Button::with_label(текст);
    б.style_context().add_class("action");
    if главная {
        б.style_context().add_class("main");
    }
    б
}

/// Ряд кнопок под раскрытой строкой, справа — как в Windows
fn ряд_действий(ящик: &gtk::Box) -> gtk::Box {
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    ряд.set_halign(gtk::Align::End);
    ряд.style_context().add_class("qs-actions");
    ящик.add(&ряд);
    ряд
}

fn значок_сигнала(сила: f64) -> &'static str {
    if сила >= 75.0 {
        "network-wireless-signal-excellent-symbolic"
    } else if сила >= 50.0 {
        "network-wireless-signal-good-symbolic"
    } else if сила >= 25.0 {
        "network-wireless-signal-ok-symbolic"
    } else {
        "network-wireless-signal-weak-symbolic"
    }
}

/// Что знаем о Wi-Fi для списка
#[derive(Default, Clone)]
struct СетиWifi {
    радио: bool,
    устройство: String,
    активная: Option<String>,
    сохранённые: Vec<String>,
    список: Vec<Value>,
}

/// Список сетей — отдельным видом внутри того же окошка.
fn сети_wifi(меню: &gtk::Box) {
    popup::очисти(меню);
    шапка_списка(меню, "Wi-Fi", None, |_| {});
    popup::тихо(меню, "Ищу сети…");
    меню.show_all();
    let м = меню.clone();
    // Сперва — что включено и к чему подключены, потом сохранённые сети,
    // потом сами сети вокруг
    позови("sys.wifi.state", json!({}), move |о| {
        if !жив(&м) {
            return;
        }
        let о = о.unwrap_or(Value::Null);
        let mut с = СетиWifi { радио: о["radio"].as_bool() == Some(true), ..Default::default() };
        с.устройство = о["devices"].as_array().and_then(|у| у.first()).and_then(|у| у["device"].as_str()).unwrap_or("").to_string();
        с.активная = о["active"].as_array().and_then(|а| {
            а.iter()
                .find(|а| а["type"].as_str().is_some_and(|т| т.contains("wireless") || т.contains("wifi")))
                .and_then(|а| а["name"].as_str().map(String::from))
        });
        let м2 = м.clone();
        позови("sys.wifi.saved", json!({}), move |о| {
            с.сохранённые = о.ok().and_then(|о| о["list"].as_array().cloned()).unwrap_or_default().iter().filter_map(|х| х.as_str().map(String::from)).collect();
            if !с.радио {
                return покажи_сети(&м2, с);
            }
            let м3 = м2.clone();
            crate::agent::позови_за("sys.wifi.scan", json!({}), 20, move |о| {
                let о = о.unwrap_or(Value::Null);
                с.список = о["list"].as_array().cloned().unwrap_or_default();
                покажи_сети(&м3, с);
            });
        });
    });
}

fn покажи_сети(меню: &gtk::Box, с: СетиWifi) {
    if !жив(меню) {
        return;
    }
    popup::очисти(меню);
    let м = меню.clone();
    шапка_списка(меню, "Wi-Fi", Some(с.радио), move |да| {
        let м = м.clone();
        позови("sys.wifi.radio", json!({ "on": да }), move |_| {
            let м = м.clone();
            glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                if жив(&м) {
                    сети_wifi(&м)
                }
            });
        });
    });
    let с = Rc::new(с);
    let перерисуй: Rc<dyn Fn()> = {
        let (м, с) = (меню.clone(), с.clone());
        Rc::new(move || покажи_сети(&м, (*с).clone()))
    };
    if !с.радио {
        popup::тихо(меню, "Wi-Fi выключен");
    } else {
        let список = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let mut сети = с.список.clone();
        // Подключённая — первой
        сети.sort_by_key(|х| х["ssid"].as_str() != с.активная.as_deref());
        for сеть in сети.iter().take(20) {
            let Some(имя) = сеть["ssid"].as_str() else { continue };
            let активна = с.активная.as_deref() == Some(имя) || сеть["active"].as_bool() == Some(true);
            let защищена = сеть["secure"].as_bool() == Some(true);
            let знакома = с.сохранённые.iter().any(|х| х == имя);
            let под = if активна {
                if защищена { "Подключено, защищено" } else { "Подключено" }
            } else if знакома {
                "Сохранена"
            } else if защищена {
                "Защищена"
            } else {
                "Открытая"
            };
            let значок = значок_сигнала(сеть["signal"].as_f64().unwrap_or(0.0));
            let (ящик, открыто) = строка_списка(&список, имя, значок, имя, &crate::i18n::tr(под), активна, перерисуй.clone());
            if !открыто {
                continue;
            }
            let итог = gtk::Label::new(None);
            итог.style_context().add_class("qs-note");
            итог.set_xalign(0.0);
            итог.set_line_wrap(true);
            итог.set_no_show_all(true);
            ящик.add(&итог);
            let ряд = ряд_действий(&ящик);
            if активна {
                let б = кнопка_действия("Отключиться", false);
                let (м, устр) = (меню.clone(), с.устройство.clone());
                б.connect_clicked(move |б| {
                    б.set_sensitive(false);
                    let м = м.clone();
                    позови("sys.wifi.disconnect", json!({ "device": устр }), move |_| {
                        if жив(&м) {
                            сети_wifi(&м)
                        }
                    });
                });
                ряд.add(&б);
            } else {
                if знакома {
                    let б = кнопка_действия("Забыть", false);
                    let (м, имя) = (меню.clone(), имя.to_string());
                    б.connect_clicked(move |_| {
                        let м = м.clone();
                        позови("sys.wifi.forget", json!({ "ssid": имя }), move |_| {
                            if жив(&м) {
                                сети_wifi(&м)
                            }
                        });
                    });
                    ряд.add(&б);
                }
                let б = кнопка_действия("Подключиться", true);
                let (м, имя, ящик2, ряд2, итог2) = (меню.clone(), имя.to_string(), ящик.clone(), ряд.clone(), итог.clone());
                let нужен_пароль = защищена && !знакома;
                б.connect_clicked(move |_| {
                    if нужен_пароль {
                        спроси_пароль(&м, &ящик2, &ряд2, &итог2, &имя, None);
                    } else {
                        подключи_wifi(&м, &ящик2, &ряд2, &итог2, &имя, None);
                    }
                });
                ряд.add(&б);
            }
        }
        if с.список.is_empty() {
            popup::тихо(&список, "Сетей не видно");
        }
        меню.add(&листание_списка(&список));
    }
    let м = меню.clone();
    низ_списка(меню, "Дополнительные параметры сети Wi-Fi", "net", move || {
        popup::очисти(&м);
        let м2 = м.clone();
        шапка_списка(&м, "Wi-Fi", None, |_| {});
        popup::тихо(&м, "Ищу сети…");
        м.show_all();
        // Заново ищем по-настоящему: nmcli пересканирует эфир
        crate::agent::позови_за("sys.wifi.scan", json!({ "rescan": true }), 25, move |_| {
            if жив(&м2) {
                сети_wifi(&м2)
            }
        });
    });
    меню.show_all();
}

/// Поле пароля под сетью — как «Введите ключ безопасности сети» в Windows
fn спроси_пароль(меню: &gtk::Box, ящик: &gtk::Box, ряд: &gtk::Box, итог: &gtk::Label, имя: &str, беда: Option<&str>) {
    for р in ряд.children() {
        ряд.remove(&р);
    }
    let поле = gtk::Entry::new();
    поле.set_visibility(false);
    поле.set_placeholder_text(Some(&crate::i18n::tr("Ключ безопасности сети")));
    поле.set_icon_from_icon_name(gtk::EntryIconPosition::Secondary, Some("view-reveal-symbolic"));
    поле.set_icon_tooltip_text(gtk::EntryIconPosition::Secondary, Some(&crate::i18n::tr("Показать символы")));
    поле.connect_icon_press(|п, _, _| п.set_visibility(!gtk::prelude::EntryExt::is_visible(п)));
    поле.set_hexpand(true);
    поле.style_context().add_class("qs-entry");
    // Поле — над кнопками, во всю ширину
    let есть = ящик.children().iter().any(|в| в.style_context().has_class("qs-entry"));
    if !есть {
        let позиция = ящик.children().len() as i32 - 1;
        ящик.add(&поле);
        ящик.reorder_child(&поле, позиция.max(1));
    }
    match беда {
        Some(т) => {
            итог.set_text(&crate::i18n::tr(т));
            итог.style_context().add_class("bad");
            итог.show();
        }
        None => итог.hide(),
    }
    let отмена = кнопка_действия("Отмена", false);
    {
        let (м, ключ) = (меню.clone(), имя.to_string());
        отмена.connect_clicked(move |_| {
            раскрой(&ключ);
            сети_wifi(&м);
        });
    }
    let далее = кнопка_действия("Далее", true);
    let ход = {
        let (м, ящик, ряд, итог, имя, поле) = (меню.clone(), ящик.clone(), ряд.clone(), итог.clone(), имя.to_string(), поле.clone());
        Rc::new(move || {
            let пароль = поле.text().to_string();
            if пароль.chars().count() < 8 {
                итог.set_text(&crate::i18n::tr("Ключ сети — не короче 8 знаков"));
                итог.style_context().add_class("bad");
                итог.show();
                return;
            }
            подключи_wifi(&м, &ящик, &ряд, &итог, &имя, Some(пароль));
        })
    };
    {
        let ход = ход.clone();
        далее.connect_clicked(move |_| ход());
    }
    поле.connect_activate(move |_| ход());
    ряд.add(&отмена);
    ряд.add(&далее);
    ящик.show_all();
    if беда.is_none() {
        итог.hide();
    }
    поле.grab_focus();
}

fn подключи_wifi(меню: &gtk::Box, ящик: &gtk::Box, ряд: &gtk::Box, итог: &gtk::Label, имя: &str, пароль: Option<String>) {
    ряд.set_sensitive(false);
    итог.style_context().remove_class("bad");
    итог.set_text(&crate::i18n::tr("Подключение…"));
    итог.show();
    let доводы = match &пароль {
        Some(п) => json!({ "ssid": имя, "password": п }),
        None => json!({ "ssid": имя }),
    };
    let (м, ящик, ряд, итог, имя) = (меню.clone(), ящик.clone(), ряд.clone(), итог.clone(), имя.to_string());
    crate::agent::позови_за("sys.wifi.connect", доводы, 50, move |о| {
        if !жив(&м) {
            return;
        }
        match о {
            Ok(_) => {
                РАСКРЫТО.with(|р| р.borrow_mut().clear());
                сети_wifi(&м);
            }
            Err(е) => {
                ряд.set_sensitive(true);
                if е.contains("ключ") {
                    let беда = if е.contains("неверный") { Some("Неверный ключ безопасности сети") } else { None };
                    спроси_пароль(&м, &ящик, &ряд, &итог, &имя, беда);
                } else {
                    итог.set_text(&format!("{}: {е}", crate::i18n::tr("Не подключилось")));
                    итог.style_context().add_class("bad");
                    итог.show();
                }
            }
        }
    });
}

/// Значок устройства Bluetooth по его виду (Icon у bluetoothctl)
fn значок_устройства(вид: &str) -> &'static str {
    match вид {
        в if в.starts_with("audio-head") || в == "audio-card" => "audio-headphones-symbolic",
        в if в.starts_with("audio") => "audio-speakers-symbolic",
        "phone" => "phone-symbolic",
        "input-mouse" | "input-tablet" => "input-mouse-symbolic",
        "input-keyboard" => "input-keyboard-symbolic",
        "input-gaming" => "input-gaming-symbolic",
        в if в.starts_with("video") => "video-display-symbolic",
        "computer" => "computer-symbolic",
        "camera-photo" | "camera-video" => "camera-photo-symbolic",
        _ => "bluetooth-symbolic",
    }
}

/// Имя устройства — или адрес вместо него. Безымянные устройства (адрес
/// через дефисы) в «новых» не показываем: их в эфире десятки, и узнать по
/// ним ничего нельзя — Windows делает так же
fn безымянное(имя: &str) -> bool {
    let ч: String = имя.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    ч.len() >= 12 && имя.chars().all(|c| c.is_ascii_hexdigit() || c == '-' || c == ':')
}

fn устройства_bt(меню: &gtk::Box) {
    popup::очисти(меню);
    шапка_списка(меню, "Bluetooth", None, |_| {});
    popup::тихо(меню, "Смотрю устройства…");
    меню.show_all();
    let м = меню.clone();
    позови("sys.bt", json!({}), move |о| {
        if жив(&м) {
            покажи_bt(&м, о.unwrap_or(Value::Null), true);
        }
    });
}

/// `искать` — запустить поиск новых устройств (он идёт несколько секунд)
fn покажи_bt(меню: &gtk::Box, о: Value, искать: bool) {
    popup::очисти(меню);
    let вкл = о["включён"].as_bool() == Some(true);
    let м = меню.clone();
    шапка_списка(меню, "Bluetooth", Some(вкл), move |да| {
        let м = м.clone();
        позови("sys.bt.power", json!({ "включить": да, "on": да }), move |о| {
            if !жив(&м) {
                return;
            }
            if let Err(е) = о {
                popup::тихо(&м, &format!("{}: {е}", crate::i18n::tr("Не вышло")));
                м.show_all();
                return;
            }
            let м = м.clone();
            glib::timeout_add_local_once(Duration::from_millis(900), move || {
                if жив(&м) {
                    устройства_bt(&м)
                }
            });
        });
    });
    let о = Rc::new(о);
    let перерисуй: Rc<dyn Fn()> = {
        let (м, о) = (меню.clone(), о.clone());
        Rc::new(move || покажи_bt(&м, (*о).clone(), false))
    };
    if о["есть"].as_bool() == Some(false) {
        popup::тихо(меню, о["почему"].as_str().unwrap_or("Bluetooth на машине нет"));
    } else if !вкл {
        popup::тихо(меню, "Bluetooth выключен");
    } else {
        let список = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let все = о["list"].as_array().cloned().unwrap_or_default();
        let mut свои: Vec<&Value> = все.iter().filter(|у| у["своё"].as_bool() == Some(true)).collect();
        свои.sort_by_key(|у| у["соединено"].as_bool() != Some(true));
        let новые: Vec<&Value> = все.iter().filter(|у| у["своё"].as_bool() != Some(true) && !безымянное(у["имя"].as_str().unwrap_or(""))).collect();
        let заголовок = |т: &str| {
            let л = gtk::Label::new(Some(&crate::i18n::tr(т)));
            л.style_context().add_class("qs-section");
            л.set_xalign(0.0);
            л
        };
        if !свои.is_empty() {
            список.add(&заголовок("Ваши устройства"));
        }
        for у in свои {
            добавь_устройство(меню, &список, у, true, перерисуй.clone());
        }
        список.add(&заголовок("Новые устройства"));
        for у in &новые {
            добавь_устройство(меню, &список, у, false, перерисуй.clone());
        }
        if искать {
            let ищу = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            ищу.style_context().add_class("qs-note");
            let к = gtk::Spinner::new();
            к.start();
            ищу.add(&к);
            ищу.add(&gtk::Label::new(Some(&crate::i18n::tr("Ищу устройства рядом…"))));
            список.add(&ищу);
            let м = меню.clone();
            crate::agent::позови_за("sys.bt.scan", json!({ "секунд": 8 }), 25, move |о| {
                // Человек мог уже раскрыть строку — поиск её не закрывает
                if жив(&м) {
                    if let Ok(о) = о {
                        покажи_bt(&м, о, false);
                    }
                }
            });
        } else if новые.is_empty() {
            popup::тихо(&список, "Поблизости ничего не нашлось. Включите на устройстве режим сопряжения и обновите список");
        }
        меню.add(&листание_списка(&список));
    }
    let м = меню.clone();
    let о2 = о.clone();
    низ_списка(меню, "Дополнительные параметры Bluetooth", "bt", move || {
        if жив(&м) {
            покажи_bt(&м, (*о2).clone(), true);
        }
    });
    меню.show_all();
}

fn добавь_устройство(меню: &gtk::Box, список: &gtk::Box, у: &Value, своё: bool, перерисуй: Rc<dyn Fn()>) {
    let адрес = у["адрес"].as_str().unwrap_or("").to_string();
    let имя = у["имя"].as_str().unwrap_or(&адрес).to_string();
    let соединено = у["соединено"].as_bool() == Some(true);
    let под = if !своё {
        ""
    } else if соединено {
        "Подключено"
    } else {
        "Не подключено"
    };
    let (ящик, открыто) = строка_списка(список, &адрес, значок_устройства(у["вид"].as_str().unwrap_or("")), &имя, &crate::i18n::tr(под), соединено, перерисуй);
    if !открыто {
        return;
    }
    let итог = gtk::Label::new(None);
    итог.style_context().add_class("qs-note");
    итог.set_xalign(0.0);
    итог.set_line_wrap(true);
    итог.set_no_show_all(true);
    ящик.add(&итог);
    let ряд = ряд_действий(&ящик);
    // Шаги по очереди: новое — сопрячь, доверять (чтобы потом подключалось
    // само) и подключить; своё — только подключить или отключить
    let сделай = {
        let (м, итог, ряд, адрес) = (меню.clone(), итог.clone(), ряд.clone(), адрес.clone());
        move |шаги: Vec<&'static str>, словами: &'static str| {
            ряд.set_sensitive(false);
            итог.style_context().remove_class("bad");
            итог.set_text(&crate::i18n::tr(словами));
            итог.show();
            шаг_bt(м.clone(), итог.clone(), ряд.clone(), адрес.clone(), шаги, 0);
        }
    };
    if своё {
        let удалить = кнопка_действия("Удалить", false);
        let с = сделай.clone();
        удалить.connect_clicked(move |_| с(vec!["remove"], "Удаляю…"));
        ряд.add(&удалить);
        let б = if соединено { кнопка_действия("Отключить", false) } else { кнопка_действия("Подключить", true) };
        let с = сделай.clone();
        б.connect_clicked(move |_| if соединено { с(vec!["disconnect"], "Отключаю…") } else { с(vec!["connect"], "Подключаю…") });
        ряд.add(&б);
    } else {
        let б = кнопка_действия("Подключить", true);
        let с = сделай.clone();
        б.connect_clicked(move |_| с(vec!["pair", "trust", "connect"], "Сопрягаю… Подтвердите на устройстве, если оно спросит"));
        ряд.add(&б);
    }
}

fn шаг_bt(м: gtk::Box, итог: gtk::Label, ряд: gtk::Box, адрес: String, шаги: Vec<&'static str>, i: usize) {
    if i >= шаги.len() {
        РАСКРЫТО.with(|р| р.borrow_mut().clear());
        if жив(&м) {
            устройства_bt(&м);
        }
        return;
    }
    let что = шаги[i];
    crate::agent::позови_за("sys.bt.device", json!({ "action": что, "адрес": адрес }), 40, move |о| {
        if !жив(&м) {
            return;
        }
        match о {
            // «Доверять» может не пройти у иных устройств — это не беда
            Ok(_) => шаг_bt(м, итог, ряд, адрес, шаги, i + 1),
            Err(_) if что == "trust" => шаг_bt(м, итог, ряд, адрес, шаги, i + 1),
            Err(е) => {
                ряд.set_sensitive(true);
                итог.set_text(&format!("{}: {е}", crate::i18n::tr("Не вышло")));
                итог.style_context().add_class("bad");
                итог.show();
            }
        }
    });
}

// ---------------------------------------------------------------- батарея

pub fn батарея(кнопка: &gtk::Button) {
    popup::открой(кнопка, "батарея", Где::Справа, 200, |меню| {
        let п = popup::тихо(меню, "Смотрю…");
        позови("sys.battery", json!({}), move |о| match о {
            Ok(о) => match о["battery"]["level"].as_f64() {
                Some(у) if о["present"].as_bool() != Some(false) => п.set_markup(&format!(
                    "<span size='x-large' weight='bold'>{}%</span>\n{}",
                    у.round(),
                    if о["charging"].as_bool() == Some(true) { "заряжается" } else { "от батареи" }
                )),
                _ => п.set_text("Батареи нет — питание от сети"),
            },
            Err(_) => п.set_text("Машина не рассказала о батарее"),
        });
    });
}

// ---------------------------------------------------------------- раскладки

/// Имя раскладки по-человечески. XKB называет их по-английски («Russian»,
/// «English (US)»), под labwc известны только коды («ru», «us»)
pub fn имя_раскладки(имя: &str) -> (String, &'static str) {
    const ИМЕНА: &[(&str, &str, &str, &str)] = &[
        ("English (US)", "us", "EN", "Английский (США)"),
        ("English (UK)", "gb", "EN", "Английский (Великобритания)"),
        ("English", "", "EN", "Английский"),
        ("Russian", "ru", "RU", "Русский"),
        ("Ukrainian", "ua", "UK", "Украинский"),
        ("Belarusian", "by", "BE", "Белорусский"),
        ("Kazakh", "kz", "KK", "Казахский"),
        ("German", "de", "DE", "Немецкий"),
        ("French", "fr", "FR", "Французский"),
        ("Spanish", "es", "ES", "Испанский"),
        ("Italian", "it", "IT", "Итальянский"),
        ("Portuguese", "pt", "PT", "Португальский"),
        ("Polish", "pl", "PL", "Польский"),
        ("Turkish", "tr", "TR", "Турецкий"),
    ];
    match ИМЕНА.iter().find(|(н, к, _, _)| имя.starts_with(н) || (!к.is_empty() && имя == *к)) {
        Some((_, _, код, по_русски)) => (код.to_string(), по_русски),
        None => (имя.chars().filter(|с| с.is_alphabetic()).take(2).collect::<String>().to_uppercase(), ""),
    }
}

/// Меню раскладок под значком языка — как в Windows: все раскладки, текущая
/// отмечена, щелчок переключает. Раньше значок открывал «Параметры», а это
/// не то, чего ждёшь от щелчка по «EN»
pub fn раскладки(кнопка: &gtk::Button) {
    popup::открой(кнопка, "раскладки", Где::Справа, 250, |меню| {
        // Свой сервер знает и список, и текущую, и умеет переключать
        let от_сервера = crate::core::скажи_серверу("layouts").filter(|о| !о.is_empty() && !о.starts_with("не понимаю"));
        let (сейчас, имена, можно): (usize, Vec<String>, bool) = match от_сервера {
            Some(о) => {
                let mut ч = о.split('\t');
                let сейчас = ч.next().and_then(|н| н.parse().ok()).unwrap_or(0);
                (сейчас, ч.map(String::from).collect(), true)
            }
            // labwc текущую раскладку не рассказывает — показываем набор
            None => {
                let файл = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config/glower/keyboard");
                let коды = std::fs::read_to_string(файл).unwrap_or_else(|_| "us,ru".into());
                (0, коды.trim().split(',').filter(|к| !к.is_empty()).map(String::from).collect(), false)
            }
        };
        for (i, имя) in имена.iter().enumerate() {
            let (код, по_русски) = имя_раскладки(имя);
            let б = gtk::Button::new();
            б.style_context().add_class("row");
            let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            let к = gtk::Label::new(Some(&код));
            к.style_context().add_class("kb-code");
            ряд.add(&к);
            let н = gtk::Label::new(Some(if по_русски.is_empty() { имя.as_str() } else { по_русски }));
            н.set_xalign(0.0);
            н.set_hexpand(true);
            н.set_ellipsize(gtk::pango::EllipsizeMode::End);
            ряд.add(&н);
            if i + 1 == сейчас {
                б.style_context().add_class("kb-on");
                let г = gtk::Image::from_icon_name(Some("object-select-symbolic"), gtk::IconSize::Button);
                г.style_context().add_class("kb-check");
                ряд.add(&г);
            }
            б.add(&ряд);
            if можно {
                б.connect_clicked(move |_| {
                    crate::core::скажи_серверу(&format!("layout-set {}", i + 1));
                    popup::закрой();
                });
            } else {
                б.set_sensitive(false);
            }
            меню.add(&б);
        }
        if !можно {
            popup::тихо(меню, "Переключение — Alt + Shift");
        }
        меню.add(&gtk::Separator::new(gtk::Orientation::Horizontal));
        popup::строка(меню, "Параметры раскладки…", false, || {
            popup::закрой();
            crate::core::открой_параметры("time");
        });
    });
}

// ---------------------------------------------------------------- питание

/// Слова — те, что понимает агент: lock, logout, suspend, reboot, poweroff.
const ДЕЙСТВИЯ: [(&str, &str, Option<&str>, bool); 6] = [
    ("lock", "Заблокировать", None, false),
    // Сменить пользователя — это выход на экран входа: сеансы двух людей
    // разом эта система не держит, и честнее сказать об этом сразу
    ("switch", "Сменить пользователя", Some("Программы закроются, и откроется экран входа, где можно выбрать другого человека."), false),
    ("logout", "Выйти из системы", Some("Все открытые программы закроются."), false),
    ("suspend", "Спящий режим", None, false),
    ("reboot", "Перезагрузка", Some("Машина перезагрузится."), false),
    ("poweroff", "Выключение", Some("Машина выключится."), true),
];

pub fn питание(кнопка: &gtk::Button) {
    let к = кнопка.clone();
    popup::открой(кнопка, "питание", Где::Справа, 210, move |меню| {
        let обновлений = ЗНАЮ.with(|з| з.borrow().обновлений);
        if обновлений > 0 {
            // Обновления ставит стол: у него есть экран «не выключайте
            // компьютер», а у нас — полоса в тридцать точек
            for (что, имя) in [("update-reboot", "Обновить и перезагрузить"), ("update-poweroff", "Обновить и завершить работу")] {
                popup::строка(меню, имя, false, move || {
                    popup::закрой();
                    скажи("питание", json!(что));
                });
            }
            меню.add(&gtk::Separator::new(gtk::Orientation::Horizontal));
        }
        for (id, имя, вопрос, опасно) in ДЕЙСТВИЯ {
            let м = меню.clone();
            let к = к.clone();
            popup::строка(меню, имя, опасно, move || match вопрос {
                Some(в) => спроси(&м, &к, id, имя, в),
                None => сделай(&к, id, имя),
            });
        }
    });
}

fn спроси(меню: &gtk::Box, кнопка: &gtk::Button, id: &'static str, имя: &'static str, вопрос: &str) {
    popup::очисти(меню);
    let з = gtk::Label::new(Some(&format!("{имя}?")));
    з.style_context().add_class("title");
    з.set_xalign(0.0);
    меню.add(&з);
    popup::тихо(меню, &format!("{вопрос} Это действие затронет всю машину, а не только оболочку."));
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    ряд.set_homogeneous(true);
    let нет = gtk::Button::with_label("Отмена");
    нет.style_context().add_class("action");
    нет.connect_clicked(|_| popup::закрой());
    let да = gtk::Button::with_label(имя);
    да.style_context().add_class("action");
    да.style_context().add_class("main");
    let к = кнопка.clone();
    да.connect_clicked(move |_| сделай(&к, id, имя));
    ряд.add(&нет);
    ряд.add(&да);
    меню.add(&ряд);
    меню.show_all();
    да.grab_focus();
}

fn сделай(кнопка: &gtk::Button, id: &'static str, имя: &'static str) {
    popup::закрой();
    let id = if id == "switch" { "logout" } else { id };
    if id == "lock" {
        позови("sys.power", json!({ "action": id }), |_| {});
        return;
    }
    let з = занавес(match id {
        "reboot" => "Перезагрузка…",
        "suspend" => "Засыпаю…",
        "logout" if имя == "Сменить пользователя" => "Открываю экран входа…",
        "logout" => "Выхожу из системы…",
        _ => "Завершение работы…",
    });
    let к = кнопка.clone();
    // Перезагрузка и выключение — с чистого листа: стол забывает окна,
    // чтобы после включения ничего не открылось само. Ему хватает
    // полсекунды
    let задержка = if matches!(id, "reboot" | "poweroff") {
        скажи("питание", json!(id));
        500
    } else {
        0
    };
    glib::timeout_add_local_once(Duration::from_millis(задержка), move || позови("sys.power", json!({ "action": id }), move |о| {
        // Сон кончится, и человек должен увидеть стол, а не занавес
        if id == "suspend" {
            glib::timeout_add_local_once(Duration::from_secs(2), move || unsafe { з.destroy() });
            return;
        }
        if let Err(е) = о {
            // Машина отказалась — убираем занавес и говорим почему: иначе
            // человек остаётся перед чёрным экраном работающего компьютера
            unsafe { з.destroy() };
            popup::открой(&к, "отказ", Где::Справа, 240, |м| {
                let т = gtk::Label::new(Some(&format!("Не вышло: {имя}")));
                т.style_context().add_class("title");
                т.set_xalign(0.0);
                м.add(&т);
                popup::тихо(м, &е);
            });
        }
    }));
}

/// Чёрный экран с надписью — поверх всего, вместе с самой полосой.
pub(crate) fn занавес(надпись: &str) -> gtk::Window {
    let о = gtk::Window::new(gtk::WindowType::Toplevel);
    о.set_decorated(false);
    о.init_layer_shell();
    о.set_layer(Layer::Overlay);
    о.set_namespace("glower-panel-curtain");
    for край in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        о.set_anchor(край, true);
    }
    о.set_exclusive_zone(-1);
    о.set_keyboard_mode(KeyboardMode::None);
    о.style_context().add_class("curtain");
    let п = gtk::Label::new(Some(надпись));
    о.add(&п);
    о.show_all();
    о
}

#[cfg(test)]
mod проверки {
    use super::*;

    // Ответы — настоящие, с живой машины: имена полей берём у агента
    #[test]
    fn заряд_словами_как_у_страничной() {
        assert_eq!(заряд_словами(&json!({ "present": true, "battery": { "level": 81 }, "charging": true })), "81% · заряжается");
        assert_eq!(заряд_словами(&json!({ "present": true, "battery": { "level": 40.4 }, "charging": false })), "40% · от батареи");
        assert_eq!(заряд_словами(&json!({ "present": false, "ac": { "online": true } })), "от сети");
        assert_eq!(заряд_словами(&json!({})), "нет батареи");
    }

    #[test]
    fn wifi_называет_сеть_и_честно_говорит_о_её_отсутствии() {
        let w = разбери_wifi(Ok(json!({ "radio": true, "active": [{ "type": "802-11-wireless", "name": "Дом" }] })));
        assert!(!w.нет && w.радио && w.под == "Дом");
        let w = разбери_wifi(Ok(json!({ "radio": false, "active": [] })));
        assert!(!w.радио && w.под == "выключен");
        assert!(разбери_wifi(Ok(json!({ "supported": false }))).нет);
        assert!(разбери_wifi(Err("нет агента".into())).нет);
    }

    #[test]
    fn bluetooth_выключенный_программно_можно_включить() {
        let б = разбери_bt(Ok(json!({ "есть": false, "подробно": { "блокировка": { "программно": true } } })));
        assert!(б.можно && б.под == "выключен");
        let б = разбери_bt(Ok(json!({ "есть": false, "подробно": { "блокировка": { "рубильником": true } } })));
        assert!(!б.можно && б.под == "выключен кнопкой");
        let б = разбери_bt(Ok(json!({ "есть": true, "включён": true })));
        assert!(б.можно && б.вкл);
    }
}
