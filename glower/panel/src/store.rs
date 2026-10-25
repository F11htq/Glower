//! Магазин — нативный, настоящим окном (glower-store).
//!
//! Тот же Магазин, что был на странице: витрина, разделы, поиск по Ubuntu и
//! Flathub, страница программы, установка и удаление, обновления
//! приложений, «Моё». Подборки лежат в store-catalog.json — том же, по
//! которому проверяется страничный (см. test/linux.mjs): список один.
//!
//! Ставит и удаляет агент, Магазин показывает и просит.

use crate::agent::{зов, позови, скажи, слушай};
use crate::apps::подготовь;
use base64::Engine;
use gtk::gdk_pixbuf::{InterpType, Pixbuf, PixbufLoader};
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

const КАТАЛОГ: &str = include_str!("store-catalog.json");

const СТИЛЬ: &str = r#"
.store .rail { background-color: @glower_bg2; border-right: 1px solid alpha(@glower_fg, 0.07); padding: 8px 6px; }
.store .rail button { padding: 8px 4px; border-radius: 10px; background-color: transparent; min-width: 64px; }
.store .rail button.on { background-color: alpha(@glower_accent, 0.24); }
.store .rail .z { font-size: 20px; }
.store .rail .p { font-size: 11px; opacity: 0.8; }
.store .top { padding: 10px 16px; border-bottom: 1px solid alpha(@glower_fg, 0.07); }
.store .body { padding: 18px 22px 40px; }
.store .h { font-size: 18px; font-weight: 600; margin: 14px 2px 8px; }
.store .hero { border-radius: 16px; padding: 22px; min-height: 170px; color: #ffffff; }
.store .hero-small { min-height: 78px; padding: 14px; }
.store .hero .tag { font-size: 11px; background-color: rgba(0, 0, 0, 0.25); border-radius: 8px; padding: 2px 8px; }
.store .hero .name { font-size: 26px; font-weight: 700; }
.store .hero-small .name { font-size: 16px; }
.store .hero .line { font-size: 13px; opacity: 0.9; }
.store .chip { border-radius: 18px; padding: 6px 14px; }
.store .card { border-radius: 12px; padding: 12px; background-color: alpha(@glower_fg, 0.05); min-width: 220px; }
.store .card:hover { background-color: alpha(@glower_fg, 0.09); }
.store .card .n { font-weight: 600; }
.store .card .a { font-size: 12px; opacity: 0.6; }
.store .ico { border-radius: 12px; min-width: 48px; min-height: 48px; font-size: 26px; }
.store .ico-big { min-width: 96px; min-height: 96px; font-size: 52px; border-radius: 22px; }
.store .tile { border-radius: 14px; padding: 18px; min-width: 190px; min-height: 96px; color: #ffffff; }
.store .tile .z { font-size: 30px; }
.store .fact { background-color: alpha(@glower_fg, 0.05); border-radius: 10px; padding: 10px 14px; }
.store .fact .k { font-size: 11px; opacity: 0.6; }
.store .about { font-size: 14px; opacity: 0.85; margin-top: 14px; }
.store .note { font-size: 13px; opacity: 0.65; margin: 8px 2px; }
.store .big { font-size: 15px; padding: 10px 22px; }
.store .work { padding: 10px 16px; background-color: @glower_bg3; border-top: 1px solid alpha(@glower_fg, 0.07); }
.store .dot { min-width: 8px; min-height: 8px; border-radius: 4px; padding: 0; background-color: alpha(@glower_fg, 0.3); }
.store .dot.on { background-color: #ffffff; }
"#;

/// Программа, как её знает Магазин: из подборки, витрины или поиска
#[derive(Clone, Debug, Default)]
struct Прога {
    пакет: String,
    имя: String,
    про: String,
    знак: String,
    флатхаб: String,
    источник: String,
}

#[derive(Clone, Debug)]
enum Экран {
    Обзор,
    Разделы,
    Раздел(String),
    Программа(Прога),
    Поиск(String),
    Обновления,
    Моё,
}

struct Окно {
    окно: gtk::Window,
    полоса: gtk::Box,
    тело: gtk::Box,
    прокрутка: gtk::ScrolledWindow,
    поле: gtk::SearchEntry,
    экран: RefCell<Экран>,
    назад: RefCell<Vec<Экран>>,
    поколение: Cell<u64>,
    каталог: Value,
    состояние: RefCell<Option<Value>>,
    сведения: RefCell<HashMap<String, Value>>,
    значки: RefCell<HashMap<String, Option<Pixbuf>>>,
    работа: Cell<bool>,
    ход: gtk::Revealer,
    ход_текст: gtk::Label,
    ход_полоса: gtk::ProgressBar,
    витрина: Cell<usize>,
    /// Есть ли интернет: None — ещё проверяем
    сеть: Cell<Option<bool>>,
}

type П = Rc<Окно>;

/* ---------- мелочи ---------- */

/// Стоит ли программа: агент пишет сюда версию (стоит), null или false
/// (не стоит). Раньше «не пусто» значило «стоит», и поиск, где агент
/// отвечает false, показывал «уже стоит» у каждой найденной программы
fn стоит(в: &Value) -> bool {
    match в {
        Value::String(с) => !с.is_empty(),
        Value::Bool(б) => *б,
        _ => false,
    }
}

fn строка(в: &Value, k: &str) -> String {
    match &в[k] {
        Value::String(с) => с.clone(),
        Value::Null => String::new(),
        другое => другое.to_string(),
    }
}

fn из_записи(з: &Value) -> Прога {
    Прога {
        пакет: строка(з, "п"),
        имя: строка(з, "имя"),
        про: строка(з, "про"),
        знак: строка(з, "зн"),
        флатхаб: строка(з, "ф"),
        источник: строка(з, "и"),
    }
}

fn размер(б: u64) -> String {
    if б == 0 {
        String::new()
    } else if б > 1048576 {
        format!("{:.1} МБ", б as f64 / 1048576.0).replace('.', ",")
    } else {
        format!("{} КБ", б / 1024)
    }
}

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

fn фон(в: &impl IsA<gtk::Widget>, css: &str) {
    let стиль = gtk::CssProvider::new();
    if стиль.load_from_data(format!("* {{ background-image: {}; }}", crate::desktop::цвета_для_gtk(css)).as_bytes()).is_ok() {
        в.style_context().add_provider(&стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2);
    }
}

fn картинка(адрес: &str, размер: i32) -> Option<Pixbuf> {
    let (_, данные) = адрес.split_once(";base64,")?;
    let байты = base64::engine::general_purpose::STANDARD.decode(данные.trim()).ok()?;
    let чтец = PixbufLoader::new();
    чтец.write(&байты).ok()?;
    чтец.close().ok()?;
    чтец.pixbuf()?.scale_simple(размер, размер, InterpType::Bilinear)
}

fn очисти(к: &gtk::Box) {
    for р in к.children() {
        к.remove(&р);
    }
}

fn сообщи(п: &П, заголовок: &str, текст: &str) {
    let д = gtk::MessageDialog::new(Some(&п.окно), gtk::DialogFlags::MODAL, gtk::MessageType::Warning, gtk::ButtonsType::Close, заголовок);
    д.set_secondary_text(Some(текст));
    д.run();
    unsafe { д.destroy() };
}

fn спроси_да(п: &П, заголовок: &str, текст: &str, да: &str) -> bool {
    let д = gtk::MessageDialog::new(Some(&п.окно), gtk::DialogFlags::MODAL, gtk::MessageType::Question, gtk::ButtonsType::None, заголовок);
    д.set_secondary_text(Some(текст));
    д.add_button("Отмена", gtk::ResponseType::Cancel);
    д.add_button(да, gtk::ResponseType::Accept);
    let ответ = д.run();
    unsafe { д.destroy() };
    ответ == gtk::ResponseType::Accept
}

/// Ответ агента — только тому экрану, который спрашивал
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

/* ---------- лицо программы ----------

   Сперва — машина: у поставленных программ значок на диске, а к спискам
   пакетов apt кладёт каталог значков для магазинов. Нет там — Flathub,
   его значки приносит агент. Нет и там — наш знак. Ради значка никто не
   ждёт: он подменяет знак, когда придёт. */

fn значок(п: &П, прога: &Прога, большой: bool) -> gtk::Widget {
    let размер = if большой { 96 } else { 48 };
    let место = gtk::Stack::new();
    место.set_valign(gtk::Align::Center);
    // Пока настоящий значок программы не пришёл — значок темы по её роду
    // (символ шрифта — только если пары в теме нет)
    let символ = if прога.знак.is_empty() { "📦" } else { прога.знак.as_str() };
    let знак: gtk::Widget = match crate::apps::имя_значка(символ) {
        Some(имя) => {
            let к = gtk::Image::from_icon_name(Some(имя), gtk::IconSize::Dialog);
            к.set_pixel_size(if большой { 48 } else { 24 });
            к.upcast()
        }
        None => gtk::Label::new(Some(символ)).upcast(),
    };
    знак.style_context().add_class("ico");
    if большой {
        знак.style_context().add_class("ico-big");
    }
    фон(&знак, "linear-gradient(140deg, rgba(255,255,255,0.14), rgba(255,255,255,0.06))");
    место.add_named(&знак, "знак");
    let ключ = format!("{}|{}", прога.пакет, прога.флатхаб);
    let поставь = {
        let место = место.clone();
        move |к: &Pixbuf| {
            let к = к.scale_simple(размер, размер, InterpType::Bilinear).unwrap_or_else(|| к.clone());
            let и = gtk::Image::from_pixbuf(Some(&к));
            и.show();
            место.add_named(&и, "лицо");
            место.set_visible_child_name("лицо");
        }
    };
    if let Some(готово) = п.значки.borrow().get(&ключ) {
        if let Some(к) = готово {
            поставь(к);
        }
        return место.upcast();
    }
    if прога.пакет.is_empty() && прога.флатхаб.is_empty() {
        return место.upcast();
    }
    let (п2, прога2) = (Rc::downgrade(п), прога.clone());
    let (куда, откуда) = async_channel::bounded::<Option<String>>(1);
    std::thread::spawn(move || {
        let mut данные = None;
        if !прога2.пакет.is_empty() {
            if let Ok(о) = зов("sys.icon", json!({ "имя": прога2.пакет, "ярлык": if прога2.флатхаб.is_empty() { &прога2.пакет } else { &прога2.флатхаб } }), 10) {
                if о["есть"].as_bool() == Some(true) {
                    данные = о["данные"].as_str().map(String::from);
                }
            }
        }
        if данные.is_none() && !прога2.флатхаб.is_empty() {
            if let Ok(о) = зов("store.icon", json!({ "ид": прога2.флатхаб }), 20) {
                if о["есть"].as_bool() == Some(true) {
                    данные = о["данные"].as_str().map(String::from);
                }
            }
        }
        let _ = куда.send_blocking(данные);
    });
    glib::MainContext::default().spawn_local(async move {
        let Ok(данные) = откуда.recv().await else { return };
        let к = данные.and_then(|д| картинка(&д, 128));
        if let Some(п) = п2.upgrade() {
            п.значки.borrow_mut().insert(ключ, к.clone());
        }
        if let Some(к) = к {
            поставь(&к);
        }
    });
    место.upcast()
}

/* ---------- кирпичи ---------- */

fn заголовок(п: &П, текст: &str) {
    п.тело.add(&ярлык(текст, "h"));
}

fn заметка(к: &gtk::Box, текст: &str) {
    к.add(&ярлык(текст, "note"));
}

fn сетка() -> gtk::FlowBox {
    let с = gtk::FlowBox::new();
    с.set_selection_mode(gtk::SelectionMode::None);
    с.set_row_spacing(10);
    с.set_column_spacing(10);
    с.set_homogeneous(true);
    с.set_max_children_per_line(4);
    с
}

fn карточка(п: &П, прога: &Прога) -> gtk::Button {
    let б = gtk::Button::new();
    б.style_context().add_class("card");
    let р = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    р.add(&значок(п, прога, false));
    let т = gtk::Box::new(gtk::Orientation::Vertical, 2);
    т.set_valign(gtk::Align::Center);
    let и = ярлык(&прога.имя, "n");
    и.set_ellipsize(gtk::pango::EllipsizeMode::End);
    и.set_line_wrap(false);
    т.add(&и);
    let а = ярлык(&прога.про, "a");
    а.set_lines(2);
    а.set_ellipsize(gtk::pango::EllipsizeMode::End);
    а.set_max_width_chars(28);
    т.add(&а);
    р.add(&т);
    б.add(&р);
    let (п2, прога2) = (п.clone(), прога.clone());
    б.connect_clicked(move |_| открой_программу(&п2, прога2.clone()));
    б
}

fn иди(п: &П, куда: Экран) {
    let было = п.экран.replace(куда);
    if !matches!(было, Экран::Поиск(_)) {
        п.назад.borrow_mut().push(было);
    }
    рисуй(п);
}

fn открой_программу(п: &П, прога: Прога) {
    иди(п, Экран::Программа(прога));
}

/* ---------- разговор с системой ---------- */

/// Что система умеет: есть ли apt, Flathub, право ставить
fn узнай_состояние(п: &П, дело: impl FnOnce(&П, Value) + 'static) {
    if let Some(с) = п.состояние.borrow().clone() {
        return дело(п, с);
    }
    let п2 = п.clone();
    позови("pkg.state", json!({}), move |о| {
        let с = о.unwrap_or_else(|е| json!({ "reason": е }));
        *п2.состояние.borrow_mut() = Some(с.clone());
        дело(&п2, с);
    });
}

fn можно_ставить(с: &Value) -> Result<(), String> {
    if !строка(с, "reason").is_empty() {
        return Err(строка(с, "reason"));
    }
    if с["allowed"].as_bool() != Some(true) {
        return Err("система не управляет программами машины".into());
    }
    Ok(())
}

/// Сведения о программе — у машины: описание, размер, версия, стоит ли
fn сведения(п: &П, прога: &Прога, заново: bool, дело: impl FnOnce(&П, Value) + 'static) {
    if !заново {
        // Запись из выдачи поиска — неполная: в ней ещё нет версии и размера,
        // и страница программы показывала «Версия: false». Такую не берём
        let есть = п.сведения.borrow().get(&прога.пакет).cloned().filter(|с| {
            с.get("candidate").is_some() || с["нет"].as_bool() == Some(true) || стоит(&с["installed"])
        });
        if let Some(с) = есть {
            return дело(п, с);
        }
    }
    let прога = прога.clone();
    узнай_состояние(п, move |п, с| {
        if let Err(почему) = можно_ставить(&с) {
            let нет = json!({ "нет": true, "почему": почему });
            п.сведения.borrow_mut().insert(прога.пакет.clone(), нет.clone());
            return дело(п, нет);
        }
        let флатпак = прога.источник == "flatpak";
        if флатпак && с["flatpak"].as_bool() != Some(true) {
            let нет = json!({ "нет": true, "почему": "на этой машине нет flatpak — программу отсюда не поставить" });
            return дело(п, нет);
        }
        let есть_списки = с["lists"].as_bool() == Some(true);
        let флатхаб_есть = с["flathubData"].as_bool() == Some(true);
        let (куда, откуда) = async_channel::bounded::<Value>(1);
        let пакет = прога.пакет.clone();
        std::thread::spawn(move || {
            let итог = if флатпак {
                match зов("pkg.info", json!({ "name": прога.пакет, "source": "flatpak" }), 60) {
                    Ok(ф) if !ф["candidate"].is_null() || стоит(&ф["installed"]) => ф,
                    _ => json!({ "нет": true, "почему": if флатхаб_есть { "на Flathub такого нет" } else { "Flathub ещё не подключён — подключим при установке" } }),
                }
            } else {
                if !есть_списки {
                    // В свежей системе списки пакетов пусты — читаем сами
                    if зов("pkg.update", json!({}), 30).is_ok() {
                        жди_работу_блокируя();
                    }
                }
                let mut о = зов("pkg.info", json!({ "name": прога.пакет }), 60).unwrap_or(Value::Null);
                if о["candidate"].is_null() && !стоит(&о["installed"]) {
                    // Под записанным именем может не быть: ищем похожее
                    if let Ok(р) = зов("pkg.search", json!({ "query": прога.пакет }), 60) {
                        let список = р["list"].as_array().cloned().unwrap_or_default();
                        if let Some(т) = список.iter().find(|х| строка(х, "name") == прога.пакет).or(список.first()).cloned() {
                            let mut и = зов("pkg.info", json!({ "name": строка(&т, "name"), "source": строка(&т, "source") }), 60).unwrap_or(json!({}));
                            if let (Some(и), Some(т)) = (и.as_object_mut(), т.as_object()) {
                                for (к, в) in т {
                                    и.entry(к.clone()).or_insert(в.clone());
                                }
                            }
                            о = и;
                        }
                    }
                }
                if о["candidate"].is_null() && !стоит(&о["installed"]) {
                    json!({ "нет": true, "почему": "в репозиториях такого нет" })
                } else {
                    о
                }
            };
            let _ = куда.send_blocking(итог);
        });
        let п2 = п.clone();
        glib::MainContext::default().spawn_local(async move {
            if let Ok(итог) = откуда.recv().await {
                п2.сведения.borrow_mut().insert(пакет, итог.clone());
                дело(&п2, итог);
            }
        });
    });
}

fn жди_работу_блокируя() -> Value {
    for _ in 0..900 {
        let j = зов("pkg.job", json!({}), 10).unwrap_or(json!({ "running": false }));
        if j["running"].as_bool() != Some(true) {
            return j;
        }
        std::thread::sleep(Duration::from_millis(1200));
    }
    json!({ "running": false, "ok": false })
}

/// Следить за работой агента: ход — внизу окна, по концу — `конец`
fn следи(п: &П, конец: impl FnOnce(&П, Value) + 'static) {
    if п.работа.replace(true) {
        return;
    }
    п.ход.set_reveal_child(true);
    п.ход_текст.set_text("Начинаю…");
    п.ход_полоса.set_fraction(0.0);
    let п1 = Rc::downgrade(п);
    let конец: Rc<RefCell<Option<Box<dyn FnOnce(&П, Value)>>>> = Rc::new(RefCell::new(Some(Box::new(конец))));
    let ждём = Rc::new(Cell::new(false));
    glib::timeout_add_local(Duration::from_millis(1200), move || {
        let Some(п) = п1.upgrade() else { return glib::ControlFlow::Break };
        if !п.работа.get() {
            return glib::ControlFlow::Break;
        }
        // Прошлый вопрос ещё без ответа — не плодим новые
        if ждём.replace(true) {
            return glib::ControlFlow::Continue;
        }
        let (п2, ждём2, конец2) = (п.clone(), ждём.clone(), конец.clone());
        позови("pkg.job", json!({}), move |о| {
            ждём2.set(false);
            let j = о.unwrap_or(json!({ "running": false, "ok": false, "error": "агент не ответил" }));
            if j["running"].as_bool() == Some(true) {
                let шаг = строка(&j, "step");
                let имя = строка(&j, "name");
                п2.ход_текст.set_text(&format!("{}{}", if имя.is_empty() { String::new() } else { format!("{имя} · ") }, if шаг.is_empty() { "Работаю…".to_string() } else { шаг }));
                п2.ход_полоса.set_fraction(j["percent"].as_f64().unwrap_or(0.0).clamp(0.0, 100.0) / 100.0);
                return;
            }
            п2.работа.set(false);
            п2.ход.set_reveal_child(false);
            if let Some(к) = конец2.borrow_mut().take() {
                к(&п2, j);
            }
        });
        glib::ControlFlow::Continue
    });
}

/* Steam из Flathub без правил для геймпадов ругается при первом запуске.
   Правила — крошечный пакет системы: ставим его сами */
const ДОПОЛНЕНИЯ: &[(&str, &str)] = &[("com.valvesoftware.Steam", "steam-devices")];

fn поставь(п: &П, имя: String, источник: String) {
    let п1 = п.clone();
    узнай_состояние(п, move |п, с| {
        if let Err(почему) = можно_ставить(&с) {
            return сообщи(п, "Не выйдет", &почему);
        }
        if источник == "flatpak" {
            if с["flatpak"].as_bool() != Some(true) {
                return сообщи(п, "Не выйдет", "На этой машине нет flatpak, а эта программа живёт только там.");
            }
            if с["flathubData"].as_bool() != Some(true) {
                if !спроси_да(п, "Подключить Flathub?", "Эта программа живёт на Flathub. Чтобы её поставить, система сперва прочитает его списки — это около тридцати мегабайт и одна-две минуты. Дальше всё оттуда ставится сразу.", "Подключить") {
                    return;
                }
                let (п2, имя2) = (п1.clone(), имя.clone());
                позови("pkg.flathub", json!({}), move |о| {
                    if let Err(е) = о {
                        return сообщи(&п2, "Flathub не подключился", &е);
                    }
                    следи(&п2, move |п, j| {
                        if j["ok"].as_bool() != Some(true) {
                            return сообщи(п, "Flathub не подключился", &строка(&j, "error"));
                        }
                        *п.состояние.borrow_mut() = None;
                        п.сведения.borrow_mut().clear();
                        начни_установку(п, имя2, "flatpak".into());
                    });
                });
                return;
            }
        }
        начни_установку(п, имя, источник);
    });
}

fn начни_установку(п: &П, имя: String, источник: String) {
    let п2 = п.clone();
    позови("pkg.install", json!({ "name": имя, "source": источник }), move |о| {
        if let Err(е) = о {
            return сообщи(&п2, "Не вышло начать установку", &е);
        }
        следи(&п2, move |п, j| {
            if j["ok"].as_bool() == Some(false) {
                сообщи(п, "Установка не удалась", &строка(&j, "error"));
            } else if let Some((_, доп)) = ДОПОЛНЕНИЯ.iter().find(|(кто, _)| *кто == имя) {
                позови("pkg.install", json!({ "name": доп, "source": "apt" }), |_| {});
            }
            п.сведения.borrow_mut().clear();
            рисуй(п);
        });
    });
}

fn убери(п: &П, имя: String, источник: String, название: &str) {
    if !спроси_да(п, &format!("Удалить {название}?"), "Программа будет удалена вместе с ненужными зависимостями.", "Удалить") {
        return;
    }
    let п2 = п.clone();
    позови("pkg.remove", json!({ "name": имя, "source": источник }), move |о| {
        if let Err(е) = о {
            return сообщи(&п2, "Не вышло удалить", &е);
        }
        следи(&п2, |п, j| {
            if j["ok"].as_bool() == Some(false) {
                сообщи(п, "Удалить не удалось", &строка(&j, "error"));
            }
            п.сведения.borrow_mut().clear();
            рисуй(п);
        });
    });
}

/* ---------- экраны ---------- */

/// Есть ли выход в сеть: достучаться до Flathub или архива Ubuntu. Только
/// из рабочего потока — имя ищется и соединение ждёт
fn есть_сеть() -> bool {
    use std::net::{TcpStream, ToSocketAddrs};
    ["dl.flathub.org:443", "archive.ubuntu.com:80", "1.1.1.1:443"].iter().any(|адрес| {
        адрес.to_socket_addrs().ok().and_then(|mut а| а.next()).is_some_and(|а| TcpStream::connect_timeout(&а, Duration::from_secs(4)).is_ok())
    })
}

/// Проверить сеть и перерисовать, если ответ изменился
fn проверь_сеть(п: &П) {
    let (куда, откуда) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = куда.send_blocking(есть_сеть());
    });
    let п2 = Rc::downgrade(п);
    glib::MainContext::default().spawn_local(async move {
        let Ok(есть) = откуда.recv().await else { return };
        let Some(п) = п2.upgrade() else { return };
        if п.сеть.replace(Some(есть)) != Some(есть) {
            рисуй(&п);
        }
    });
}

/// Без сети Магазину показать нечего: подборка — это лишь названия, а
/// поставить ни одну программу не выйдет. Раньше здесь всё равно рисовались
/// карточки, и человек жал на них впустую. Теперь — честный экран, а сеть
/// проверяется снова каждые пять секунд: появилась — витрина вернётся сама
fn без_сети(п: &П) {
    let к = gtk::Box::new(gtk::Orientation::Vertical, 12);
    к.set_valign(gtk::Align::Center);
    к.set_halign(gtk::Align::Center);
    к.set_vexpand(true);
    к.set_margin_top(90);
    let знак = ярлык("📡", "h");
    знак.style_context().add_class("ico-big");
    знак.set_xalign(0.5);
    к.add(&знак);
    let з = ярлык("Нет подключения к интернету", "h");
    з.set_xalign(0.5);
    к.add(&з);
    let т = ярлык("Магазину нужна сеть, чтобы показывать и устанавливать программы. Подключитесь к Wi-Fi или кабелю — витрина появится сама.", "note");
    т.set_xalign(0.5);
    т.set_justify(gtk::Justification::Center);
    т.set_line_wrap(true);
    т.set_max_width_chars(60);
    к.add(&т);
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    ряд.set_halign(gtk::Align::Center);
    let снова = gtk::Button::with_label("Повторить");
    снова.style_context().add_class("big");
    let п2 = п.clone();
    снова.connect_clicked(move |_| проверь_сеть(&п2));
    ряд.add(&снова);
    let сеть = gtk::Button::with_label("Настройки сети");
    сеть.style_context().add_class("big");
    сеть.connect_clicked(|_| crate::core::открой_параметры("net"));
    ряд.add(&сеть);
    let моё = gtk::Button::with_label("Мои программы");
    моё.style_context().add_class("big");
    let п2 = п.clone();
    моё.connect_clicked(move |_| иди(&п2, Экран::Моё));
    ряд.add(&моё);
    к.add(&ряд);
    п.тело.add(&к);
    let (п2, поколение) = (Rc::downgrade(п), п.поколение.get());
    glib::timeout_add_local(Duration::from_secs(5), move || {
        let Some(п) = п2.upgrade() else { return glib::ControlFlow::Break };
        if п.поколение.get() != поколение {
            return glib::ControlFlow::Break;
        }
        проверь_сеть(&п);
        glib::ControlFlow::Continue
    });
}

fn ждём_сеть(п: &П) {
    let к = gtk::Box::new(gtk::Orientation::Vertical, 12);
    к.set_halign(gtk::Align::Center);
    к.set_margin_top(160);
    let к_ = gtk::Spinner::new();
    к_.start();
    к.add(&к_);
    к.add(&ярлык("Проверяю подключение…", "note"));
    п.тело.add(&к);
}

fn рисуй(п: &П) {
    п.поколение.set(п.поколение.get() + 1);
    очисти(&п.тело);
    let экран = п.экран.borrow().clone();
    // Своё на машине видно и без сети; остальное — только из сети
    let нужна_сеть = !matches!(экран, Экран::Моё);
    match п.сеть.get() {
        Some(false) if нужна_сеть => без_сети(п),
        None if нужна_сеть => ждём_сеть(п),
        _ => match &экран {
        Экран::Обзор => обзор(п),
        Экран::Разделы => разделы(п),
        Экран::Раздел(id) => раздел(п, id),
        Экран::Программа(прога) => программа(п, прога),
        Экран::Поиск(что) => поиск(п, что),
        Экран::Обновления => обновления(п),
        Экран::Моё => моё(п),
        },
    }
    п.окно.set_title(&match &экран {
        Экран::Программа(прога) => format!("{} — Магазин", прога.имя),
        _ => "Магазин".into(),
    });
    рисуй_полосу(п);
    п.тело.show_all();
    п.прокрутка.vadjustment().set_value(0.0);
}

fn рисуй_полосу(п: &П) {
    очисти(&п.полоса);
    let экран = п.экран.borrow().clone();
    let пункты: [(&str, &str, Экран); 5] = [
        ("🏠", "Главная", Экран::Обзор),
        ("🧩", "Разделы", Экран::Разделы),
        ("🎮", "Игры", Экран::Раздел("игры".into())),
        ("⬆️", "Обновления", Экран::Обновления),
        ("📚", "Моё", Экран::Моё),
    ];
    for (i, (знак, имя, куда)) in пункты.into_iter().enumerate() {
        if i == 3 {
            п.полоса.add(&gtk::Separator::new(gtk::Orientation::Horizontal));
        }
        let б = gtk::Button::new();
        let к = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let з = crate::apps::значок_эмодзи(знак, 20);
        з.style_context().add_class("z");
        let п_ = gtk::Label::new(Some(имя));
        п_.style_context().add_class("p");
        к.add(&з);
        к.add(&п_);
        б.add(&к);
        let на_нём = match (&экран, &куда) {
            (Экран::Обзор, Экран::Обзор) | (Экран::Разделы, Экран::Разделы) | (Экран::Обновления, Экран::Обновления) | (Экран::Моё, Экран::Моё) => true,
            (Экран::Раздел(а), Экран::Раздел(б)) => а == б,
            (Экран::Раздел(а), Экран::Разделы) => а != "игры",
            _ => false,
        };
        if на_нём {
            б.style_context().add_class("on");
        }
        let п2 = п.clone();
        б.connect_clicked(move |_| {
            п2.поле.set_text("");
            п2.назад.borrow_mut().clear();
            *п2.экран.borrow_mut() = куда.clone();
            рисуй(&п2);
        });
        п.полоса.add(&б);
    }
    п.полоса.show_all();
}

fn карточка_витрины(п: &П, в: &Value, большая: bool) -> gtk::Button {
    let б = gtk::Button::new();
    б.style_context().add_class("hero");
    if !большая {
        б.style_context().add_class("hero-small");
    }
    фон(&б, &строка(в, "фон"));
    let р = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    let т = gtk::Box::new(gtk::Orientation::Vertical, 6);
    т.set_hexpand(true);
    т.set_valign(gtk::Align::Center);
    let метка = строка(в, "ярлык");
    if !метка.is_empty() && большая {
        let м = gtk::Label::new(Some(&метка));
        м.style_context().add_class("tag");
        м.set_halign(gtk::Align::Start);
        т.add(&м);
    }
    т.add(&ярлык(&строка(в, "имя"), "name"));
    let л = ярлык(&строка(в, "строка"), "line");
    л.set_max_width_chars(40);
    т.add(&л);
    р.add(&т);
    let прога = Прога {
        пакет: строка(в, "пакет"),
        имя: строка(в, "имя"),
        про: строка(в, "строка"),
        знак: строка(в, "знак"),
        флатхаб: строка(в, "флатхаб"),
        источник: String::new(),
    };
    р.add(&значок(п, &прога, большая));
    б.add(&р);
    let п2 = п.clone();
    б.connect_clicked(move |_| открой_программу(&п2, прога.clone()));
    б
}

fn обзор(п: &П) {
    let витрина = п.каталог["витрина"].as_array().cloned().unwrap_or_default();
    let второй = п.каталог["второй_ряд"].as_array().cloned().unwrap_or_default();
    let верх = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    верх.set_homogeneous(true);
    let главная = gtk::Box::new(gtk::Orientation::Vertical, 8);
    верх.add(&главная);
    // Витрина листается сама, раз в семь секунд, и точками под ней
    let рисуй_главную: Rc<dyn Fn()> = {
        let (п2, главная, витрина) = (Rc::downgrade(п), главная.clone(), витрина.clone());
        Rc::new(move || {
            let Some(п) = п2.upgrade() else { return };
            очисти(&главная);
            if витрина.is_empty() {
                return;
            }
            let н = п.витрина.get() % витрина.len();
            let к = карточка_витрины(&п, &витрина[н], true);
            к.set_vexpand(true);
            главная.add(&к);
            let точки = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            точки.set_halign(gtk::Align::Center);
            for i in 0..витрина.len() {
                let т = gtk::Button::new();
                т.style_context().add_class("dot");
                if i == н {
                    т.style_context().add_class("on");
                }
                let п3 = п.clone();
                т.connect_clicked(move |_| {
                    п3.витрина.set(i);
                    рисуй(&п3);
                });
                точки.add(&т);
            }
            главная.add(&точки);
            главная.show_all();
        })
    };
    рисуй_главную();
    {
        let (п2, поколение, рисуй2) = (Rc::downgrade(п), п.поколение.get(), рисуй_главную.clone());
        glib::timeout_add_local(Duration::from_secs(7), move || {
            let Some(п) = п2.upgrade() else { return glib::ControlFlow::Break };
            if п.поколение.get() != поколение {
                return glib::ControlFlow::Break;
            }
            п.витрина.set(п.витрина.get() + 1);
            рисуй2();
            glib::ControlFlow::Continue
        });
    }
    let бок = gtk::Box::new(gtk::Orientation::Vertical, 10);
    if let Some(в) = второй.first() {
        бок.add(&карточка_витрины(п, в, false));
    }
    let пара = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    пара.set_homogeneous(true);
    for в in второй.iter().skip(1).take(2) {
        пара.add(&карточка_витрины(п, в, false));
    }
    бок.add(&пара);
    верх.add(&бок);
    п.тело.add(&верх);

    // Разделы кругляшами
    let чипы = gtk::FlowBox::new();
    чипы.set_selection_mode(gtk::SelectionMode::None);
    чипы.set_margin_top(16);
    for р in п.каталог["разделы"].as_array().cloned().unwrap_or_default() {
        let б = gtk::Button::new();
        let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        ряд.add(&crate::apps::значок_эмодзи(&строка(&р, "знак"), 16));
        ряд.add(&gtk::Label::new(Some(&строка(&р, "имя"))));
        б.add(&ряд);
        б.style_context().add_class("chip");
        let (п2, id) = (п.clone(), строка(&р, "id"));
        б.connect_clicked(move |_| иди(&п2, Экран::Раздел(id.clone())));
        чипы.add(&б);
    }
    п.тело.add(&чипы);

    for (id, имя) in [("сеть", "Популярные программы"), ("игры", "Популярные игры"), ("работа", "Для работы"), ("медиа", "Музыка и кино")] {
        let ш = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let з = ярлык(имя, "h");
        з.set_hexpand(true);
        ш.add(&з);
        let ещё = gtk::Button::with_label("Все ›");
        ещё.set_relief(gtk::ReliefStyle::None);
        ещё.set_valign(gtk::Align::Center);
        let (п2, id2) = (п.clone(), id.to_string());
        ещё.connect_clicked(move |_| иди(&п2, Экран::Раздел(id2.clone())));
        ш.add(&ещё);
        п.тело.add(&ш);
        let с = сетка();
        for з in п.каталог["подборки"][id].as_array().cloned().unwrap_or_default().iter().take(8) {
            с.add(&карточка(п, &из_записи(з)));
        }
        п.тело.add(&с);
    }
}

fn разделы(п: &П) {
    заголовок(п, "Разделы");
    let с = gtk::FlowBox::new();
    с.set_selection_mode(gtk::SelectionMode::None);
    с.set_row_spacing(12);
    с.set_column_spacing(12);
    for р in п.каталог["разделы"].as_array().cloned().unwrap_or_default() {
        let id = строка(&р, "id");
        let б = gtk::Button::new();
        б.style_context().add_class("tile");
        фон(&б, &строка(&р, "фон"));
        let к = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let з = gtk::Label::new(Some(&строка(&р, "знак")));
        з.style_context().add_class("z");
        з.set_halign(gtk::Align::Start);
        к.add(&з);
        к.add(&ярлык(&строка(&р, "имя"), "n"));
        let сколько = п.каталог["подборки"][&id].as_array().map(|а| а.len()).unwrap_or(0);
        к.add(&ярлык(&format!("{сколько} программ"), "a"));
        б.add(&к);
        let п2 = п.clone();
        б.connect_clicked(move |_| иди(&п2, Экран::Раздел(id.clone())));
        с.add(&б);
    }
    п.тело.add(&с);
}

fn кнопка_назад(п: &П) {
    let б = crate::apps::значок_кнопка("‹ Назад");
    б.set_halign(gtk::Align::Start);
    let п2 = п.clone();
    б.connect_clicked(move |_| {
        let прежний = п2.назад.borrow_mut().pop().unwrap_or(Экран::Обзор);
        *п2.экран.borrow_mut() = прежний;
        рисуй(&п2);
    });
    п.тело.add(&б);
}

fn раздел(п: &П, id: &str) {
    let р = п.каталог["разделы"].as_array().and_then(|а| а.iter().find(|р| строка(р, "id") == id).cloned()).unwrap_or(json!({}));
    if id != "игры" {
        кнопка_назад(п);
    }
    заголовок(п, &format!("{} {}", строка(&р, "знак"), строка(&р, "имя")));
    let с = сетка();
    for з in п.каталог["подборки"][id].as_array().cloned().unwrap_or_default() {
        с.add(&карточка(п, &из_записи(&з)));
    }
    п.тело.add(&с);
}

fn программа(п: &П, прога: &Прога) {
    кнопка_назад(п);
    let шапка = gtk::Box::new(gtk::Orientation::Horizontal, 20);
    шапка.set_margin_top(14);
    шапка.add(&значок(п, прога, true));
    let т = gtk::Box::new(gtk::Orientation::Vertical, 6);
    т.set_hexpand(true);
    т.set_valign(gtk::Align::Center);
    let и = ярлык(&прога.имя, "h");
    т.add(&и);
    т.add(&ярлык(&прога.про, "note"));
    шапка.add(&т);
    let место = gtk::Box::new(gtk::Orientation::Vertical, 6);
    место.set_valign(gtk::Align::Center);
    шапка.add(&место);
    п.тело.add(&шапка);
    let ниже = gtk::Box::new(gtk::Orientation::Vertical, 8);
    п.тело.add(&ниже);
    let ждём = gtk::Spinner::new();
    ждём.start();
    место.add(&ждём);

    let (место2, ниже2, прога2) = (место.clone(), ниже.clone(), прога.clone());
    let поколение = п.поколение.get();
    сведения(п, прога, false, move |п, о| {
        if п.поколение.get() != поколение {
            return;
        }
        очисти(&место2);
        if о["нет"].as_bool() == Some(true) {
            заметка(&место2, &строка(&о, "почему"));
            if строка(&о, "почему").contains("такого нет") {
                заметка(&ниже2, "Возможно, у программы другое имя в этом выпуске Ubuntu — попробуйте поиск сверху.");
            }
            место2.show_all();
            ниже2.show_all();
            return;
        }
        let стоит = стоит(&о["installed"]);
        let имя = { let и = строка(&о, "name"); if и.is_empty() { прога2.пакет.clone() } else { и } };
        let источник = строка(&о, "source");
        let кн = crate::apps::значок_кнопка(if стоит { "🗑 Удалить" } else { "⬇ Установить" });
        кн.style_context().add_class("big");
        if !стоит {
            кн.style_context().add_class("accent");
        }
        let заглушка = о["snap"].as_bool() == Some(true) && !стоит;
        if заглушка {
            кн.set_label("Недоступно");
            кн.set_sensitive(false);
        }
        кн.set_sensitive(!заглушка && !п.работа.get());
        let (п2, имя2, источник2, название) = (п.clone(), имя.clone(), источник.clone(), прога2.имя.clone());
        кн.connect_clicked(move |_| {
            if стоит {
                убери(&п2, имя2.clone(), источник2.clone(), &название);
            } else {
                поставь(&п2, имя2.clone(), источник2.clone());
            }
        });
        место2.add(&кн);
        if стоит {
            // Открыть поставленное — тем, чем его открывает система
            let п3 = п.clone();
            let пакет = имя.clone();
            let открыть = gtk::Button::with_label("Открыть");
            открыть.connect_clicked(move |_| запусти_пакет(&п3, &пакет));
            место2.add(&открыть);
        }

        let факты = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        факты.set_margin_top(16);
        let факт = |имя: &str, что: &str| {
            if что.is_empty() {
                return;
            }
            let ф = gtk::Box::new(gtk::Orientation::Vertical, 2);
            ф.style_context().add_class("fact");
            ф.add(&ярлык(имя, "k"));
            ф.add(&ярлык(что, "n"));
            факты.add(&ф);
        };
        факт("Источник", if источник == "flatpak" { "Flathub" } else { "Ubuntu" });
        let версия = [&о["installed"], &о["candidate"]].into_iter().filter_map(|в| в.as_str()).find(|в| !в.is_empty()).unwrap_or_default().to_string();
        факт("Версия", &версия);
        let р = размер(о["size"].as_u64().unwrap_or(0));
        факт("Размер", &if р.is_empty() { р } else { format!("{}{р}", if источник == "flatpak" { "скачает " } else { "займёт " }) });
        факт("Пакет", &имя);
        ниже2.add(&факты);
        let про = строка(&о, "about");
        if !про.is_empty() {
            ниже2.add(&ярлык(&про, "about"));
        }
        if о["snap"].as_bool() == Some(true) {
            заметка(&ниже2, "Это заглушка: программа ставится через Snap, а он в системе не работает.");
        }
        место2.show_all();
        ниже2.show_all();
    });
}

/// Запустить поставленную программу: ищем её ярлык по имени пакета
fn запусти_пакет(п: &П, пакет: &str) {
    let ниже = пакет.to_lowercase();
    let найдено = gio::AppInfo::all().into_iter().find(|а| {
        let id = а.id().map(|и| и.to_lowercase()).unwrap_or_default();
        id.trim_end_matches(".desktop") == ниже || id.contains(&ниже)
    });
    match найдено {
        Some(а) => {
            if let Err(е) = а.launch(&[], None::<&gio::AppLaunchContext>) {
                сообщи(п, "Не запустилась", &е.to_string());
            }
        }
        None => сообщи(п, "Ярлыка не нашлось", "Программа стоит, но в меню её нет — у неё может не быть окна."),
    }
}

fn поиск(п: &П, что: &str) {
    заголовок(п, &format!("Найдено по запросу «{что}»"));
    let место = gtk::Box::new(gtk::Orientation::Vertical, 8);
    п.тело.add(&место);
    if что.chars().count() < 2 {
        заметка(&место, "Хотя бы две буквы");
        return;
    }
    let ждём = gtk::Spinner::new();
    ждём.start();
    место.add(&ждём);
    let что = что.to_string();
    let поколение = п.поколение.get();
    let место2 = место.clone();
    узнай_состояние(п, move |п, с| {
        if let Err(почему) = можно_ставить(&с) {
            очисти(&место2);
            заметка(&место2, &почему);
            место2.show_all();
            return;
        }
        let списки = с["lists"].as_bool() == Some(true);
        if !списки {
            очисти(&место2);
            заметка(&место2, "Читаю списки пакетов — это делается один раз…");
            let ж = gtk::Spinner::new();
            ж.start();
            место2.add(&ж);
            место2.show_all();
        }
        // Только сама выдача: имя, строка о программе и стоит ли она. Раньше
        // следом по одной спрашивались подробности каждой из 24 программ —
        // по паре секунд на каждую, и результат человек ждал по минуте.
        // Версию и размер страница программы спросит сама, когда её откроют
        let (куда, откуда) = async_channel::bounded::<Result<Vec<Value>, String>>(1);
        std::thread::spawn(move || {
            if !списки && зов("pkg.update", json!({}), 30).is_ok() {
                жди_работу_блокируя();
            }
            let итог = зов("pkg.search", json!({ "query": что }), 90)
                .map(|р| р["list"].as_array().cloned().unwrap_or_default().into_iter().take(24).collect::<Vec<Value>>());
            let _ = куда.send_blocking(итог);
        });
        let п2 = п.clone();
        glib::MainContext::default().spawn_local(async move {
          while let Ok(итог) = откуда.recv().await {
            if списки == false {
                *п2.состояние.borrow_mut() = None;
            }
            if п2.поколение.get() != поколение {
                return;
            }
            очисти(&место2);
            match итог {
                Err(е) => заметка(&место2, &е),
                Ok(список) if список.is_empty() => заметка(&место2, "Ничего не нашлось. У программ бывают свои имена — попробуйте другое написание"),
                Ok(список) => {
                    let с = сетка();
                    for х in список {
                        let имя = строка(&х, "name");
                        п2.сведения.borrow_mut().insert(имя.clone(), х.clone());
                        let флатпак = строка(&х, "source") == "flatpak";
                        let mut про: Vec<String> = vec![строка(&х, "about"), if флатпак { "Flathub".into() } else { "Ubuntu".into() }];
                        if стоит(&х["installed"]) {
                            про.push("уже стоит".into());
                        }
                        про.push(размер(х["size"].as_u64().unwrap_or(0)));
                        let заголовок = строка(&х, "title");
                        let прога = Прога {
                            пакет: имя.clone(),
                            имя: if заголовок.is_empty() || заголовок == имя { имя.clone() } else { заголовок },
                            про: про.into_iter().filter(|с| !с.is_empty()).collect::<Vec<_>>().join(" · "),
                            знак: if флатпак { "🫙".into() } else { "📦".into() },
                            флатхаб: if флатпак { имя.clone() } else { String::new() },
                            источник: if флатпак { "flatpak".into() } else { String::new() },
                        };
                        с.add(&карточка(&п2, &прога));
                    }
                    место2.add(&с);
                }
            }
            место2.show_all();
          }
        });
    });
}

fn обновления(п: &П) {
    заголовок(п, "Обновления");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 8);
    п.тело.add(&место);
    заметка(&место, "Спрашиваю систему, что нового…");
    let ж = gtk::Spinner::new();
    ж.start();
    место.add(&ж);
    let (место2, п1) = (место.clone(), п.clone());
    спроси(п, "pkg.upgrade.check", json!({}), move |о| {
        очисти(&место2);
        let д = match о {
            Ok(д) => д,
            Err(е) => {
                заметка(&место2, &format!("Система не ответила про обновления: {е}"));
                место2.show_all();
                return;
            }
        };
        // Система обновляется при перезагрузке — здесь о ней одна строка
        let системных = д["list"].as_array().map(|а| а.len()).unwrap_or(0);
        if системных > 0 {
            let р = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            р.style_context().add_class("card");
            let т = gtk::Box::new(gtk::Orientation::Vertical, 2);
            т.set_hexpand(true);
            т.add(&ярлык(&format!("✨ Обновления системы: {системных}"), "n"));
            т.add(&ярлык("Ставятся при перезагрузке — меню питания → «Обновить и перезагрузить»", "a"));
            р.add(&т);
            let б = gtk::Button::with_label("Открыть");
            б.set_valign(gtk::Align::Center);
            б.connect_clicked(|_| {
                if std::process::Command::new("glower-settings").arg("update").spawn().is_err() {
                    скажи("параметры-раздел", json!("update"));
                }
            });
            р.add(&б);
            место2.add(&р);
        }
        let флатхаб = д["flathub"].as_array().cloned().unwrap_or_default();
        if флатхаб.is_empty() {
            заметка(&место2, "Все приложения обновлены");
        } else {
            let ш = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let з = ярлык(&format!("Программы Flathub · {}", флатхаб.len()), "h");
            з.set_hexpand(true);
            ш.add(&з);
            let б = crate::apps::значок_кнопка("⬆ Обновить");
            б.style_context().add_class("accent");
            б.set_valign(gtk::Align::Center);
            б.set_sensitive(!п1.работа.get());
            let п2 = п1.clone();
            б.connect_clicked(move |_| {
                let п3 = п2.clone();
                позови("pkg.upgrade.run", json!({ "source": "flatpak" }), move |о| match о {
                    Ok(_) => следи(&п3, |п, j| {
                        if j["ok"].as_bool() == Some(false) {
                            сообщи(п, "Обновить не удалось", &строка(&j, "error"));
                        }
                        рисуй(п);
                    }),
                    Err(е) => сообщи(&п3, "Не вышло начать", &е),
                });
            });
            ш.add(&б);
            место2.add(&ш);
            for х in флатхаб {
                let имя = { let и = строка(&х, "имя"); if и.is_empty() { строка(&х, "name") } else { и } };
                let станет = строка(&х, "станет");
                let прога = Прога { пакет: String::new(), имя, про: if станет.is_empty() { строка(&х, "name") } else { format!("будет {станет} · {}", строка(&х, "name")) }, знак: "🫙".into(), флатхаб: строка(&х, "name"), источник: "flatpak".into() };
                let к = карточка(&п1, &прога);
                место2.add(&к);
            }
        }
        место2.show_all();
    });
}

