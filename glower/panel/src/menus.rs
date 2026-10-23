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

fn плитка(ряд: &gtk::Grid, место: i32, имя: &str, под: &str, вкл: bool, можно: bool) -> gtk::Button {
    let б = gtk::Button::new();
    б.style_context().add_class("tile");
    if вкл {
        б.style_context().add_class("on");
    }
    let к = gtk::Box::new(gtk::Orientation::Vertical, 1);
    let и = gtk::Label::new(Some(имя));
    и.style_context().add_class("tile-name");
    и.set_xalign(0.0);
    let п = gtk::Label::new(Some(под));
    п.style_context().add_class("tile-sub");
    п.set_xalign(0.0);
    п.set_ellipsize(gtk::pango::EllipsizeMode::End);
    к.add(&и);
    к.add(&п);
    б.add(&к);
    б.set_sensitive(можно);
    б.set_hexpand(true);
    ряд.attach(&б, место % 2, место / 2, 1, 1);
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
fn ползунок(меню: &gtk::Box, имя: &str, взять: &'static str, поле: &'static str, поставить: &'static str) {
    let ряд = gtk::Box::new(gtk::Orientation::Vertical, 2);
    ряд.style_context().add_class("slider");
    let л = gtk::Label::new(Some(имя));
    л.set_xalign(0.0);
    let ш = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    ш.set_draw_value(false);
    ш.set_sensitive(false);
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
    popup::открой(кнопка, "быстрые", Где::Справа, 300, |меню| наполни_быстрые(меню));
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
            let б = плитка(&плитки, 0, "Wi-Fi", &w.под, w.радио, !w.нет);
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
            if радио {
                let м = меню.clone();
                popup::строка(&сети, "Сети Wi-Fi  ›", false, move || сети_wifi(&м));
            }
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
            let б = плитка(&плитки, 1, "Bluetooth", &b.под, b.вкл, b.можно);
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
    for (место, ключ, имя, вкл, да, нет) in [
        (2, "dnd", "Не беспокоить", dnd, "включено", "выключено"),
        (3, "nightLight", "Ночной свет", ночной, "тёплые тона", "выключен"),
    ] {
        let б = плитка(&плитки, место, имя, if вкл { да } else { нет }, вкл, true);
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
        let н = gtk::Label::new(Some(&format!("♫ {}", if и.название.is_empty() { "Без названия" } else { &и.название })));
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
            let б = gtk::Button::with_label(знак);
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

    ползунок(меню, "Громкость", "sys.volume.get", "volume", "sys.volume.set");
    ползунок(меню, "Яркость", "sys.brightness.get", "value", "sys.brightness.set");

    let (_, заряд) = popup::пара(меню, "Питание", "…");
    позови("sys.battery", json!({}), move |о| {
        заряд.set_text(&о.map(|о| заряд_словами(&о)).unwrap_or_else(|_| "—".into()));
    });

    // Раскладки переключает оконный сервер (Alt+Shift) — всем программам
    // разом; здесь — какие включены и куда идти, чтобы их сменить
    let раскладка = ЗНАЮ.with(|з| з.borrow().раскладка.clone());
    popup::строка(меню, &format!("Раскладки: {раскладка}  ·  Alt+Shift"), false, || {
        скажи("покажи", json!("раскладка"));
        popup::закрой();
    });

    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    ряд.set_homogeneous(true);
    let параметры = gtk::Button::with_label("Параметры");
    параметры.style_context().add_class("action");
    параметры.connect_clicked(|_| {
        скажи("открой", json!({ "вид": "приложение", "id": "settings" }));
        popup::закрой();
    });
    let замок = gtk::Button::with_label("Заблокировать");
    замок.style_context().add_class("action");
    замок.connect_clicked(|_| {
        позови("sys.power", json!({ "action": "lock" }), |_| {});
        popup::закрой();
    });
    ряд.add(&параметры);
    ряд.add(&замок);
    меню.add(&ряд);
    меню.show_all();
}

/// Список сетей — отдельным видом внутри того же окошка.
fn сети_wifi(меню: &gtk::Box) {
    popup::очисти(меню);
    let м = меню.clone();
    popup::строка(меню, "‹  Wi-Fi", false, move || наполни_быстрые(&м));
    let ищу = popup::тихо(меню, "Ищу сети…");
    меню.show_all();
    let м = меню.clone();
    позови("sys.wifi.scan", json!({}), move |о| {
        if !жив(&м) {
            return;
        }
        м.remove(&ищу);
        let о = о.unwrap_or(Value::Null);
        let сети = о["список"].as_array().or(о["list"].as_array()).cloned().unwrap_or_default();
        let mut показано = 0;
        for с in сети.iter().take(8) {
            let Some(имя) = с["ssid"].as_str().or(с["имя"].as_str()).or(с["name"].as_str()) else { continue };
            let сигнал = с["signal"].as_f64().or(с["сигнал"].as_f64());
            let текст = match сигнал {
                Some(s) => format!("{имя}   {}%", s.round()),
                None => имя.to_string(),
            };
            let имя = имя.to_string();
            let мм = м.clone();
            popup::строка(&м, &текст, false, move || {
                let мм = мм.clone();
                позови("sys.wifi.connect", json!({ "ssid": имя }), move |о| {
                    if о.is_ok() {
                        if жив(&мм) {
                            наполни_быстрые(&мм)
                        }
                    } else {
                        // Пароль спрашивать здесь негде: за этим есть Параметры
                        скажи("открой", json!({ "вид": "приложение", "id": "settings", "довод": { "section": "net" } }));
                        popup::закрой();
                    }
                });
            });
            показано += 1;
        }
        if показано == 0 {
            popup::тихо(&м, "Сетей не видно");
        }
        м.show_all();
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