fn моё(п: &П) {
    заголовок(п, "Программы машины");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 8);
    п.тело.add(&место);
    let ж = gtk::Spinner::new();
    ж.start();
    место.add(&ж);
    let (место2, п1) = (место.clone(), п.clone());
    спроси(п, "sys.apps", json!({}), move |о| {
        очисти(&место2);
        let список = о.ok().and_then(|д| д["list"].as_array().cloned()).unwrap_or_default();
        if список.is_empty() {
            заметка(&место2, "Система не назвала ни одной программы");
        }
        let с = сетка();
        let тема = gtk::IconTheme::default();
        for а in список.iter().take(120) {
            let id = строка(а, "id");
            let имя = { let и = строка(а, "name"); if и.is_empty() { id.clone() } else { и } };
            let б = gtk::Button::new();
            б.style_context().add_class("card");
            let р = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            // Значок — настоящий, программы: по нему её и узнают
            let картинка = gio::DesktopAppInfo::new(&id)
                .and_then(|д| д.icon())
                .and_then(|и| тема.as_ref()?.lookup_by_gicon(&и, 48, gtk::IconLookupFlags::FORCE_SIZE))
                .and_then(|и| и.load_icon().ok());
            match картинка {
                Some(к) => р.add(&gtk::Image::from_pixbuf(Some(&к))),
                None => {
                    let з = gtk::Label::new(Some(if а["flatpak"].as_bool() == Some(true) { "🫙" } else { "🐧" }));
                    з.style_context().add_class("ico");
                    р.add(&з);
                }
            }
            let т = gtk::Box::new(gtk::Orientation::Vertical, 2);
            т.set_valign(gtk::Align::Center);
            let и = ярлык(&имя, "n");
            и.set_ellipsize(gtk::pango::EllipsizeMode::End);
            и.set_line_wrap(false);
            т.add(&и);
            let про = строка(а, "comment");
            let а_ = ярлык(if про.is_empty() { "Программа машины" } else { &про }, "a");
            а_.set_lines(2);
            а_.set_ellipsize(gtk::pango::EllipsizeMode::End);
            а_.set_max_width_chars(28);
            т.add(&а_);
            р.add(&т);
            б.add(&р);
            let (п2, id2) = (п1.clone(), id.clone());
            б.connect_clicked(move |_| {
                if let Some(д) = gio::DesktopAppInfo::new(&id2) {
                    if let Err(е) = д.launch(&[], None::<&gio::AppLaunchContext>) {
                        сообщи(&п2, "Не запустилась", &е.to_string());
                    }
                }
            });
            // Здесь же и удаляют: это список «моего»
            let (id3, имя3) = (id.clone(), имя.clone());
            б.connect_button_press_event(move |_, е| {
                if е.button() != 3 {
                    return glib::Propagation::Proceed;
                }
                let меню = gtk::Menu::new();
                let пункт = gtk::MenuItem::with_label("Удалить");
                let (id4, имя4) = (id3.clone(), имя3.clone());
                пункт.connect_activate(move |_| скажи("обзор-дело", json!({ "что": "удали", "id": id4, "имя": имя4, "вид": "машина" })));
                меню.append(&пункт);
                меню.show_all();
                меню.popup_at_pointer(Some(е));
                glib::Propagation::Stop
            });
            с.add(&б);
        }
        место2.add(&с);
        место2.show_all();
    });
}

/* ---------- окно ---------- */

pub fn запусти(доводы: Vec<String>) -> ! {
    подготовь("glower-store", "Магазин");
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль магазина");
    gtk::StyleContext::add_provider_for_screen(&gtk::gdk::Screen::default().expect("экран"), &стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_default_size(1060, 700);
    окно.set_icon_name(Some("system-software-install"));
    окно.style_context().add_class("glower-app");
    crate::apps::строка_заголовка(&окно);
    окно.style_context().add_class("store");

    let середина = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    окно.add(&середина);
    let полоса = gtk::Box::new(gtk::Orientation::Vertical, 4);
    полоса.style_context().add_class("rail");
    середина.add(&полоса);
    let правая = gtk::Box::new(gtk::Orientation::Vertical, 0);
    правая.set_hexpand(true);
    середина.add(&правая);
    let верх = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    верх.style_context().add_class("top");
    let поле = gtk::SearchEntry::new();
    поле.set_placeholder_text(Some("Найти программу: gimp, telegram, htop…"));
    поле.set_hexpand(true);
    верх.add(&поле);
    правая.add(&верх);
    let тело = gtk::Box::new(gtk::Orientation::Vertical, 6);
    тело.style_context().add_class("body");
    let прокрутка = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    прокрутка.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    прокрутка.set_vexpand(true);
    прокрутка.add(&тело);
    правая.add(&прокрутка);
    let ход = gtk::Revealer::new();
    let строка_хода = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    строка_хода.style_context().add_class("work");
    let ход_текст = gtk::Label::new(None);
    ход_текст.set_xalign(0.0);
    ход_текст.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let ход_полоса = gtk::ProgressBar::new();
    ход_полоса.set_hexpand(true);
    ход_полоса.set_valign(gtk::Align::Center);
    строка_хода.add(&ход_текст);
    строка_хода.add(&ход_полоса);
    ход.add(&строка_хода);
    правая.add(&ход);

    let каталог: Value = serde_json::from_str(КАТАЛОГ).expect("каталог магазина");
    let mut первый = Экран::Обзор;
    for д in &доводы {
        if let Some(что) = д.strip_prefix("--find=") {
            поле.set_text(что);
            первый = Экран::Поиск(что.to_string());
        } else if д == "updates" {
            первый = Экран::Обновления;
        } else if д == "installed" {
            первый = Экран::Моё;
        }
    }
    let п: П = Rc::new(Окно {
        окно: окно.clone(),
        полоса,
        тело,
        прокрутка,
        поле: поле.clone(),
        экран: RefCell::new(первый),
        назад: RefCell::new(Vec::new()),
        поколение: Cell::new(0),
        каталог,
        состояние: RefCell::new(None),
        сведения: RefCell::new(HashMap::new()),
        значки: RefCell::new(HashMap::new()),
        работа: Cell::new(false),
        ход,
        ход_текст,
        ход_полоса,
        витрина: Cell::new(0),
        сеть: Cell::new(None),
    });

    {
        let п1 = п.clone();
        поле.connect_activate(move |поле| {
            let что = поле.text().trim().to_string();
            if что.is_empty() {
                return;
            }
            иди(&п1, Экран::Поиск(что));
        });
        let п1 = п.clone();
        поле.connect_search_changed(move |поле| {
            if поле.text().is_empty() && matches!(*п1.экран.borrow(), Экран::Поиск(_)) {
                *п1.экран.borrow_mut() = п1.назад.borrow_mut().pop().unwrap_or(Экран::Обзор);
                рисуй(&п1);
            }
        });
    }
    // Работа могла начаться не здесь: щёлкнули по .deb, а Магазин закрыт
    {
        let п1 = п.clone();
        позови("pkg.job", json!({}), move |о| {
            if о.is_ok_and(|j| j["running"].as_bool() == Some(true)) {
                следи(&п1, |п, _| рисуй(п));
            }
        });
    }
    // Просили открыть Магазин, а он уже открыт: на нужную вкладку
    {
        let вести = слушай(&[], &["магазин-открой"]);
        let п1 = Rc::downgrade(&п);
        glib::MainContext::default().spawn_local(async move {
            while let Ok(в) = вести.recv().await {
                let Some(п) = п1.upgrade() else { break };
                let найти = строка(&в.что, "найти");
                let вкладка = строка(&в.что, "вкладка");
                let куда = if !найти.is_empty() {
                    п.поле.set_text(&найти);
                    Экран::Поиск(найти)
                } else if вкладка == "updates" {
                    Экран::Обновления
                } else if вкладка == "installed" {
                    Экран::Моё
                } else {
                    continue;
                };
                иди(&п, куда);
                п.окно.present();
            }
        });
    }
    окно.connect_key_press_event({
        let поле = поле.clone();
        move |_, е| {
            if е.state().contains(gtk::gdk::ModifierType::CONTROL_MASK) && crate::notepad::латинская(е) == "f" {
                поле.grab_focus();
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
    п.ход.set_reveal_child(false);
    рисуй(&п);
    проверь_сеть(&п);
    gtk::main();
    std::process::exit(0);
}

#[cfg(test)]
mod проверки {
    use super::*;

    #[test]
    fn каталог_читается_и_полон() {
        let к: Value = serde_json::from_str(КАТАЛОГ).unwrap();
        let разделы = к["разделы"].as_array().unwrap();
        assert_eq!(разделы.len(), 7);
        for р in разделы {
            let id = строка(р, "id");
            assert!(к["подборки"][&id].as_array().is_some_and(|а| а.len() >= 10), "{id}");
        }
        assert_eq!(к["витрина"].as_array().unwrap().len(), 4);
        let steam = к["подборки"]["игры"].as_array().unwrap().iter().map(из_записи).find(|п| п.имя == "Steam").unwrap();
        assert_eq!(steam.источник, "flatpak");
    }

    #[test]
    fn размеры_по_русски() {
        assert_eq!(размер(0), "");
        assert_eq!(размер(2048), "2 КБ");
        assert_eq!(размер(5 << 20), "5,0 МБ");
    }
}
