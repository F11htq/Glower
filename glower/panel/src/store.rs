//! Магазин — нативный, настоящим окном (glower-store).
//!
//! Слева — разделы: Главная, Библиотека, Загрузки и обновления, Желаемое,
//! категории и источник программ. Справа — витрина, «Топ по загрузкам»,
//! «Лучшие по оценкам», подборки и все программы с сортировкой; страница
//! программы со скриншотами, оценками и отзывами людей; поиск с
//! подсказками; внизу — строка с тем, что система делает прямо сейчас.
//!
//! Всё, что здесь видно, — настоящее: описания, скриншоты, число установок и
//! подборки — с Flathub, оценки и отзывы — из ODRS (их же показывают GNOME
//! Software и KDE Discover), версии и размеры — у самой машины. Сводит это
//! агент (agent/store.mjs), а ставит и удаляет — он же (agent/packages.mjs).
//!
//! Тема, акцент, прозрачность и язык — из Параметров, как у всех программ
//! системы. Источник программ и «только свободные» — тоже из Параметров
//! (Приложения → Магазин), и переключатель источника здесь пишет туда же.
//!
//! Наши подборки лежат в store-catalog.json — том же, по которому
//! проверяется Магазин (см. test/linux.mjs): список один.

use crate::agent::{зов, позови, скажи, слушай};
use crate::apps::подготовь;
use base64::Engine;
use gtk::gdk_pixbuf::{InterpType, Pixbuf, PixbufLoader};
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

const КАТАЛОГ: &str = include_str!("store-catalog.json");
const ЖЕЛАЕМОЕ: &str = "магазин-желаемое.json";
const ИСТОРИЯ: &str = "магазин-история.json";

/* Стиль — именами цветов системы (@glower_…): тёмная и светлая тема и
   акцент из Параметров меняют Магазин вместе со всеми окнами. Анимаций
   почти нет: витрина сменяется плавным переходом, и только */
const СТИЛЬ: &str = r#"
.store .side { background-color: @glower_bg2; border-right: 1px solid alpha(@glower_fg, 0.07); padding: 10px 8px; }
.store .brand { padding: 4px 10px 12px; }
.store .brand label { font-size: 16px; font-weight: 700; }
.store .side button.nav { padding: 6px 10px; border-radius: 9px; background: none; box-shadow: none; border: none; }
.store .side button.nav label { font-size: 13px; }
.store .side.narrow button.nav { padding: 8px; }
.store .side.narrow .brand { padding: 4px 0 12px; }
.store .side button.nav:hover { background-color: alpha(@glower_fg, 0.07); }
.store .side button.nav.on { background-color: @glower_accent; }
.store .side button.nav.on label, .store .side button.nav.on image { color: #ffffff; }
.store .side button.nav.on .badge { background-color: #ffffff; color: @glower_accent; }
.store .side .group { font-size: 11px; opacity: 0.6; margin: 12px 10px 3px; }
.store .badge { background-color: @glower_accent2; color: #ffffff; border-radius: 10px; font-size: 11px; font-weight: 600; padding: 0 7px; }
.store .plate { margin: 12px 2px 0; padding: 12px; border-radius: 14px; background-image: linear-gradient(135deg, @glower_accent, @glower_accent2); color: #ffffff; }
.store .plate .n { font-weight: 700; font-size: 14px; }
.store .plate button { background-color: #ffffff; border-radius: 17px; border: none; box-shadow: none; margin-top: 8px; }
.store .plate button label { color: #1d1836; font-weight: 600; }
.store .seg { background-color: alpha(@glower_fg, 0.07); border-radius: 10px; padding: 3px; }
.store .seg button { border: none; background: none; box-shadow: none; padding: 5px 8px; border-radius: 8px; font-size: 12px; opacity: 0.75; }
.store .seg button.on { background-color: @glower_accent; opacity: 1; }
.store .seg button.on label { color: #ffffff; }
.store .top { padding: 10px 22px; border-bottom: 1px solid alpha(@glower_fg, 0.07); }
.store .body { padding: 20px 22px 40px; }
.store .h { font-size: 18px; font-weight: 700; margin: 22px 2px 10px; }
.store .h.first { margin-top: 0; }
.store .hero { border-radius: 20px; padding: 26px; min-height: 170px; color: #ffffff; }
.store .hero .tag { font-size: 12px; opacity: 0.85; font-weight: 600; }
.store .hero .name { font-size: 28px; font-weight: 700; }
.store .hero .line { font-size: 14px; opacity: 0.92; }
.store .hero button.get { background-color: #ffffff; }
.store .hero button.get label { color: #1d1836; }
.store .hero button.ghost { border-color: rgba(255,255,255,0.6); }
.store .hero button.ghost label { color: #ffffff; }
.store .dot { min-width: 8px; min-height: 8px; border-radius: 4px; padding: 0; border: none; background-color: alpha(@glower_fg, 0.25); }
.store .dot.on { min-width: 22px; background-color: @glower_accent; }
.store .card, .store .row { border-radius: 16px; padding: 12px 14px; background-color: alpha(@glower_fg, 0.045); border: 1px solid alpha(@glower_fg, 0.07); }
.store button.card:hover { background-color: alpha(@glower_fg, 0.08); }
.store .card .n, .store .row .n { font-weight: 600; font-size: 15px; }
.store .card .a, .store .row .a { font-size: 12px; opacity: 0.65; }
.store .rank { font-weight: 700; color: @glower_accent2; }
.store .stars { color: #ff9f43; }
.store .ico { border-radius: 14px; min-width: 52px; min-height: 52px; font-size: 26px; }
.store .ico-big { min-width: 112px; min-height: 112px; font-size: 52px; border-radius: 30px; }
.store button.get { background-image: none; background-color: @glower_accent; border-radius: 17px; border: none; box-shadow: none; padding: 6px 16px; min-width: 84px; }
.store button.get label { color: #ffffff; font-weight: 600; }
.store button.get:hover { background-color: shade(@glower_accent, 1.08); }
.store button.get.open, .store button.get.busy { background-color: alpha(@glower_fg, 0.10); }
.store button.get.open label, .store button.get.busy label { color: @glower_fg; }
.store button.ghost { border-radius: 17px; border: 1px solid alpha(@glower_fg, 0.15); background: none; box-shadow: none; padding: 6px 14px; }
.store button.ghost.on { color: @glower_accent2; border-color: @glower_accent2; }
.store .facts { border: 1px solid alpha(@glower_fg, 0.08); border-radius: 16px; background-color: alpha(@glower_fg, 0.03); }
.store .facts > box { padding: 12px; border-right: 1px solid alpha(@glower_fg, 0.08); }
.store .facts .b { font-weight: 700; font-size: 16px; }
.store .facts .k { font-size: 12px; opacity: 0.6; }
.store .shot { border-radius: 12px; border: 1px solid alpha(@glower_fg, 0.08); background: none; padding: 0; }
.store .long { font-size: 14px; opacity: 0.85; }
.store .info .k { opacity: 0.6; }
.store .rev { border-radius: 14px; padding: 12px 14px; background-color: alpha(@glower_fg, 0.045); border: 1px solid alpha(@glower_fg, 0.07); }
.store .bars progressbar trough { min-height: 6px; border-radius: 3px; background-color: alpha(@glower_fg, 0.10); }
.store .bars progressbar progress { min-height: 6px; border-radius: 3px; background-color: #ff9f43; background-image: none; }
.store .note { font-size: 13px; opacity: 0.65; margin: 8px 2px; }
.store .empty { opacity: 0.65; margin: 30px 0; }
.store .big { font-size: 15px; padding: 10px 22px; min-width: 140px; }
.store .status { padding: 5px 14px; background-color: @glower_bg2; border-top: 1px solid alpha(@glower_fg, 0.07); font-family: monospace; font-size: 12px; }
.store .status .dollar { color: @glower_accent2; font-weight: 700; }
.store .status progressbar trough { min-height: 4px; border-radius: 2px; background-color: alpha(@glower_fg, 0.10); }
.store .status progressbar progress { min-height: 4px; border-radius: 2px; background-image: linear-gradient(90deg, @glower_accent, @glower_accent2); }
.store .dlbar trough { min-height: 6px; border-radius: 3px; background-color: alpha(@glower_fg, 0.10); }
.store .dlbar progress { min-height: 6px; border-radius: 3px; background-color: @glower_accent; background-image: none; }
.store .toast { background-color: @glower_bg3; border: 1px solid alpha(@glower_fg, 0.12); border-radius: 14px; padding: 10px 14px; min-width: 230px; }
.store .toast .n { font-weight: 700; }
.store .sug { padding: 4px; }
.store .sug button { border: none; background: none; box-shadow: none; padding: 6px 10px; border-radius: 8px; }
.store .sug button:hover { background-color: alpha(@glower_fg, 0.08); }
"#;

/// Программа, как её знает Магазин: из подборки, витрины, Flathub или поиска
#[derive(Clone, Debug, Default)]
struct Прога {
    пакет: String,
    имя: String,
    про: String,
    знак: String,
    флатхаб: String,
    источник: String,
    /// Средняя оценка и сколько оценок (ODRS)
    оценка: Option<(f64, u64)>,
    /// Установок за месяц (Flathub)
    за_месяц: u64,
    /// Свободная лицензия; None — не знаем
    свободная: Option<bool>,
}

impl Прога {
    /// Под каким именем её знает агент и ODRS
    fn ключ(&self) -> String {
        if !self.флатхаб.is_empty() { self.флатхаб.clone() } else { self.пакет.clone() }
    }
    fn с_flathub(&self) -> bool {
        self.источник == "flatpak"
    }
    fn в_json(&self) -> Value {
        json!({ "п": self.пакет, "имя": self.имя, "про": self.про, "зн": self.знак, "ф": self.флатхаб, "и": self.источник })
    }
}

#[derive(Clone, Debug)]
enum Экран {
    Обзор,
    Раздел(String),
    Программа(Прога),
    Поиск(String),
    Загрузки,
    Библиотека,
    Желаемое,
}

struct Окно {
    окно: gtk::Window,
    бок: gtk::Box,
    тело: gtk::Box,
    прокрутка: gtk::ScrolledWindow,
    поле: gtk::SearchEntry,
    подсказки: gtk::Popover,
    подсказки_список: gtk::Box,
    тосты: gtk::Box,
    экран: RefCell<Экран>,
    назад: RefCell<Vec<Экран>>,
    поколение: Cell<u64>,
    каталог: Value,
    состояние: RefCell<Option<Value>>,
    сведения: RefCell<HashMap<String, Value>>,
    значки: RefCell<HashMap<String, Option<Pixbuf>>>,
    работа: Cell<bool>,
    /// Какой пакет сейчас ставится или удаляется — его кнопки показывают
    /// ход на месте «Установить», как Microsoft Store
    в_работе: RefCell<String>,
    /// Ход на странице программы: подпись и полоса
    кнопка_хода: RefCell<Option<(gtk::Label, gtk::ProgressBar)>>,
    /// Кнопки «Установить» на карточках: по имени пакета
    кнопки: RefCell<Vec<(String, glib::WeakRef<gtk::Button>)>>,
    /// Звёзды на карточках, которым оценка ещё не пришла
    звёзды: RefCell<Vec<(String, glib::WeakRef<gtk::Label>)>>,
    /// Что стоит на машине: имена пакетов apt (поставленных руками) и id Flathub
    стоят: RefCell<HashSet<String>>,
    оценки: RefCell<HashMap<String, (f64, u64)>>,
    /// Все программы, что Магазин видел, — для подсказок поиска
    известные: RefCell<Vec<Прога>>,
    обновления: RefCell<Vec<Value>>,
    системных: Cell<usize>,
    желаемое: RefCell<Vec<Прога>>,
    история: RefCell<Vec<Value>>,
    статус: gtk::Label,
    статус_полоса: gtk::ProgressBar,
    витрина: Cell<usize>,
    сортировка: RefCell<String>,
    /// Есть ли интернет: None — ещё проверяем
    сеть: Cell<Option<bool>>,
}

type П = Rc<Окно>;

/* ---------- настройки из Параметров ----------
   Пишет их ядро панели (оно одно владеет settings.json): просим его через
   «параметры-дело», а читаем файл сами — он маленький */

fn настройки() -> Value {
    std::fs::read_to_string(glib::user_config_dir().join("glower").join("settings.json"))
        .ok()
        .and_then(|т| serde_json::from_str(&т).ok())
        .unwrap_or(Value::Null)
}

/// Боковая полоса: шире — со словами, в узком окне — одни значки, как
/// в макете на телефоне. Слова тогда — в подсказке под мышью
const ШИРЕ: i32 = 210;
const УЖЕ: i32 = 60;
/// Окно уже этого — полоса сворачивается до значков
const ПОРОГ_ОКНА: i32 = 1000;

thread_local! {
    static УЗКО: Cell<bool> = const { Cell::new(false) };
    /// Источник, выбранный здесь, — пока ядро панели не записало его в файл
    static ВЫБРАН: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Откуда показывать программы: all — отовсюду, apt — Ubuntu, flatpak — Flathub
fn источник() -> String {
    if let Some(и) = ВЫБРАН.with(|в| в.borrow().clone()) {
        return и;
    }
    match настройки()["storeSource"].as_str() {
        Some(и @ ("apt" | "flatpak")) => и.to_string(),
        _ => "all".into(),
    }
}

fn только_свободные() -> bool {
    настройки()["storeFreeOnly"].as_bool() == Some(true)
}

fn задай_настройку(k: &str, v: Value) {
    скажи("параметры-дело", json!({ "что": "set", "k": k, "v": v }));
}

/// Видна ли программа при нынешних настройках
fn видна(прога: &Прога) -> bool {
    let по_источнику = match источник().as_str() {
        "apt" => !прога.с_flathub(),
        "flatpak" => !прога.флатхаб.is_empty(),
        _ => true,
    };
    по_источнику && !(только_свободные() && прога.свободная == Some(false))
}

fn язык() -> String {
    crate::core::язык_сейчас()
}

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
        ..Default::default()
    }
}

fn оценка_из(в: &Value) -> Option<(f64, u64)> {
    let с = в["средняя"].as_f64()?;
    Some((с, в["всего"].as_u64().unwrap_or(0)))
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

/// 3 966 300 → «4,0 млн», 52 953 → «53 тыс.»
fn сколько(н: u64) -> String {
    if н >= 1_000_000 {
        format!("{:.1} {}", н as f64 / 1e6, crate::i18n::tr("млн")).replace('.', ",")
    } else if н >= 1000 {
        format!("{} {}", (н as f64 / 1000.0).round(), crate::i18n::tr("тыс."))
    } else {
        н.to_string()
    }
}

fn звёзды_текст(о: (f64, u64)) -> String {
    format!("★ {:.1}", о.0).replace('.', ",")
}

/// Те же правила, что у агента (вес_оценки в agent/store.mjs): три пятёрки
/// весят меньше, чем 4,7 из тысячи
fn вес(о: Option<(f64, u64)>) -> f64 {
    match о {
        Some((с, н)) => (с * н as f64 + 3.8 * 50.0) / (н as f64 + 50.0),
        None => 0.0,
    }
}

fn ярлык(текст: &str, класс: &str) -> gtk::Label {
    let л = gtk::Label::new(Some(текст));
    л.set_xalign(0.0);
    л.set_line_wrap(true);
    л.set_line_wrap_mode(gtk::pango::WrapMode::WordChar);
    if !класс.is_empty() {
        for к in класс.split(' ') {
            л.style_context().add_class(к);
        }
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
    let к = картинка_как_есть(адрес)?;
    к.scale_simple(размер, размер, InterpType::Bilinear)
}

fn картинка_как_есть(адрес: &str) -> Option<Pixbuf> {
    let (_, данные) = адрес.split_once(";base64,")?;
    let байты = base64::engine::general_purpose::STANDARD.decode(данные.trim()).ok()?;
    let чтец = PixbufLoader::new();
    чтец.write(&байты).ok()?;
    чтец.close().ok()?;
    чтец.pixbuf()
}

fn очисти(к: &impl IsA<gtk::Container>) {
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
    crate::agent::позови_за(метод, доводы, 60, move |о| {
        if п2.upgrade().is_some_and(|п| п.поколение.get() == поколение) {
            дело(о);
        }
    });
}

/// Всплывающая записка в углу окна — на три с половиной секунды
fn тост(п: &П, заголовок: &str, текст: &str) {
    let к = gtk::Box::new(gtk::Orientation::Vertical, 2);
    к.style_context().add_class("toast");
    к.add(&ярлык(заголовок, "n"));
    if !текст.is_empty() {
        к.add(&ярлык(текст, "a"));
    }
    let о = gtk::Revealer::new();
    о.set_transition_type(gtk::RevealerTransitionType::Crossfade);
    о.add(&к);
    п.тосты.add(&о);
    о.show_all();
    о.set_reveal_child(true);
    glib::timeout_add_local_once(Duration::from_millis(3500), move || {
        о.set_reveal_child(false);
        glib::timeout_add_local_once(Duration::from_millis(300), move || unsafe { о.destroy() });
    });
}

/* ---------- лицо программы ----------

   Сперва — машина: у поставленных программ значок на диске, а к спискам
   пакетов apt кладёт каталог значков для магазинов. Нет там — Flathub,
   его значки приносит агент. Нет и там — наш знак. Ради значка никто не
   ждёт: он подменяет знак, когда придёт. */

fn значок(п: &П, прога: &Прога, большой: bool) -> gtk::Widget {
    let размер = if большой { 112 } else { 52 };
    let место = gtk::Stack::new();
    место.set_valign(gtk::Align::Center);
    // Пока настоящий значок программы не пришёл — значок темы по её роду
    // (символ шрифта — только если пары в теме нет)
    let символ = if прога.знак.is_empty() { "📦" } else { прога.знак.as_str() };
    let знак: gtk::Widget = match crate::apps::имя_значка(символ) {
        Some(имя) => {
            let к = gtk::Image::from_icon_name(Some(имя), gtk::IconSize::Dialog);
            к.set_pixel_size(if большой { 52 } else { 26 });
            к.upcast()
        }
        None => gtk::Label::new(Some(символ)).upcast(),
    };
    знак.style_context().add_class("ico");
    if большой {
        знак.style_context().add_class("ico-big");
    }
    фон(&знак, "linear-gradient(140deg, rgba(127,127,127,0.20), rgba(127,127,127,0.08))");
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

fn заголовок(п: &П, текст: &str) -> gtk::Label {
    let з = ярлык(текст, "h");
    п.тело.add(&з);
    з
}

fn заметка(к: &gtk::Box, текст: &str) {
    к.add(&ярлык(текст, "note"));
}

fn пусто(к: &gtk::Box, текст: &str) {
    let л = ярлык(текст, "empty");
    л.set_xalign(0.5);
    л.set_justify(gtk::Justification::Center);
    к.add(&л);
}

fn ждём(к: &gtk::Box) {
    let ж = gtk::Spinner::new();
    ж.start();
    ж.set_halign(gtk::Align::Start);
    к.add(&ж);
}

/// Перемешать на месте (Фишер — Йетс)
fn перемешай<T>(в: &mut [T]) {
    for i in (1..в.len()).rev() {
        let j = glib::random_int_range(0, i as i32 + 1) as usize;
        в.swap(i, j);
    }
}

/// Программа из ответа Flathub (раздел, поиск, подборка)
fn из_flathub(х: &Value) -> Прога {
    let имя = строка(х, "name");
    let заголовок = строка(х, "title");
    Прога {
        пакет: имя.clone(),
        имя: if заголовок.is_empty() { имя.clone() } else { заголовок },
        про: строка(х, "about"),
        знак: "🫙".into(),
        флатхаб: имя,
        источник: "flatpak".into(),
        оценка: оценка_из(&х["оценка"]),
        за_месяц: х["за_месяц"].as_u64().unwrap_or(0),
        свободная: х["свободная"].as_bool(),
    }
}

/// Запомнить программу для подсказок поиска
fn знаю(п: &П, прога: &Прога) {
    let mut и = п.известные.borrow_mut();
    if !и.iter().any(|х| х.ключ() == прога.ключ()) {
        и.push(прога.clone());
    }
}

/// Страница раздела с Flathub — в отдельном потоке, ответ в главном цикле
fn страница_раздела(раздел: &str, страница: u32, дело: impl FnOnce(Result<Value, String>) + 'static) {
    crate::agent::позови_за("pkg.category", json!({ "раздел": раздел, "страница": страница }), 40, дело);
}

fn подборка_flathub(вид: &str, страница: u32, сколько: u32, дело: impl FnOnce(Result<Value, String>) + 'static) {
    crate::agent::позови_за("store.collection", json!({ "вид": вид, "страница": страница, "сколько": сколько, "язык": язык() }), 40, дело);
}

fn сетка() -> gtk::FlowBox {
    let с = gtk::FlowBox::new();
    с.set_selection_mode(gtk::SelectionMode::None);
    с.set_row_spacing(12);
    с.set_column_spacing(12);
    с.set_homogeneous(true);
    с.set_min_children_per_line(1);
    с.set_max_children_per_line(4);
    с
}

/// Ряд карточек, который листается вбок
fn лента() -> (gtk::ScrolledWindow, gtk::Box) {
    let л = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    л.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    л.set_propagate_natural_height(true);
    let р = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    р.set_margin_bottom(8);
    л.add(&р);
    (л, р)
}

/// Подпись кнопки установки для программы — по тому, что знает Магазин
fn оформи_кнопку(п: &П, б: &gtk::Button, прога: &Прога) {
    let ктх = б.style_context();
    ктх.remove_class("open");
    ктх.remove_class("busy");
    let имя = прога.ключ();
    if п.работа.get() && (*п.в_работе.borrow() == имя || *п.в_работе.borrow() == прога.пакет) {
        ктх.add_class("busy");
        б.set_label("…");
    } else if п.стоят.borrow().contains(&имя) || п.стоят.borrow().contains(&прога.пакет) {
        ктх.add_class("open");
        б.set_label(&crate::i18n::tr("Открыть"));
    } else {
        б.set_label(&crate::i18n::tr("Установить"));
    }
}

/// Кнопка «Установить / Открыть / 45 %» — на карточке и в витрине
fn кнопка_установки(п: &П, прога: &Прога) -> gtk::Button {
    let б = gtk::Button::new();
    б.style_context().add_class("get");
    б.set_valign(gtk::Align::Center);
    оформи_кнопку(п, &б, прога);
    п.кнопки.borrow_mut().push((прога.ключ(), б.downgrade()));
    let (п2, прога2) = (Rc::downgrade(п), прога.clone());
    б.connect_clicked(move |б| {
        let Some(п) = п2.upgrade() else { return };
        if б.style_context().has_class("busy") {
            return;
        }
        if б.style_context().has_class("open") {
            return запусти_пакет(&п, &прога2.ключ());
        }
        if п.работа.get() {
            return тост(&п, "Сначала дождитесь конца текущей установки", "");
        }
        if прога2.с_flathub() {
            оформи_все_кнопки_для(&п, &прога2.ключ(), "…");
            поставь(&п, прога2.ключ(), "flatpak".into(), прога2.имя.clone());
        } else {
            // У пакетов Ubuntu имя в подборке бывает старым — сперва
            // спрашиваем машину, как он называется на самом деле
            let (п3, прога3) = (п.clone(), прога2.clone());
            сведения(&п, &прога2, false, move |п, о| {
                if о["нет"].as_bool() == Some(true) {
                    return сообщи(п, &format!("{} не поставить", прога3.имя), &строка(&о, "почему"));
                }
                let имя = { let и = строка(&о, "name"); if и.is_empty() { прога3.пакет.clone() } else { и } };
                поставь(&п3, имя, строка(&о, "source"), прога3.имя.clone());
            });
        }
    });
    б
}

fn оформи_все_кнопки_для(п: &П, имя: &str, текст: &str) {
    for (к, б) in п.кнопки.borrow().iter() {
        if к == имя {
            if let Some(б) = б.upgrade() {
                б.style_context().add_class("busy");
                б.set_label(текст);
            }
        }
    }
}

fn обнови_кнопки(п: &П) {
    п.кнопки.borrow_mut().retain(|(_, б)| б.upgrade().is_some());
    let кнопки = п.кнопки.borrow().clone();
    let все: Vec<Прога> = п.известные.borrow().clone();
    for (к, б) in кнопки {
        let Some(б) = б.upgrade() else { continue };
        let прога = все.iter().find(|х| х.ключ() == к).cloned().unwrap_or(Прога { пакет: к.clone(), ..Default::default() });
        оформи_кнопку(п, &б, &прога);
    }
}

/// «★ 4,7» — звёзды дорисуются, когда придут оценки
fn звёзды_карточки(п: &П, прога: &Прога) -> gtk::Label {
    let з = ярлык("", "a stars");
    з.set_line_wrap(false);
    let оценка = прога.оценка.or_else(|| п.оценки.borrow().get(&прога.ключ()).copied());
    match оценка {
        Some(о) => з.set_text(&звёзды_текст(о)),
        None => п.звёзды.borrow_mut().push((прога.ключ(), з.downgrade())),
    }
    з
}

/// Строка о программе — в две строки, с многоточием
fn подпись_карточки(текст: &str) -> gtk::Label {
    let а = ярлык(текст, "a");
    а.set_lines(2);
    а.set_ellipsize(gtk::pango::EllipsizeMode::End);
    а.set_max_width_chars(26);
    а
}

/// Карточка программы: значок, имя (и место в топе), кнопка, оценка и строка о ней
fn карточка_с_местом(п: &П, прога: &Прога, место: Option<usize>) -> gtk::Widget {
    знаю(п, прога);
    let б = gtk::Button::new();
    б.style_context().add_class("card");
    б.set_relief(gtk::ReliefStyle::None);
    let р = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    р.add(&значок(п, прога, false));
    let т = gtk::Box::new(gtk::Orientation::Vertical, 3);
    т.set_valign(gtk::Align::Center);
    т.set_hexpand(true);
    let верх = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    if let Some(м) = место {
        let к = ярлык(&м.to_string(), "rank");
        к.set_line_wrap(false);
        верх.add(&к);
    }
    let и = ярлык(&прога.имя, "n");
    и.set_ellipsize(gtk::pango::EllipsizeMode::End);
    и.set_line_wrap(false);
    и.set_max_width_chars(16);
    верх.add(&и);
    верх.add(&звёзды_карточки(п, прога));
    т.add(&верх);
    // В топе — за что место: сколько раз её поставили за месяц
    if место.is_some() && прога.за_месяц > 0 {
        т.add(&подпись_карточки(&format!("{} установок за месяц", сколько(прога.за_месяц))));
    } else {
        т.add(&подпись_карточки(&прога.про));
    }
    р.add(&т);
    р.add(&кнопка_установки(п, прога));
    б.add(&р);
    б.set_size_request(330, -1);
    let (п2, прога2) = (п.clone(), прога.clone());
    б.connect_clicked(move |_| открой_программу(&п2, прога2.clone()));
    б.upcast()
}

fn карточка(п: &П, прога: &Прога) -> gtk::Widget {
    карточка_с_местом(п, прога, None)
}

fn иди(п: &П, куда: Экран) {
    п.подсказки.popdown();
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

/// Что стоит на машине — по этому карточки пишут «Открыть»
fn узнай_поставленное(п: &П) {
    let п2 = Rc::downgrade(п);
    crate::agent::позови_за("pkg.installed", json!({}), 60, move |о| {
        let Some(п) = п2.upgrade() else { return };
        let Ok(о) = о else { return };
        let все: HashSet<String> = о["list"].as_array().cloned().unwrap_or_default().iter().map(|х| строка(х, "name")).filter(|и| !и.is_empty()).collect();
        *п.стоят.borrow_mut() = все;
        обнови_кнопки(&п);
    });
}

/// Обновления: сколько их — для значка в боковой полосе и плашки
fn узнай_обновления(п: &П, дело: impl FnOnce(&П) + 'static) {
    let п2 = п.clone();
    crate::agent::позови_за("pkg.upgrade.check", json!({}), 120, move |о| {
        if let Ok(д) = о {
            *п2.обновления.borrow_mut() = д["flathub"].as_array().cloned().unwrap_or_default();
            п2.системных.set(д["list"].as_array().map(|а| а.len()).unwrap_or(0));
        }
        рисуй_бок(&п2);
        дело(&п2);
    });
}

/// Оценки наших подборок пачкой: карточки дорисуют звёзды
fn узнай_оценки(п: &П, ключи: Vec<String>) {
    if ключи.is_empty() {
        return;
    }
    let п2 = Rc::downgrade(п);
    crate::agent::позови_за("store.ratings", json!({ "ids": ключи }), 60, move |о| {
        let Some(п) = п2.upgrade() else { return };
        let Ok(о) = о else { return };
        if let Some(м) = о.as_object() {
            let mut оценки = п.оценки.borrow_mut();
            for (к, в) in м {
                if let Some(о) = оценка_из(в) {
                    оценки.insert(к.clone(), о);
                }
            }
        }
        дорисуй_звёзды(&п);
    });
}

fn дорисуй_звёзды(п: &П) {
    let оценки = п.оценки.borrow().clone();
    п.звёзды.borrow_mut().retain(|(к, з)| {
        let Some(з) = з.upgrade() else { return false };
        match оценки.get(к) {
            Some(о) => {
                з.set_text(&звёзды_текст(*о));
                false
            }
            None => true,
        }
    });
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
        let флатпак = прога.с_flathub();
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
                match зов("pkg.info", json!({ "name": прога.ключ(), "source": "flatpak" }), 60) {
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
                crate::apps::вне_задачи(move || дело(&п2, итог));
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

/* ---------- строка состояния: что система делает прямо сейчас ---------- */

/// Та самая команда, которой агент делает дело, — человеку виднее, что
/// происходит, когда он видит её словами системы
fn команда(действие: &str, имя: &str, источник: &str) -> String {
    let флатпак = источник == "flatpak";
    match (действие, флатпак) {
        ("install", true) => format!("flatpak install flathub {имя}"),
        ("install", false) => format!("sudo apt install {имя}"),
        ("remove", true) => format!("flatpak uninstall {имя}"),
        ("remove", false) => format!("sudo apt remove {имя}"),
        ("update", true) if имя.is_empty() => "flatpak update".into(),
        ("update", true) => format!("flatpak update {имя}"),
        ("update", false) => "sudo apt upgrade".into(),
        ("flathub", _) => "flatpak remote-add flathub".into(),
        _ => имя.to_string(),
    }
}

fn статус(п: &П, текст: &str) {
    п.статус.set_text(текст);
}

/* ---------- история загрузок ---------- */

fn запиши_историю(п: &П, действие: &str, прога_имя: &str, пакет: &str, источник: &str) {
    let mut и = п.история.borrow_mut();
    и.insert(0, json!({ "действие": действие, "имя": прога_имя, "пакет": пакет, "источник": источник, "когда": glib::real_time() / 1_000_000, "итог": "идёт" }));
    и.truncate(40);
    crate::apps::пиши_данные(ИСТОРИЯ, &Value::Array(и.clone()));
}

fn закрой_историю(п: &П, удалось: bool, ошибка: &str) {
    let mut и = п.история.borrow_mut();
    if let Some(з) = и.iter_mut().find(|з| з["итог"] == "идёт") {
        з["итог"] = json!(if удалось { "готово" } else { "не вышло" });
        if !ошибка.is_empty() {
            з["ошибка"] = json!(ошибка);
        }
    }
    crate::apps::пиши_данные(ИСТОРИЯ, &Value::Array(и.clone()));
}

/// Следить за работой агента: ход — внизу окна, по концу — `конец`
fn следи(п: &П, конец: impl FnOnce(&П, Value) + 'static) {
    if п.работа.replace(true) {
        return;
    }
    п.статус_полоса.set_fraction(0.0);
    п.статус_полоса.show();
    let п1 = Rc::downgrade(п);
    let конец: Rc<RefCell<Option<Box<dyn FnOnce(&П, Value)>>>> = Rc::new(RefCell::new(Some(Box::new(конец))));
    let ждём = Rc::new(Cell::new(false));
    let команда_сейчас = п.статус.text().to_string();
    glib::timeout_add_local(Duration::from_millis(1200), move || {
        let Some(п) = п1.upgrade() else { return glib::ControlFlow::Break };
        if !п.работа.get() {
            return glib::ControlFlow::Break;
        }
        // Прошлый вопрос ещё без ответа — не плодим новые
        if ждём.replace(true) {
            return glib::ControlFlow::Continue;
        }
        let (п2, ждём2, конец2, команда2) = (п.clone(), ждём.clone(), конец.clone(), команда_сейчас.clone());
        позови("pkg.job", json!({}), move |о| {
            ждём2.set(false);
            let j = о.unwrap_or(json!({ "running": false, "ok": false, "error": "агент не ответил" }));
            if j["running"].as_bool() == Some(true) {
                let шаг = строка(&j, "step");
                let доля = j["percent"].as_f64().unwrap_or(0.0).clamp(0.0, 100.0);
                статус(&п2, &format!("{команда2} · {}{}", if шаг.is_empty() { "работаю".to_string() } else { шаг }, if доля > 0.0 { format!(" · {}%", доля.round()) } else { String::new() }));
                п2.статус_полоса.set_fraction(доля / 100.0);
                покажи_ход(&п2, &j);
                let имя = п2.в_работе.borrow().clone();
                if !имя.is_empty() {
                    оформи_все_кнопки_для(&п2, &имя, &if доля > 0.0 { format!("{}%", доля.round()) } else { "…".into() });
                }
                return;
            }
            п2.работа.set(false);
            п2.в_работе.borrow_mut().clear();
            *п2.кнопка_хода.borrow_mut() = None;
            п2.статус_полоса.hide();
            let удалось = j["ok"].as_bool() != Some(false);
            закрой_историю(&п2, удалось, &строка(&j, "error"));
            статус(&п2, if удалось { "готово" } else { "не вышло — подробности в окне" });
            if let Some(к) = конец2.borrow_mut().take() {
                к(&п2, j);
            }
            узнай_поставленное(&п2);
        });
        glib::ControlFlow::Continue
    });
}

/* Steam из Flathub без правил для геймпадов ругается при первом запуске.
   Правила — крошечный пакет системы: ставим его сами */
const ДОПОЛНЕНИЯ: &[(&str, &str)] = &[("com.valvesoftware.Steam", "steam-devices")];

/// Установка не началась (отказ, отмена) — вернуть кнопку на место
fn сорвалось(п: &П) {
    п.в_работе.borrow_mut().clear();
    *п.кнопка_хода.borrow_mut() = None;
    обнови_кнопки(п);
    рисуй(п);
}

/// Ход на странице программы: что делается и сколько процентов
fn покажи_ход(п: &П, j: &Value) {
    let Some((подпись, полоса)) = п.кнопка_хода.borrow().clone() else { return };
    let доля = j["percent"].as_f64().unwrap_or(0.0).clamp(0.0, 100.0);
    let шаг = строка(j, "step");
    let шаг = if шаг.is_empty() { crate::i18n::tr("Установка…") } else { шаг };
    подпись.set_text(&if доля > 0.0 { format!("{шаг} · {}%", доля.round()) } else { шаг });
    if доля > 0.0 {
        полоса.set_fraction(доля / 100.0);
    } else {
        полоса.pulse();
    }
}

/// Полоса хода на месте кнопки «Установить»
fn ход_вместо_кнопки(п: &П, место: &gtk::Box, текст: &str) {
    очисти(место);
    let к = gtk::Box::new(gtk::Orientation::Vertical, 6);
    к.style_context().add_class("dlbar");
    let подпись = ярлык(&crate::i18n::tr(текст), "note");
    let полоса = gtk::ProgressBar::new();
    полоса.style_context().add_class("dlbar");
    полоса.set_pulse_step(0.12);
    полоса.set_size_request(220, -1);
    к.add(&подпись);
    к.add(&полоса);
    место.add(&к);
    место.show_all();
    *п.кнопка_хода.borrow_mut() = Some((подпись, полоса.clone()));
    // Пока агент не сказал процентов — полоса «дышит»
    let п1 = Rc::downgrade(п);
    glib::timeout_add_local(Duration::from_millis(250), move || {
        let Some(п) = п1.upgrade() else { return glib::ControlFlow::Break };
        let живо = п.кнопка_хода.borrow().as_ref().is_some_and(|(_, п)| п == &полоса);
        if !живо {
            return glib::ControlFlow::Break;
        }
        if полоса.fraction() == 0.0 || !п.работа.get() {
            полоса.pulse();
        }
        glib::ControlFlow::Continue
    });
}

fn поставь(п: &П, имя: String, источник: String, название: String) {
    *п.в_работе.borrow_mut() = имя.clone();
    let п1 = п.clone();
    узнай_состояние(п, move |п, с| {
        if let Err(почему) = можно_ставить(&с) {
            сорвалось(п);
            return сообщи(п, "Не выйдет", &почему);
        }
        if источник == "flatpak" {
            if с["flatpak"].as_bool() != Some(true) {
                сорвалось(п);
                return сообщи(п, "Не выйдет", "На этой машине нет flatpak, а эта программа живёт только там.");
            }
            if с["flathubData"].as_bool() != Some(true) {
                if !спроси_да(п, "Подключить Flathub?", "Эта программа живёт на Flathub. Чтобы её поставить, система сперва прочитает его списки — это около тридцати мегабайт и одна-две минуты. Дальше всё оттуда ставится сразу.", "Подключить") {
                    return сорвалось(п);
                }
                let (п2, имя2, название2) = (п1.clone(), имя.clone(), название.clone());
                позови("pkg.flathub", json!({}), move |о| {
                    if let Err(е) = о {
                        сорвалось(&п2);
                        return сообщи(&п2, "Flathub не подключился", &е);
                    }
                    статус(&п2, &команда("flathub", "", "flatpak"));
                    следи(&п2, move |п, j| {
                        if j["ok"].as_bool() != Some(true) {
                            сорвалось(п);
                            return сообщи(п, "Flathub не подключился", &строка(&j, "error"));
                        }
                        *п.в_работе.borrow_mut() = имя2.clone();
                        *п.состояние.borrow_mut() = None;
                        п.сведения.borrow_mut().clear();
                        начни_установку(п, имя2, "flatpak".into(), название2);
                    });
                });
                return;
            }
        }
        начни_установку(п, имя, источник, название);
    });
}

fn начни_установку(п: &П, имя: String, источник: String, название: String) {
    let п2 = п.clone();
    *п.в_работе.borrow_mut() = имя.clone();
    позови("pkg.install", json!({ "name": имя, "source": источник }), move |о| {
        if let Err(е) = о {
            сорвалось(&п2);
            return сообщи(&п2, "Не вышло начать установку", &е);
        }
        статус(&п2, &команда("install", &имя, &источник));
        запиши_историю(&п2, "install", &название, &имя, &источник);
        следи(&п2, move |п, j| {
            if j["ok"].as_bool() == Some(false) {
                сообщи(п, "Установка не удалась", &строка(&j, "error"));
            } else {
                тост(п, &format!("{название} установлен"), "Можно запускать из меню программ");
                if let Some((_, доп)) = ДОПОЛНЕНИЯ.iter().find(|(кто, _)| *кто == имя) {
                    позови("pkg.install", json!({ "name": доп, "source": "apt" }), |_| {});
                }
            }
            п.сведения.borrow_mut().clear();
            п.стоят.borrow_mut().insert(имя.clone());
            рисуй(п);
        });
    });
}

/// true — человек согласился, удаление пошло
fn убери(п: &П, имя: String, источник: String, название: &str) -> bool {
    if !спроси_да(п, &format!("Удалить {название}?"), "Программа будет удалена вместе с ненужными зависимостями.", "Удалить") {
        return false;
    }
    *п.в_работе.borrow_mut() = имя.clone();
    let (п2, название2) = (п.clone(), название.to_string());
    позови("pkg.remove", json!({ "name": имя, "source": источник }), move |о| {
        if let Err(е) = о {
            сорвалось(&п2);
            return сообщи(&п2, "Не вышло удалить", &е);
        }
        статус(&п2, &команда("remove", &имя, &источник));
        запиши_историю(&п2, "remove", &название2, &имя, &источник);
        следи(&п2, move |п, j| {
            if j["ok"].as_bool() == Some(false) {
                сообщи(п, "Удалить не удалось", &строка(&j, "error"));
            } else {
                тост(п, &format!("{название2} удалён"), "Место на диске освобождено");
                п.стоят.borrow_mut().remove(&имя);
            }
            п.сведения.borrow_mut().clear();
            рисуй(п);
        });
    });
    true
}

/// Обновить программы Flathub: одну (`ид`) или все разом
fn обнови(п: &П, ид: Option<String>, название: &str) {
    if п.работа.get() {
        return тост(п, "Сначала дождитесь конца текущей работы", "");
    }
    let mut доводы = json!({ "source": "flatpak" });
    if let Some(и) = &ид {
        доводы["ид"] = json!(и);
        *п.в_работе.borrow_mut() = и.clone();
    }
    let (п2, ид2, название2) = (п.clone(), ид.clone().unwrap_or_default(), название.to_string());
    позови("pkg.upgrade.run", доводы, move |о| match о {
        Ok(_) => {
            статус(&п2, &команда("update", &ид2, "flatpak"));
            запиши_историю(&п2, "update", &название2, &ид2, "flatpak");
            следи(&п2, move |п, j| {
                if j["ok"].as_bool() == Some(false) {
                    сообщи(п, "Обновить не удалось", &строка(&j, "error"));
                } else {
                    тост(п, &format!("{название2}: обновлено"), "Новая версия готова");
                }
                узнай_обновления(п, |п| рисуй(п));
            })
        }
        Err(е) => {
            п2.в_работе.borrow_mut().clear();
            сообщи(&п2, "Не вышло начать", &е)
        }
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
    let моё = gtk::Button::with_label("Библиотека");
    моё.style_context().add_class("big");
    let п2 = п.clone();
    моё.connect_clicked(move |_| иди(&п2, Экран::Библиотека));
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
    п.кнопки.borrow_mut().clear();
    п.звёзды.borrow_mut().clear();
    let экран = п.экран.borrow().clone();
    // Своё на машине видно и без сети; остальное — только из сети
    let нужна_сеть = !matches!(экран, Экран::Библиотека | Экран::Желаемое | Экран::Загрузки);
    match п.сеть.get() {
        Some(false) if нужна_сеть => без_сети(п),
        None if нужна_сеть => ждём_сеть(п),
        _ => match &экран {
            Экран::Обзор => обзор(п),
            Экран::Раздел(id) => раздел(п, id),
            Экран::Программа(прога) => программа(п, прога),
            Экран::Поиск(что) => поиск(п, что),
            Экран::Загрузки => загрузки(п),
            Экран::Библиотека => библиотека(п),
            Экран::Желаемое => желаемое(п),
        },
    }
    п.окно.set_title(&match &экран {
        Экран::Программа(прога) => format!("{} — Магазин", прога.имя),
        _ => "Магазин".into(),
    });
    рисуй_бок(п);
    п.тело.show_all();
    п.прокрутка.vadjustment().set_value(0.0);
}

/* ---------- боковая полоса ---------- */

fn пункт(п: &П, значок: &str, имя: &str, значок_число: usize, на_нём: bool, куда: Экран) -> gtk::Button {
    let б = gtk::Button::new();
    б.style_context().add_class("nav");
    if на_нём {
        б.style_context().add_class("on");
    }
    let узко = УЗКО.with(|у| у.get());
    let р = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    if узко {
        р.set_halign(gtk::Align::Center);
        б.set_tooltip_text(Some(&if значок_число > 0 { format!("{} · {значок_число}", crate::i18n::tr(имя)) } else { crate::i18n::tr(имя) }));
    }
    let з: gtk::Widget = if значок.is_ascii() {
        let и = gtk::Image::from_icon_name(Some(значок), gtk::IconSize::Menu);
        и.set_pixel_size(18);
        и.upcast()
    } else {
        crate::apps::значок_эмодзи(значок, 18)
    };
    р.add(&з);
    if !узко {
        let л = gtk::Label::new(Some(имя));
        л.set_xalign(0.0);
        л.set_hexpand(true);
        л.set_ellipsize(gtk::pango::EllipsizeMode::End);
        р.add(&л);
    }
    if значок_число > 0 && !узко {
        let н = gtk::Label::new(Some(&значок_число.to_string()));
        н.style_context().add_class("badge");
        н.set_valign(gtk::Align::Center);
        р.add(&н);
    }
    б.add(&р);
    let п2 = п.clone();
    б.connect_clicked(move |_| {
        п2.поле.set_text("");
        п2.назад.borrow_mut().clear();
        *п2.экран.borrow_mut() = куда.clone();
        рисуй(&п2);
    });
    б
}

fn рисуй_бок(п: &П) {
    очисти(&п.бок);
    let узко = УЗКО.with(|у| у.get());
    if узко { п.бок.style_context().add_class("narrow") } else { п.бок.style_context().remove_class("narrow") }
    if let Some(о) = п.бок.parent().and_then(|в| в.parent()) {
        о.set_size_request(if узко { УЖЕ } else { ШИРЕ }, -1);
    }
    let экран = п.экран.borrow().clone();
    // Знак системы — тот же, что в панели и меню программ
    let бренд = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    бренд.style_context().add_class("brand");
    if let Some(з) = crate::apps::картинка_системы("logo-64.png", 24, 24) {
        бренд.add(&gtk::Image::from_pixbuf(Some(&з)));
    }
    if узко {
        бренд.set_halign(gtk::Align::Center);
    } else {
        бренд.add(&gtk::Label::new(Some("Магазин")));
    }
    п.бок.add(&бренд);

    let обновлений = п.обновления.borrow().len();
    let желаемого = п.желаемое.borrow().len();
    п.бок.add(&пункт(п, "go-home-symbolic", "Главная", 0, matches!(экран, Экран::Обзор), Экран::Обзор));
    п.бок.add(&пункт(п, "view-grid-symbolic", "Библиотека", 0, matches!(экран, Экран::Библиотека), Экран::Библиотека));
    п.бок.add(&пункт(п, "folder-download-symbolic", "Загрузки", обновлений, matches!(экран, Экран::Загрузки), Экран::Загрузки));
    п.бок.add(&пункт(п, "emblem-favorite-symbolic", "Желаемое", желаемого, matches!(экран, Экран::Желаемое), Экран::Желаемое));

    if узко {
        let ч = gtk::Separator::new(gtk::Orientation::Horizontal);
        ч.set_margin_top(8);
        ч.set_margin_bottom(8);
        п.бок.add(&ч);
    } else {
        п.бок.add(&ярлык("Категории", "group"));
    }
    for р in п.каталог["разделы"].as_array().cloned().unwrap_or_default() {
        let id = строка(&р, "id");
        let на_нём = matches!(&экран, Экран::Раздел(а) if *а == id);
        п.бок.add(&пункт(п, &строка(&р, "знак"), &строка(&р, "имя"), 0, на_нём, Экран::Раздел(id)));
    }

    if обновлений > 0 && !узко {
        let плашка = gtk::Box::new(gtk::Orientation::Vertical, 2);
        плашка.style_context().add_class("plate");
        плашка.add(&ярлык(&format!("Обновлений: {обновлений}"), "n"));
        плашка.add(&ярлык("Доступны новые версии программ", ""));
        let б = gtk::Button::with_label("Обновить всё");
        let п2 = п.clone();
        б.connect_clicked(move |_| {
            обнови(&п2, None, "Все программы Flathub");
            *п2.экран.borrow_mut() = Экран::Загрузки;
            рисуй(&п2);
        });
        плашка.add(&б);
        п.бок.add(&плашка);
    }

    // Источник — та же настройка, что в Параметрах → Приложения → Магазин.
    // В узкой полосе ему не место: он есть в Параметрах
    if узко {
        п.бок.show_all();
        return;
    }
    let низ = gtk::Box::new(gtk::Orientation::Vertical, 6);
    низ.set_vexpand(true);
    низ.set_valign(gtk::Align::End);
    низ.add(&ярлык("Источник", "group"));
    let сег = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    сег.set_homogeneous(true);
    сег.style_context().add_class("seg");
    let сейчас = источник();
    for (id, имя) in [("all", "Все"), ("apt", "Ubuntu"), ("flatpak", "Flathub")] {
        let б = gtk::Button::with_label(имя);
        if сейчас == id {
            б.style_context().add_class("on");
        }
        let п2 = п.clone();
        б.connect_clicked(move |_| {
            задай_настройку("storeSource", json!(id));
            // Ядро запишет файл через миг — а нарисовать хочется сразу
            ВЫБРАН.with(|в| *в.borrow_mut() = Some(id.to_string()));
            рисуй(&п2);
        });
        сег.add(&б);
    }
    низ.add(&сег);
    п.бок.add(&низ);
    п.бок.show_all();
}

/* ---------- главная ---------- */

fn слайд(п: &П, в: &Value) -> gtk::Widget {
    let к = gtk::Box::new(gtk::Orientation::Horizontal, 22);
    к.style_context().add_class("hero");
    фон(&к, &строка(в, "фон"));
    let прога = Прога {
        пакет: строка(в, "пакет"),
        имя: строка(в, "имя"),
        про: строка(в, "строка"),
        знак: строка(в, "знак"),
        флатхаб: строка(в, "флатхаб"),
        // Витрина знает программу и в Ubuntu, и на Flathub — ставим оттуда,
        // откуда велит источник в Параметрах
        источник: if источник() == "apt" || строка(в, "флатхаб").is_empty() { String::new() } else { "flatpak".into() },
        ..Default::default()
    };
    знаю(п, &прога);
    к.add(&значок(п, &прога, true));
    let т = gtk::Box::new(gtk::Orientation::Vertical, 6);
    т.set_hexpand(true);
    т.set_valign(gtk::Align::Center);
    let метка = строка(в, "ярлык");
    if !метка.is_empty() {
        т.add(&ярлык(&метка, "tag"));
    }
    т.add(&ярлык(&прога.имя, "name"));
    let л = ярлык(&прога.про, "line");
    л.set_max_width_chars(50);
    т.add(&л);
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    ряд.set_margin_top(8);
    ряд.add(&кнопка_установки(п, &прога));
    let подробнее = gtk::Button::with_label("Подробнее");
    подробнее.style_context().add_class("ghost");
    let (п2, прога2) = (п.clone(), прога.clone());
    подробнее.connect_clicked(move |_| открой_программу(&п2, прога2.clone()));
    ряд.add(&подробнее);
    т.add(&ряд);
    к.add(&т);
    к.upcast()
}

fn витрина(п: &П) {
    let витрина = п.каталог["витрина"].as_array().cloned().unwrap_or_default();
    if витрина.is_empty() {
        return;
    }
    let стопка = gtk::Stack::new();
    стопка.set_transition_type(gtk::StackTransitionType::Crossfade);
    стопка.set_transition_duration(400);
    стопка.set_homogeneous(true);
    for (i, в) in витрина.iter().enumerate() {
        стопка.add_named(&слайд(п, в), &i.to_string());
    }
    п.тело.add(&стопка);
    let точки = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    точки.set_halign(gtk::Align::Center);
    точки.set_margin_top(10);
    let кнопки: Rc<Vec<gtk::Button>> = Rc::new((0..витрина.len()).map(|_| gtk::Button::new()).collect());
    let покажи: Rc<dyn Fn(usize)> = {
        let (стопка, кнопки, п2) = (стопка.clone(), кнопки.clone(), Rc::downgrade(п));
        Rc::new(move |н: usize| {
            if let Some(п) = п2.upgrade() {
                п.витрина.set(н);
            }
            стопка.set_visible_child_name(&н.to_string());
            for (i, к) in кнопки.iter().enumerate() {
                if i == н { к.style_context().add_class("on") } else { к.style_context().remove_class("on") }
            }
        })
    };
    for (i, т) in кнопки.iter().enumerate() {
        т.style_context().add_class("dot");
        let покажи2 = покажи.clone();
        т.connect_clicked(move |_| покажи2(i));
        точки.add(т);
    }
    п.тело.add(&точки);
    let н = п.витрина.get() % витрина.len();
    let (покажи2, всего) = (покажи.clone(), витрина.len());
    glib::idle_add_local_once(move || покажи2(н));
    // Сама листается раз в семь секунд
    let (п2, поколение) = (Rc::downgrade(п), п.поколение.get());
    glib::timeout_add_local(Duration::from_secs(7), move || {
        let Some(п) = п2.upgrade() else { return glib::ControlFlow::Break };
        if п.поколение.get() != поколение {
            return glib::ControlFlow::Break;
        }
        покажи((п.витрина.get() + 1) % всего);
        glib::ControlFlow::Continue
    });
}

/// Заголовок ряда с «Все ›» справа
fn шапка_ряда(п: &П, имя: &str, куда: Option<Экран>) -> gtk::Box {
    let ш = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let з = ярлык(имя, "h");
    з.set_hexpand(true);
    ш.add(&з);
    if let Some(куда) = куда {
        let ещё = gtk::Button::with_label("Все ›");
        ещё.set_relief(gtk::ReliefStyle::None);
        ещё.set_valign(gtk::Align::End);
        let п2 = п.clone();
        ещё.connect_clicked(move |_| иди(&п2, куда.clone()));
        ш.add(&ещё);
    }
    п.тело.add(&ш);
    ш
}

/// Наши подборки — все программы из store-catalog.json, без повторов
fn вся_подборка(п: &П) -> Vec<Прога> {
    let mut все: Vec<Прога> = Vec::new();
    for р in п.каталог["разделы"].as_array().cloned().unwrap_or_default() {
        for з in п.каталог["подборки"][строка(&р, "id")].as_array().cloned().unwrap_or_default() {
            let прога = из_записи(&з);
            if !все.iter().any(|х| х.ключ() == прога.ключ()) {
                все.push(прога);
            }
        }
    }
    все
}

fn обзор(п: &П) {
    витрина(п);
    let с_flathub = источник() != "apt";

    // Топ по загрузкам: популярное на Flathub — по установкам за месяц
    if с_flathub {
        шапка_ряда(п, "Топ по загрузкам", None);
        let (л, ряд) = лента();
        ждём(&ряд);
        п.тело.add(&л);
        let п2 = п.clone();
        let поколение = п.поколение.get();
        подборка_flathub("popular", 1, 24, move |о| {
            if п2.поколение.get() != поколение {
                return;
            }
            очисти(&ряд);
            match о {
                Ok(о) => {
                    let список: Vec<Прога> = о["list"].as_array().cloned().unwrap_or_default().iter().map(из_flathub).filter(видна).take(10).collect();
                    for (i, прога) in список.iter().enumerate() {
                        ряд.add(&карточка_с_местом(&п2, прога, Some(i + 1)));
                    }
                }
                Err(е) => ряд.add(&ярлык(&format!("Flathub не ответил: {е}"), "note")),
            }
            ряд.show_all();
        });
    }

    // Лучшие по оценкам: оценки людей (ODRS) у популярного, но не по
    // голой средней — три пятёрки весят меньше, чем 4,7 из тысячи
    шапка_ряда(п, "Лучшие по оценкам", None);
    let (л, ряд) = лента();
    ждём(&ряд);
    п.тело.add(&л);
    {
        let п2 = п.clone();
        let поколение = п.поколение.get();
        let наши = вся_подборка(п);
        let (куда, откуда) = async_channel::bounded::<Vec<Прога>>(1);
        let язык_ = язык();
        std::thread::spawn(move || {
            let mut все: Vec<Прога> = Vec::new();
            if с_flathub {
                for с in 1..=3 {
                    if let Ok(о) = зов("store.collection", json!({ "вид": "popular", "страница": с, "сколько": 48, "язык": язык_ }), 40) {
                        все.extend(о["list"].as_array().cloned().unwrap_or_default().iter().map(из_flathub));
                    }
                }
            }
            // Пакеты Ubuntu из наших подборок — по их оценкам в ODRS
            let свои: Vec<Прога> = наши.into_iter().filter(|х| !х.с_flathub()).collect();
            if let Ok(о) = зов("store.ratings", json!({ "ids": свои.iter().map(|х| х.ключ()).collect::<Vec<_>>() }), 40) {
                for mut х in свои {
                    х.оценка = оценка_из(&о[х.ключ()]);
                    все.push(х);
                }
            }
            let _ = куда.send_blocking(все);
        });
        glib::MainContext::default().spawn_local(async move {
            let Ok(mut все) = откуда.recv().await else { return };
            if п2.поколение.get() != поколение {
                return;
            }
            все.retain(|х| х.оценка.is_some_and(|(_, н)| н >= 20) && видна(х));
            все.sort_by(|а, б| вес(б.оценка).partial_cmp(&вес(а.оценка)).unwrap_or(std::cmp::Ordering::Equal));
            очисти(&ряд);
            if все.is_empty() {
                ряд.add(&ярлык("Оценок пока нет — ODRS не ответил", "note"));
            }
            for прога in все.iter().take(10) {
                ряд.add(&карточка(&п2, прога));
            }
            ряд.show_all();
        });
    }

    // Наши подборки — каждый раз в новом порядке, с примесью Flathub
    for (id, имя) in [("сеть", "Популярные программы"), ("игры", "Популярные игры"), ("работа", "Для работы"), ("медиа", "Музыка и кино")] {
        шапка_ряда(п, имя, Some(Экран::Раздел(id.into())));
        let (л, ряд) = лента();
        п.тело.add(&л);
        let подборка: Vec<Прога> = п.каталог["подборки"][id].as_array().cloned().unwrap_or_default().iter().map(из_записи).filter(видна).collect();
        узнай_оценки(п, подборка.iter().map(|х| х.ключ()).collect());
        for прога in подборка.iter().take(10) {
            ряд.add(&карточка(п, прога));
        }
        // Рекомендации каждый раз разные: к подборке подмешиваем случайные
        // программы этого раздела с Flathub. Не успел Flathub ответить за
        // три секунды — ряд остаётся как есть, чтобы не прыгал на глазах
        if !с_flathub {
            continue;
        }
        let (п2, ряд2, поколение, начато) = (п.clone(), ряд.clone(), п.поколение.get(), std::time::Instant::now());
        let страница = glib::random_int_range(1, 4) as u32;
        страница_раздела(id, страница, move |о| {
            let Ok(о) = о else { return };
            if п2.поколение.get() != поколение || начато.elapsed().as_secs_f64() > 3.0 {
                return;
            }
            let mut все = подборка.clone();
            for х in о["list"].as_array().cloned().unwrap_or_default() {
                let прога = из_flathub(&х);
                if видна(&прога) && !все.iter().any(|п| п.флатхаб == прога.флатхаб && !п.флатхаб.is_empty()) {
                    все.push(прога);
                }
            }
            перемешай(&mut все);
            очисти(&ряд2);
            for прога in все.iter().take(10) {
                ряд2.add(&карточка(&п2, прога));
            }
            узнай_оценки(&п2, все.iter().take(10).map(|х| х.ключ()).collect());
            ряд2.show_all();
        });
    }

    все_приложения(п);
}

const СОРТИРОВКИ: &[(&str, &str)] = &[
    ("popular", "По популярности"),
    ("trending", "В тренде"),
    ("recently-added", "Новые"),
    ("recently-updated", "Недавно обновлённые"),
    ("rating", "По рейтингу"),
    ("name", "По алфавиту"),
];

/// «Все приложения»: Flathub целиком, страницами, в выбранном порядке. По
/// рейтингу и по алфавиту — то, что уже показано, переставляется на месте
fn все_приложения(п: &П) {
    let ш = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let з = ярлык("Все приложения", "h");
    з.set_hexpand(true);
    ш.add(&з);
    let выбор = gtk::ComboBoxText::new();
    for (id, имя) in СОРТИРОВКИ {
        выбор.append(Some(id), &crate::i18n::tr(имя));
    }
    выбор.set_active_id(Some(&п.сортировка.borrow()));
    выбор.set_valign(gtk::Align::End);
    ш.add(&выбор);
    п.тело.add(&ш);
    let с = сетка();
    п.тело.add(&с);
    let низ = gtk::Box::new(gtk::Orientation::Vertical, 8);
    п.тело.add(&низ);
    let показано: Rc<RefCell<Vec<Прога>>> = Rc::new(RefCell::new(Vec::new()));

    if источник() == "apt" {
        // Только Ubuntu: Flathub не спрашиваем — показываем наши подборки
        let mut свои: Vec<Прога> = вся_подборка(п).into_iter().filter(видна).collect();
        свои.sort_by(|а, б| а.имя.to_lowercase().cmp(&б.имя.to_lowercase()));
        узнай_оценки(п, свои.iter().map(|х| х.ключ()).collect());
        for прога in &свои {
            с.add(&карточка(п, прога));
        }
        выбор.set_sensitive(false);
        return;
    }
    let (п2, с2, низ2, показано2) = (п.clone(), с.clone(), низ.clone(), показано.clone());
    выбор.connect_changed(move |в| {
        let Some(id) = в.active_id() else { return };
        *п2.сортировка.borrow_mut() = id.to_string();
        match id.as_str() {
            "rating" | "name" => {
                let mut все = показано2.borrow().clone();
                if id == "rating" {
                    все.sort_by(|а, б| вес(б.оценка).partial_cmp(&вес(а.оценка)).unwrap_or(std::cmp::Ordering::Equal));
                } else {
                    все.sort_by(|а, б| а.имя.to_lowercase().cmp(&б.имя.to_lowercase()));
                }
                очисти(&с2);
                for прога in &все {
                    с2.add(&карточка(&п2, прога));
                }
                с2.show_all();
            }
            вид => {
                очисти(&с2);
                показано2.borrow_mut().clear();
                догрузи_всё(&п2, вид.to_string(), 1, показано2.clone(), с2.clone(), низ2.clone());
            }
        }
    });
    let вид = п.сортировка.borrow().clone();
    let вид = if matches!(вид.as_str(), "rating" | "name") { "popular".to_string() } else { вид };
    догрузи_всё(п, вид, 1, показано, с, низ);
}

fn догрузи_всё(п: &П, вид: String, страница: u32, показано: Rc<RefCell<Vec<Прога>>>, с: gtk::FlowBox, низ: gtk::Box) {
    очисти(&низ);
    ждём(&низ);
    низ.show_all();
    let (п2, поколение) = (п.clone(), п.поколение.get());
    подборка_flathub(&вид.clone(), страница, 24, move |о| {
        if п2.поколение.get() != поколение {
            return;
        }
        очисти(&низ);
        let о = match о {
            Ok(о) => о,
            Err(е) => {
                заметка(&низ, &е);
                низ.show_all();
                return;
            }
        };
        for х in о["list"].as_array().cloned().unwrap_or_default() {
            let прога = из_flathub(&х);
            if !видна(&прога) || показано.borrow().iter().any(|п| п.ключ() == прога.ключ()) {
                continue;
            }
            показано.borrow_mut().push(прога.clone());
            с.add(&карточка(&п2, &прога));
        }
        с.show_all();
        if (страница as u64) < о["страниц"].as_u64().unwrap_or(1) {
            let б = gtk::Button::with_label("Показать ещё");
            б.set_halign(gtk::Align::Center);
            let (п3, вид3, показано3, с3, низ3) = (п2.clone(), вид.clone(), показано.clone(), с.clone(), низ.clone());
            б.connect_clicked(move |_| догрузи_всё(&п3, вид3.clone(), страница + 1, показано3.clone(), с3.clone(), низ3.clone()));
            низ.add(&б);
        }
        низ.show_all();
    });
}

fn кнопка_назад(п: &П) {
    let б = gtk::Button::with_label("‹ Назад");
    б.style_context().add_class("ghost");
    б.set_halign(gtk::Align::Start);
    б.set_margin_bottom(14);
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
    let з = заголовок(п, &format!("{} {}", строка(&р, "знак"), строка(&р, "имя")));
    з.style_context().add_class("first");
    let с = сетка();
    let mut уже: Vec<String> = Vec::new();
    let подборка: Vec<Прога> = п.каталог["подборки"][id].as_array().cloned().unwrap_or_default().iter().map(из_записи).filter(видна).collect();
    узнай_оценки(п, подборка.iter().map(|х| х.ключ()).collect());
    for прога in &подборка {
        if !прога.флатхаб.is_empty() {
            уже.push(прога.флатхаб.clone());
        }
        с.add(&карточка(п, прога));
    }
    п.тело.add(&с);
    if источник() == "apt" {
        return;
    }
    // А за подборкой — всё, что есть в этом разделе на Flathub, страницами
    let ещё_з = ярлык("Ещё в разделе", "h");
    п.тело.add(&ещё_з);
    let с2 = сетка();
    п.тело.add(&с2);
    let низ = gtk::Box::new(gtk::Orientation::Vertical, 8);
    п.тело.add(&низ);
    догрузи(п, id.to_string(), 1, Rc::new(RefCell::new(уже)), с2, низ);
}

/// Следующая страница раздела с Flathub — и кнопка «Показать ещё», пока есть что
fn догрузи(п: &П, id: String, страница: u32, уже: Rc<RefCell<Vec<String>>>, с: gtk::FlowBox, низ: gtk::Box) {
    очисти(&низ);
    ждём(&низ);
    низ.show_all();
    let (п2, поколение) = (п.clone(), п.поколение.get());
    страница_раздела(&id.clone(), страница, move |о| {
        if п2.поколение.get() != поколение {
            return;
        }
        очисти(&низ);
        let о = match о {
            Ok(о) => о,
            Err(е) => {
                заметка(&низ, &е);
                низ.show_all();
                return;
            }
        };
        let mut новые = Vec::new();
        for х in о["list"].as_array().cloned().unwrap_or_default() {
            let прога = из_flathub(&х);
            if уже.borrow().contains(&прога.флатхаб) || !видна(&прога) {
                continue;
            }
            уже.borrow_mut().push(прога.флатхаб.clone());
            новые.push(прога.ключ());
            с.add(&карточка(&п2, &прога));
        }
        узнай_оценки(&п2, новые);
        с.show_all();
        if (страница as u64) < о["страниц"].as_u64().unwrap_or(1) {
            let б = gtk::Button::with_label("Показать ещё");
            б.set_halign(gtk::Align::Center);
            let (п3, id3, уже3, с3, низ3) = (п2.clone(), id.clone(), уже.clone(), с.clone(), низ.clone());
            б.connect_clicked(move |_| догрузи(&п3, id3.clone(), страница + 1, уже3.clone(), с3.clone(), низ3.clone()));
            низ.add(&б);
        }
        низ.show_all();
    });
}

/* ---------- страница программы ---------- */

/// Раздел Магазина по категориям Flathub — для «Похожих»
fn раздел_по_категориям(категории: &[Value]) -> Option<&'static str> {
    for к in категории.iter().filter_map(|к| к.as_str()) {
        let р = match к {
            "Game" => "игры",
            "Network" | "Chat" | "WebBrowser" | "Email" => "сеть",
            "Office" => "работа",
            "AudioVideo" | "Audio" | "Video" | "Player" => "медиа",
            "Graphics" => "графика",
            "Development" => "разработка",
            "System" | "Utility" => "система",
            _ => continue,
        };
        return Some(р);
    }
    None
}

fn в_желаемом(п: &П, прога: &Прога) -> bool {
    п.желаемое.borrow().iter().any(|х| х.ключ() == прога.ключ())
}

fn переключи_желаемое(п: &П, прога: &Прога) -> bool {
    let есть = в_желаемом(п, прога);
    {
        let mut ж = п.желаемое.borrow_mut();
        if есть {
            ж.retain(|х| х.ключ() != прога.ключ());
        } else {
            ж.insert(0, прога.clone());
        }
        crate::apps::пиши_данные(ЖЕЛАЕМОЕ, &Value::Array(ж.iter().map(|х| х.в_json()).collect()));
    }
    рисуй_бок(п);
    !есть
}

fn факт(факты: &gtk::Box, большое: &str, подпись: &str) {
    if большое.is_empty() {
        return;
    }
    let ф = gtk::Box::new(gtk::Orientation::Vertical, 2);
    ф.set_hexpand(true);
    let б = ярлык(большое, "b");
    б.set_xalign(0.5);
    let к = ярлык(подпись, "k");
    к.set_xalign(0.5);
    ф.add(&б);
    ф.add(&к);
    факты.add(&ф);
    ф.show_all();
}

fn строка_сведений(сетка: &gtk::Grid, ряд: &Cell<i32>, имя: &str, что: &str) {
    if что.is_empty() {
        return;
    }
    let к = ярлык(имя, "k");
    let з = ярлык(что, "");
    з.set_selectable(true);
    з.set_hexpand(true);
    сетка.attach(&к, 0, ряд.get(), 1, 1);
    сетка.attach(&з, 1, ряд.get(), 1, 1);
    ряд.set(ряд.get() + 1);
    сетка.show_all();
}

fn программа(п: &П, прога: &Прога) {
    знаю(п, прога);
    кнопка_назад(п);
    let шапка = gtk::Box::new(gtk::Orientation::Horizontal, 20);
    шапка.add(&значок(п, прога, true));
    let т = gtk::Box::new(gtk::Orientation::Vertical, 4);
    т.set_hexpand(true);
    т.set_valign(gtk::Align::Center);
    let и = ярлык(&прога.имя, "h first");
    т.add(&и);
    let кто = ярлык(&прога.про, "note");
    т.add(&кто);
    let действия = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    действия.set_margin_top(10);
    let место = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    место.set_valign(gtk::Align::Center);
    действия.add(&место);
    // Желаемое — помнится на диске, видно в боковой полосе
    let сердце = gtk::Button::with_label(if в_желаемом(п, прога) { "♥ В желаемом" } else { "♡ В желаемое" });
    сердце.style_context().add_class("ghost");
    if в_желаемом(п, прога) {
        сердце.style_context().add_class("on");
    }
    let (п2, прога2) = (п.clone(), прога.clone());
    сердце.connect_clicked(move |б| {
        let стало = переключи_желаемое(&п2, &прога2);
        б.set_label(&crate::i18n::tr(if стало { "♥ В желаемом" } else { "♡ В желаемое" }));
        if стало { б.style_context().add_class("on") } else { б.style_context().remove_class("on") }
        тост(&п2, if стало { "Добавлено в желаемое" } else { "Убрано из желаемого" }, &прога2.имя);
    });
    действия.add(&сердце);
    // Поделиться: страница на Flathub или команда для терминала
    let поделиться = gtk::Button::with_label("Поделиться");
    поделиться.style_context().add_class("ghost");
    let (п2, прога2) = (п.clone(), прога.clone());
    поделиться.connect_clicked(move |б| {
        let что = if прога2.флатхаб.is_empty() { format!("sudo apt install {}", прога2.пакет) } else { format!("https://flathub.org/apps/{}", прога2.флатхаб) };
        б.clipboard(&gtk::gdk::SELECTION_CLIPBOARD).set_text(&что);
        тост(&п2, if прога2.флатхаб.is_empty() { "Команда скопирована" } else { "Ссылка скопирована" }, &что);
    });
    действия.add(&поделиться);
    т.add(&действия);
    шапка.add(&т);
    п.тело.add(&шапка);

    let факты = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    факты.style_context().add_class("facts");
    факты.set_homogeneous(true);
    факты.set_margin_top(20);
    факты.set_margin_bottom(6);
    п.тело.add(&факты);
    let снимки_з = ярлык("Скриншоты", "h");
    п.тело.add(&снимки_з);
    let (лента_снимков, снимки) = лента();
    п.тело.add(&лента_снимков);
    let колонки = gtk::Box::new(gtk::Orientation::Horizontal, 24);
    колонки.set_homogeneous(false);
    let левая = gtk::Box::new(gtk::Orientation::Vertical, 8);
    левая.set_hexpand(true);
    let правая = gtk::Box::new(gtk::Orientation::Vertical, 8);
    правая.set_size_request(320, -1);
    // Иначе растягивающиеся строки сведений отнимают ширину у описания
    правая.set_hexpand(false);
    колонки.add(&левая);
    колонки.add(&правая);
    п.тело.add(&колонки);
    let похожие_место = gtk::Box::new(gtk::Orientation::Vertical, 0);
    п.тело.add(&похожие_место);

    левая.add(&ярлык("Описание", "h"));
    let описание = ярлык(&прога.про, "long");
    описание.set_max_width_chars(70);
    левая.add(&описание);
    let нового = gtk::Box::new(gtk::Orientation::Vertical, 6);
    левая.add(&нового);
    левая.add(&ярлык("Отзывы", "h"));
    let отзывы = gtk::Box::new(gtk::Orientation::Vertical, 10);
    ждём(&отзывы);
    левая.add(&отзывы);

    правая.add(&ярлык("Сведения", "h"));
    let инфо = gtk::Grid::new();
    инфо.style_context().add_class("info");
    инфо.set_row_spacing(6);
    инфо.set_column_spacing(16);
    правая.add(&инфо);
    let ряд_инфо = Rc::new(Cell::new(0));
    правая.add(&ярлык("Оценки", "h"));
    let полосы = gtk::Box::new(gtk::Orientation::Vertical, 4);
    полосы.style_context().add_class("bars");
    правая.add(&полосы);

    ждём(&место);
    let поколение = п.поколение.get();

    // 1. Машина: стоит ли, версия, размер, кнопки
    {
        let (место2, факты2, инфо2, ряд2, прога2) = (место.clone(), факты.clone(), инфо.clone(), ряд_инфо.clone(), прога.clone());
        сведения(п, прога, false, move |п, о| {
            if п.поколение.get() != поколение {
                return;
            }
            очисти(&место2);
            if о["нет"].as_bool() == Some(true) {
                let н = ярлык(&строка(&о, "почему"), "note");
                место2.add(&н);
                if строка(&о, "почему").contains("такого нет") {
                    место2.add(&ярлык("Возможно, у программы другое имя в этом выпуске Ubuntu — попробуйте поиск сверху.", "note"));
                }
                место2.show_all();
                return;
            }
            let стоит = стоит(&о["installed"]);
            let имя = { let и = строка(&о, "name"); if и.is_empty() { прога2.ключ() } else { и } };
            let источник = строка(&о, "source");
            if стоит {
                п.стоят.borrow_mut().insert(имя.clone());
            }
            // Как Microsoft Store: «Установить» → ход на месте кнопки → «Открыть»
            let эта_в_работе = п.работа.get() && *п.в_работе.borrow() == имя;
            if эта_в_работе {
                ход_вместо_кнопки(п, &место2, if стоит { "Удаление…" } else { "Установка…" });
            } else if стоит {
                // Открыть поставленное — тем, чем его открывает система
                let п3 = п.clone();
                let пакет = имя.clone();
                let открыть = gtk::Button::with_label("Открыть");
                открыть.style_context().add_class("get");
                открыть.connect_clicked(move |_| запусти_пакет(&п3, &пакет));
                место2.add(&открыть);
                let удалить = gtk::Button::with_label("Удалить");
                удалить.style_context().add_class("ghost");
                удалить.set_sensitive(!п.работа.get());
                let (п2, имя2, источник2, название) = (п.clone(), имя.clone(), источник.clone(), прога2.имя.clone());
                let место3 = место2.clone();
                удалить.connect_clicked(move |_| {
                    if убери(&п2, имя2.clone(), источник2.clone(), &название) {
                        ход_вместо_кнопки(&п2, &место3, "Удаление…");
                    }
                });
                место2.add(&удалить);
            } else {
                let кн = gtk::Button::with_label("Установить");
                кн.style_context().add_class("get");
                let заглушка = о["snap"].as_bool() == Some(true);
                if заглушка {
                    кн.set_label(&crate::i18n::tr("Недоступно"));
                }
                // Другая программа ещё ставится — эта ждёт своей очереди
                кн.set_sensitive(!заглушка && !п.работа.get());
                if п.работа.get() {
                    кн.set_tooltip_text(Some(&crate::i18n::tr("Сначала дождитесь конца текущей установки")));
                }
                let (п2, имя2, источник2, название) = (п.clone(), имя.clone(), источник.clone(), прога2.имя.clone());
                let место3 = место2.clone();
                кн.connect_clicked(move |_| {
                    ход_вместо_кнопки(&п2, &место3, "Подготовка…");
                    поставь(&п2, имя2.clone(), источник2.clone(), название.clone());
                });
                место2.add(&кн);
            }
            let р = размер(о["size"].as_u64().unwrap_or(0));
            факт(&факты2, &р, if источник == "flatpak" { "Скачает" } else { "Займёт" });
            let версия = [&о["installed"], &о["candidate"]].into_iter().filter_map(|в| в.as_str()).find(|в| !в.is_empty()).unwrap_or_default().to_string();
            строка_сведений(&инфо2, &ряд2, "Источник", if источник == "flatpak" { "Flathub" } else { "Ubuntu" });
            строка_сведений(&инфо2, &ряд2, "Пакет", &имя);
            строка_сведений(&инфо2, &ряд2, "Версия", &версия);
            if о["snap"].as_bool() == Some(true) {
                место2.add(&ярлык("Это заглушка: программа ставится через Snap, а он в системе не работает.", "note"));
            }
            if о["flathubНеОтвечает"].as_bool() == Some(true) {
                место2.add(&ярлык("Flathub сейчас отвечает с ошибкой — сведения взяты из его каталога. Установить можно: если основной адрес Flathub не ответит, система поставит программу через зеркало.", "note"));
            }
            место2.show_all();
        });
    }

    // 2. Flathub: описание, скриншоты, что нового, разработчик, возраст, загрузки
    if !прога.флатхаб.is_empty() {
        let (кто2, описание2, нового2, факты2, инфо2, ряд2, снимки2, снимки_з2, лента2, похожие2, полосы2, п2) = (
            кто.clone(), описание.clone(), нового.clone(), факты.clone(), инфо.clone(), ряд_инфо.clone(), снимки.clone(), снимки_з.clone(), лента_снимков.clone(), похожие_место.clone(), полосы.clone(), п.clone(),
        );
        let ид = прога.флатхаб.clone();
        спроси(п, "store.details", json!({ "ид": прога.флатхаб, "язык": язык() }), move |о| {
            let Ok(д) = о else {
                снимки_з2.hide();
                лента2.hide();
                return;
            };
            if д["нет"].as_bool() == Some(true) {
                снимки_з2.hide();
                лента2.hide();
                return;
            }
            let разработчик = строка(&д, "разработчик");
            let раздел = раздел_по_категориям(д["категории"].as_array().map(|а| а.as_slice()).unwrap_or(&[]));
            let имя_раздела = раздел.and_then(|р| п2.каталог["разделы"].as_array()?.iter().find(|х| строка(х, "id") == р).map(|х| строка(х, "имя"))).unwrap_or_default();
            кто2.set_text(&[разработчик.clone(), имя_раздела.clone()].into_iter().filter(|с| !с.is_empty()).collect::<Vec<_>>().join(", "));
            let текст = строка(&д, "описание");
            if !текст.is_empty() {
                описание2.set_text(&текст);
            }
            if let Some(о) = оценка_из(&д["оценка"]) {
                факт(&факты2, &format!("{:.1} ★", о.0).replace('.', ","), &format!("{} оценок", сколько(о.1)));
                полосы_оценок(&полосы2, &д["оценка"]);
            }
            if let Some(в) = д["возраст"].as_u64() {
                факт(&факты2, &format!("{в}+"), "Возраст");
            }
            if let Some(з) = д["загрузок"].as_u64().filter(|з| *з > 0) {
                факт(&факты2, &сколько(з), "Загрузок");
            }
            if let Some(м) = д["за_месяц"].as_u64().filter(|м| *м > 0) {
                факт(&факты2, &сколько(м), "За месяц");
            }
            строка_сведений(&инфо2, &ряд2, "Разработчик", &разработчик);
            строка_сведений(&инфо2, &ряд2, "Лицензия", &{
                let л = строка(&д, "лицензия");
                if д["свободная"].as_bool() == Some(false) { format!("{л} (закрытая)") } else { л }
            });
            строка_сведений(&инфо2, &ряд2, "Категория", &имя_раздела);
            if д["проверено"].as_bool() == Some(true) {
                строка_сведений(&инфо2, &ряд2, "Проверено", "✓ разработчик подтвердил на Flathub");
            }
            let сайт = строка(&д, "сайт");
            if !сайт.is_empty() {
                let ссылка = gtk::LinkButton::with_label(&сайт, &сайт.trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/'));
                ссылка.set_halign(gtk::Align::Start);
                инфо2.attach(&ярлык("Сайт", "k"), 0, ряд2.get(), 1, 1);
                инфо2.attach(&ссылка, 1, ряд2.get(), 1, 1);
                ряд2.set(ряд2.get() + 1);
                инфо2.show_all();
            }
            // Что нового в последнем выпуске
            let в = &д["выпуск"];
            if в.is_object() {
                нового2.add(&ярлык("Что нового", "h"));
                let когда = в["когда"].as_i64().and_then(|т| glib::DateTime::from_unix_local(т).ok()).and_then(|д| д.format("%d.%m.%Y").ok()).map(|с| format!(" · {с}")).unwrap_or_default();
                нового2.add(&ярлык(&format!("Версия {}{когда}", строка(в, "версия")), "n"));
                let что = строка(в, "что");
                if !что.is_empty() {
                    let л = ярлык(&что, "long");
                    л.set_max_width_chars(70);
                    нового2.add(&л);
                }
                нового2.show_all();
            }
            // Скриншоты — по одному, в своей очереди
            let список = д["скриншоты"].as_array().cloned().unwrap_or_default();
            if список.is_empty() {
                снимки_з2.hide();
                лента2.hide();
            }
            for с in список {
                скриншот(&п2, &снимки2, строка(&с, "адрес"), строка(&с, "подпись"));
            }
            if let Some(р) = раздел {
                похожие(&п2, &похожие2, р, &ид);
            }
        });
    } else {
        снимки_з.hide();
        лента_снимков.set_no_show_all(true);
        снимки_з.set_no_show_all(true);
        // Пакет Ubuntu: оценки ODRS по имени пакета
        let (факты2, полосы2) = (факты.clone(), полосы.clone());
        let ключ = прога.пакет.clone();
        спроси(п, "store.ratings", json!({ "ids": [прога.пакет] }), move |о| {
            let Ok(о) = о else { return };
            if let Some(з) = оценка_из(&о[&ключ]) {
                факт(&факты2, &format!("{:.1} ★", з.0).replace('.', ","), &format!("{} оценок", сколько(з.1)));
                полосы_оценок(&полосы2, &о[&ключ]);
            }
        });
    }

    // 3. Отзывы людей (ODRS)
    {
        let (отзывы2, полосы2) = (отзывы.clone(), полосы.clone());
        спроси(п, "store.reviews", json!({ "ид": прога.ключ(), "язык": язык(), "сколько": 6 }), move |о| {
            очисти(&отзывы2);
            let список = о.ok().and_then(|о| о["list"].as_array().cloned()).unwrap_or_default();
            if список.is_empty() {
                отзывы2.add(&ярлык("Отзывов пока нет.", "note"));
            }
            for о in список {
                let к = gtk::Box::new(gtk::Orientation::Vertical, 4);
                к.style_context().add_class("rev");
                let верх = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                верх.add(&ярлык(&строка(&о, "кто"), "n"));
                let н = о["оценка"].as_u64().unwrap_or(0).min(5) as usize;
                верх.add(&ярлык(&format!("{}{}", "★".repeat(н), "☆".repeat(5 - н)), "stars"));
                к.add(&верх);
                let з = строка(&о, "заголовок");
                if !з.is_empty() {
                    к.add(&ярлык(&з, "n"));
                }
                let т = ярлык(&строка(&о, "текст"), "long");
                т.set_max_width_chars(70);
                к.add(&т);
                отзывы2.add(&к);
            }
            отзывы2.show_all();
            if полосы2.children().is_empty() {
                полосы2.add(&ярлык("Оценок пока нет.", "note"));
                полосы2.show_all();
            }
        });
    }
}

fn полосы_оценок(место: &gtk::Box, о: &Value) {
    очисти(место);
    let звёзды: Vec<u64> = о["звёзды"].as_array().cloned().unwrap_or_default().iter().map(|з| з.as_u64().unwrap_or(0)).collect();
    let всего: u64 = звёзды.iter().sum::<u64>().max(1);
    for н in (1..=5).rev() {
        let р = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let ч = ярлык(&н.to_string(), "k");
        ч.set_width_chars(1);
        р.add(&ч);
        let пб = gtk::ProgressBar::new();
        пб.set_hexpand(true);
        пб.set_valign(gtk::Align::Center);
        пб.set_fraction(*звёзды.get(н - 1).unwrap_or(&0) as f64 / всего as f64);
        р.add(&пб);
        место.add(&р);
    }
    место.show_all();
}

fn скриншот(п: &П, ряд: &gtk::Box, адрес: String, подпись: String) {
    let б = gtk::Button::new();
    б.style_context().add_class("shot");
    б.set_relief(gtk::ReliefStyle::None);
    let ж = gtk::Spinner::new();
    ж.start();
    ж.set_size_request(320, 180);
    б.add(&ж);
    if !подпись.is_empty() {
        б.set_tooltip_text(Some(&подпись));
    }
    ряд.add(&б);
    ряд.show_all();
    let (б2, п2) = (б.clone(), Rc::downgrade(п));
    crate::agent::позови_за("store.shot", json!({ "адрес": адрес }), 40, move |о| {
        let Some(к) = о.ok().filter(|о| о["есть"].as_bool() == Some(true)).and_then(|о| о["данные"].as_str().and_then(картинка_как_есть)) else {
            unsafe { б2.destroy() };
            return;
        };
        let высота = 200;
        let ширина = (к.width() as f64 * высота as f64 / к.height().max(1) as f64) as i32;
        let мал = к.scale_simple(ширина, высота, InterpType::Bilinear).unwrap_or_else(|| к.clone());
        очисти(&б2);
        б2.add(&gtk::Image::from_pixbuf(Some(&мал)));
        б2.show_all();
        // Щелчок — во всю ширину окна
        б2.connect_clicked(move |_| {
            let Some(п) = п2.upgrade() else { return };
            let д = gtk::Dialog::new();
            д.set_transient_for(Some(&п.окно));
            д.set_modal(true);
            д.set_title(&подпись);
            let w = (п.окно.allocated_width() - 80).clamp(400, к.width().max(400));
            let h = (к.height() as f64 * w as f64 / к.width().max(1) as f64) as i32;
            if let Some(б) = к.scale_simple(w, h, InterpType::Bilinear) {
                д.content_area().add(&gtk::Image::from_pixbuf(Some(&б)));
            }
            д.show_all();
            д.run();
            unsafe { д.destroy() };
        });
    });
}

fn похожие(п: &П, место: &gtk::Box, раздел: &str, кроме: &str) {
    let кроме = кроме.to_string();
    let (п2, место2) = (п.clone(), место.clone());
    let поколение = п.поколение.get();
    страница_раздела(раздел, 1, move |о| {
        if п2.поколение.get() != поколение {
            return;
        }
        let Ok(о) = о else { return };
        let mut список: Vec<Прога> = о["list"].as_array().cloned().unwrap_or_default().iter().map(из_flathub).filter(|х| х.флатхаб != кроме && видна(х)).collect();
        перемешай(&mut список);
        if список.is_empty() {
            return;
        }
        место2.add(&ярлык("Похожие приложения", "h"));
        let (л, ряд) = лента();
        for прога in список.iter().take(8) {
            ряд.add(&карточка(&п2, прога));
        }
        узнай_оценки(&п2, список.iter().take(8).map(|х| х.ключ()).collect());
        место2.add(&л);
        место2.show_all();
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
            } else {
                тост(п, &format!("Запускаю {}", а.display_name()), "");
            }
        }
        None => сообщи(п, "Ярлыка не нашлось", "Программа стоит, но в меню её нет — у неё может не быть окна."),
    }
}

/* ---------- поиск ---------- */

/// Подсказки под полем поиска — сразу, из того, что Магазин уже знает
fn подскажи(п: &П, что: &str) {
    очисти(&п.подсказки_список);
    let что = что.trim().to_lowercase();
    if что.chars().count() < 2 {
        п.подсказки.popdown();
        return;
    }
    let найдено: Vec<Прога> = п.известные.borrow().iter().filter(|х| видна(х) && (х.имя.to_lowercase().contains(&что) || х.ключ().to_lowercase().contains(&что))).take(6).cloned().collect();
    if найдено.is_empty() {
        п.подсказки.popdown();
        return;
    }
    for прога in найдено {
        let б = gtk::Button::new();
        let р = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let и = gtk::Label::new(Some(&прога.имя));
        и.set_xalign(0.0);
        и.set_hexpand(true);
        р.add(&и);
        let откуда = ярлык(if прога.с_flathub() { "Flathub" } else { "Ubuntu" }, "a");
        р.add(&откуда);
        б.add(&р);
        let п2 = п.clone();
        б.connect_clicked(move |_| {
            п2.подсказки.popdown();
            п2.поле.set_text("");
            открой_программу(&п2, прога.clone());
        });
        п.подсказки_список.add(&б);
    }
    п.подсказки_список.show_all();
    п.подсказки.popup();
}

fn поиск(п: &П, что: &str) {
    let з = заголовок(п, &format!("Найдено по запросу «{что}»"));
    з.style_context().add_class("first");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 8);
    п.тело.add(&место);
    if что.chars().count() < 2 {
        заметка(&место, "Хотя бы две буквы");
        return;
    }
    ждём(&место);
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
            ждём(&место2);
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
                .map(|р| р["list"].as_array().cloned().unwrap_or_default().into_iter().take(40).collect::<Vec<Value>>());
            let _ = куда.send_blocking(итог);
        });
        let п2 = п.clone();
        glib::MainContext::default().spawn_local(async move {
            while let Ok(итог) = откуда.recv().await {
                if !списки {
                    *п2.состояние.borrow_mut() = None;
                }
                if п2.поколение.get() != поколение {
                    return;
                }
                очисти(&место2);
                let итог = итог.map(|с| {
                    с.into_iter()
                        .map(|х| {
                            let имя = строка(&х, "name");
                            п2.сведения.borrow_mut().insert(имя.clone(), х.clone());
                            let флатпак = строка(&х, "source") == "flatpak";
                            if стоит(&х["installed"]) {
                                п2.стоят.borrow_mut().insert(имя.clone());
                            }
                            let заголовок = строка(&х, "title");
                            Прога {
                                пакет: имя.clone(),
                                имя: if заголовок.is_empty() || заголовок == имя { имя.clone() } else { заголовок },
                                про: [строка(&х, "about"), if флатпак { "Flathub".into() } else { "Ubuntu".into() }, размер(х["size"].as_u64().unwrap_or(0))].into_iter().filter(|с| !с.is_empty()).collect::<Vec<_>>().join(" · "),
                                знак: if флатпак { "🫙".into() } else { "📦".into() },
                                флатхаб: if флатпак { имя.clone() } else { String::new() },
                                источник: if флатпак { "flatpak".into() } else { String::new() },
                                ..Default::default()
                            }
                        })
                        .filter(видна)
                        .take(24)
                        .collect::<Vec<Прога>>()
                });
                match итог {
                    Err(е) => заметка(&место2, &е),
                    Ok(список) if список.is_empty() => пусто(&место2, "Ничего не нашлось. Проверьте написание — у программ бывают свои имена"),
                    Ok(список) => {
                        let с = сетка();
                        узнай_оценки(&п2, список.iter().map(|х| х.ключ()).collect());
                        for прога in &список {
                            с.add(&карточка(&п2, прога));
                        }
                        место2.add(&с);
                    }
                }
                место2.show_all();
            }
        });
    });
}

/* ---------- загрузки и обновления ---------- */

fn загрузки(п: &П) {
    let ш = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let з = ярлык("Загрузки и обновления", "h first");
    з.set_hexpand(true);
    ш.add(&з);
    let все = gtk::Button::with_label("Получить обновления");
    все.style_context().add_class("get");
    все.set_valign(gtk::Align::Center);
    все.set_no_show_all(true);
    let п2 = п.clone();
    все.connect_clicked(move |_| обнови(&п2, None, "Все программы Flathub"));
    ш.add(&все);
    п.тело.add(&ш);

    п.тело.add(&ярлык("Доступные обновления", "h"));
    let место = gtk::Box::new(gtk::Orientation::Vertical, 10);
    п.тело.add(&место);
    заметка(&место, "Спрашиваю систему, что нового…");
    ждём(&место);
    let (место2, все2) = (место.clone(), все.clone());
    let поколение = п.поколение.get();
    узнай_обновления(п, move |п| {
        if п.поколение.get() != поколение {
            return;
        }
        очисти(&место2);
        // Система обновляется при перезагрузке — здесь о ней одна строка
        let системных = п.системных.get();
        if системных > 0 {
            let р = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            р.style_context().add_class("row");
            let т = gtk::Box::new(gtk::Orientation::Vertical, 2);
            т.set_hexpand(true);
            т.add(&ярлык(&format!("✨ Обновления системы: {системных}"), "n"));
            т.add(&ярлык("Ставятся при перезагрузке — меню питания → «Обновить и перезагрузить»", "a"));
            р.add(&т);
            let б = gtk::Button::with_label("Открыть");
            б.style_context().add_class("ghost");
            б.set_valign(gtk::Align::Center);
            б.connect_clicked(|_| {
                if std::process::Command::new("glower-settings").arg("update").spawn().is_err() {
                    скажи("параметры-раздел", json!("update"));
                }
            });
            р.add(&б);
            место2.add(&р);
        }
        let флатхаб = п.обновления.borrow().clone();
        if флатхаб.is_empty() && системных == 0 {
            пусто(&место2, "Всё свежее. Программы обновлены.");
        }
        все2.set_visible(!флатхаб.is_empty());
        все2.set_sensitive(!п.работа.get());
        for х in флатхаб {
            let имя = { let и = строка(&х, "имя"); if и.is_empty() { строка(&х, "name") } else { и } };
            let ид = строка(&х, "name");
            let станет = строка(&х, "станет");
            let прога = Прога { пакет: ид.clone(), имя: имя.clone(), про: String::new(), знак: "🫙".into(), флатхаб: ид.clone(), источник: "flatpak".into(), ..Default::default() };
            let р = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            р.style_context().add_class("row");
            р.add(&значок(п, &прога, false));
            let т = gtk::Box::new(gtk::Orientation::Vertical, 2);
            т.set_hexpand(true);
            т.set_valign(gtk::Align::Center);
            т.add(&ярлык(&имя, "n"));
            т.add(&ярлык(&if станет.is_empty() { ид.clone() } else { format!("Версия {станет}") }, "a"));
            р.add(&т);
            let б = gtk::Button::with_label("Обновить");
            б.style_context().add_class("get");
            б.set_valign(gtk::Align::Center);
            б.set_sensitive(!п.работа.get());
            п.кнопки.borrow_mut().push((ид.clone(), б.downgrade()));
            let (п3, ид3, имя3) = (п.clone(), ид.clone(), имя.clone());
            б.connect_clicked(move |б| {
                б.set_label("…");
                обнови(&п3, Some(ид3.clone()), &имя3);
            });
            р.add(&б);
            место2.add(&р);
        }
        место2.show_all();
    });

    п.тело.add(&ярлык("Недавние загрузки", "h"));
    let история = п.история.borrow().clone();
    if история.is_empty() {
        let к = gtk::Box::new(gtk::Orientation::Vertical, 0);
        пусто(&к, "Загрузок пока не было. Установите что-нибудь на главной.");
        п.тело.add(&к);
    }
    for з in история.iter().take(15) {
        let действие = match строка(з, "действие").as_str() {
            "install" => "Установка",
            "remove" => "Удаление",
            _ => "Обновление",
        };
        let когда = з["когда"].as_i64().and_then(|т| glib::DateTime::from_unix_local(т).ok()).and_then(|д| д.format("%d.%m %H:%M").ok()).map(|с| с.to_string()).unwrap_or_default();
        let итог = строка(з, "итог");
        let прога = Прога {
            пакет: строка(з, "пакет"),
            имя: строка(з, "имя"),
            флатхаб: if строка(з, "источник") == "flatpak" { строка(з, "пакет") } else { String::new() },
            источник: строка(з, "источник"),
            ..Default::default()
        };
        let р = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        р.style_context().add_class("row");
        р.add(&значок(п, &прога, false));
        let т = gtk::Box::new(gtk::Orientation::Vertical, 3);
        т.set_hexpand(true);
        т.set_valign(gtk::Align::Center);
        т.add(&ярлык(if прога.имя.is_empty() { &прога.пакет } else { &прога.имя }, "n"));
        let ошибка = строка(з, "ошибка");
        т.add(&ярлык(&format!("{действие}, {когда} · {итог}{}", if ошибка.is_empty() { String::new() } else { format!(": {ошибка}") }), "a"));
        if итог == "идёт" && п.работа.get() {
            let полоса = gtk::ProgressBar::new();
            полоса.style_context().add_class("dlbar");
            // Та же доля, что в строке состояния
            полоса.set_fraction(п.статус_полоса.fraction());
            let (полоса2, п2) = (полоса.downgrade(), Rc::downgrade(п));
            glib::timeout_add_local(Duration::from_millis(500), move || {
                let (Some(п), Some(полоса)) = (п2.upgrade(), полоса2.upgrade()) else { return glib::ControlFlow::Break };
                полоса.set_fraction(п.статус_полоса.fraction());
                if п.работа.get() { glib::ControlFlow::Continue } else { glib::ControlFlow::Break }
            });
            т.add(&полоса);
        }
        р.add(&т);
        п.тело.add(&р);
    }
}

/* ---------- библиотека и желаемое ---------- */

fn библиотека(п: &П) {
    let з = заголовок(п, "Библиотека");
    з.style_context().add_class("first");
    let место = gtk::Box::new(gtk::Orientation::Vertical, 10);
    п.тело.add(&место);
    ждём(&место);
    let (место2, п1) = (место.clone(), п.clone());
    спроси(п, "sys.apps", json!({}), move |о| {
        очисти(&место2);
        let mut список = о.ok().and_then(|д| д["list"].as_array().cloned()).unwrap_or_default();
        список.sort_by_key(|а| строка(а, "name").to_lowercase());
        if список.is_empty() {
            пусто(&место2, "Пока пусто. Откройте «Главную» и выберите первую программу.");
        }
        let тема = gtk::IconTheme::default();
        for а in список.iter().take(200) {
            let id = строка(а, "id");
            let имя = { let и = строка(а, "name"); if и.is_empty() { id.clone() } else { и } };
            let р = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            р.style_context().add_class("row");
            // Значок — настоящий, программы: по нему её и узнают
            let картинка = gio::DesktopAppInfo::new(&id)
                .and_then(|д| д.icon())
                .and_then(|и| тема.as_ref()?.lookup_by_gicon(&и, 52, gtk::IconLookupFlags::FORCE_SIZE))
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
            т.set_hexpand(true);
            let и = ярлык(&имя, "n");
            и.set_ellipsize(gtk::pango::EllipsizeMode::End);
            и.set_line_wrap(false);
            т.add(&и);
            let про = строка(а, "comment");
            let а_ = ярлык(if про.is_empty() { "Программа машины" } else { &про }, "a");
            а_.set_ellipsize(gtk::pango::EllipsizeMode::End);
            а_.set_line_wrap(false);
            т.add(&а_);
            р.add(&т);
            let открыть = gtk::Button::with_label("Открыть");
            открыть.style_context().add_class("get");
            открыть.style_context().add_class("open");
            открыть.set_valign(gtk::Align::Center);
            let (п2, id2) = (п1.clone(), id.clone());
            открыть.connect_clicked(move |_| {
                if let Some(д) = gio::DesktopAppInfo::new(&id2) {
                    if let Err(е) = д.launch(&[], None::<&gio::AppLaunchContext>) {
                        сообщи(&п2, "Не запустилась", &е.to_string());
                    }
                }
            });
            р.add(&открыть);
            // Удаляет оболочка: она знает, чем программа поставлена (apt,
            // Flathub, AppImage, Wine), и спросит человека
            let удалить = gtk::Button::with_label("Удалить");
            удалить.style_context().add_class("ghost");
            удалить.set_valign(gtk::Align::Center);
            let (id3, имя3) = (id.clone(), имя.clone());
            удалить.connect_clicked(move |_| скажи("обзор-дело", json!({ "что": "удали", "id": id3, "имя": имя3, "вид": "машина" })));
            р.add(&удалить);
            место2.add(&р);
        }
        место2.show_all();
    });
}

fn желаемое(п: &П) {
    let з = заголовок(п, "Желаемое");
    з.style_context().add_class("first");
    let список = п.желаемое.borrow().clone();
    if список.is_empty() {
        let к = gtk::Box::new(gtk::Orientation::Vertical, 0);
        пусто(&к, "Нажмите ♡ на странице программы, и она появится здесь.");
        п.тело.add(&к);
        return;
    }
    let с = сетка();
    узнай_оценки(п, список.iter().map(|х| х.ключ()).collect());
    for прога in &список {
        с.add(&карточка(п, прога));
    }
    п.тело.add(&с);
}

/* ---------- окно ---------- */

pub fn запусти(доводы: Vec<String>) -> ! {
    подготовь("glower-store", "Магазин");
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль магазина");
    gtk::StyleContext::add_provider_for_screen(&gtk::gdk::Screen::default().expect("экран"), &стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_default_size(1180, 760);
    окно.set_icon_name(Some("system-software-install"));
    окно.style_context().add_class("glower-app");
    crate::apps::строка_заголовка(&окно);
    окно.style_context().add_class("store");

    let слои = gtk::Overlay::new();
    окно.add(&слои);
    let середина = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    слои.add(&середина);
    let бок_прокрутка = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    бок_прокрутка.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    бок_прокрутка.set_size_request(ШИРЕ, -1);
    // Пункты полосы растягиваются внутри неё — но сама она ширины не берёт
    бок_прокрутка.set_hexpand(false);
    let бок = gtk::Box::new(gtk::Orientation::Vertical, 2);
    бок.style_context().add_class("side");
    бок_прокрутка.add(&бок);
    середина.add(&бок_прокрутка);
    let правая = gtk::Box::new(gtk::Orientation::Vertical, 0);
    правая.set_hexpand(true);
    середина.add(&правая);
    let верх = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    верх.style_context().add_class("top");
    let поле = gtk::SearchEntry::new();
    поле.set_placeholder_text(Some("Найти приложение: gimp, telegram, htop…"));
    поле.set_width_chars(40);
    поле.set_halign(gtk::Align::Start);
    верх.add(&поле);
    правая.add(&верх);
    let подсказки = gtk::Popover::new(Some(&поле));
    подсказки.set_modal(false);
    подсказки.set_position(gtk::PositionType::Bottom);
    let подсказки_список = gtk::Box::new(gtk::Orientation::Vertical, 0);
    подсказки_список.style_context().add_class("sug");
    подсказки_список.set_size_request(360, -1);
    подсказки.add(&подсказки_список);
    let тело = gtk::Box::new(gtk::Orientation::Vertical, 6);
    тело.style_context().add_class("body");
    let прокрутка = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    прокрутка.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    прокрутка.set_vexpand(true);
    прокрутка.add(&тело);
    правая.add(&прокрутка);
    // Строка состояния: «$ flatpak install flathub org.gimp.GIMP · 45 %»
    let низ = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    низ.style_context().add_class("status");
    let доллар = gtk::Label::new(Some("$"));
    доллар.style_context().add_class("dollar");
    низ.add(&доллар);
    let подпись_статуса = gtk::Label::new(Some(&crate::i18n::tr("готово к работе")));
    подпись_статуса.set_xalign(0.0);
    подпись_статуса.set_hexpand(true);
    подпись_статуса.set_ellipsize(gtk::pango::EllipsizeMode::End);
    низ.add(&подпись_статуса);
    let статус_полоса = gtk::ProgressBar::new();
    статус_полоса.set_size_request(220, -1);
    статус_полоса.set_valign(gtk::Align::Center);
    статус_полоса.set_no_show_all(true);
    низ.add(&статус_полоса);
    правая.add(&низ);
    let тосты = gtk::Box::new(gtk::Orientation::Vertical, 8);
    тосты.set_halign(gtk::Align::End);
    тосты.set_valign(gtk::Align::Start);
    тосты.set_margin_top(64);
    тосты.set_margin_end(16);
    слои.add_overlay(&тосты);
    слои.set_overlay_pass_through(&тосты, true);

    let mut каталог: Value = serde_json::from_str(КАТАЛОГ).expect("каталог магазина");
    // Каждый раз — в новом порядке: витрина, подборки на главной и в разделах.
    // Магазин, открытый второй раз, не должен показывать то же самое
    if let Some(в) = каталог["витрина"].as_array_mut() {
        перемешай(в);
    }
    if let Some(п) = каталог["подборки"].as_object_mut() {
        for (_, список) in п.iter_mut() {
            if let Some(с) = список.as_array_mut() {
                перемешай(с);
            }
        }
    }
    let mut первый = Экран::Обзор;
    for д in &доводы {
        if let Some(что) = д.strip_prefix("--find=") {
            поле.set_text(что);
            первый = Экран::Поиск(что.to_string());
        } else if д == "updates" {
            первый = Экран::Загрузки;
        } else if д == "installed" {
            первый = Экран::Библиотека;
        }
    }
    let желаемое: Vec<Прога> = crate::apps::читай_данные(ЖЕЛАЕМОЕ).as_array().cloned().unwrap_or_default().iter().map(из_записи).collect();
    let история: Vec<Value> = crate::apps::читай_данные(ИСТОРИЯ).as_array().cloned().unwrap_or_default();
    let п: П = Rc::new(Окно {
        окно: окно.clone(),
        бок,
        тело,
        прокрутка,
        поле: поле.clone(),
        подсказки,
        подсказки_список,
        тосты,
        экран: RefCell::new(первый),
        назад: RefCell::new(Vec::new()),
        поколение: Cell::new(0),
        каталог,
        состояние: RefCell::new(None),
        сведения: RefCell::new(HashMap::new()),
        значки: RefCell::new(HashMap::new()),
        работа: Cell::new(false),
        в_работе: RefCell::new(String::new()),
        кнопка_хода: RefCell::new(None),
        кнопки: RefCell::new(Vec::new()),
        звёзды: RefCell::new(Vec::new()),
        стоят: RefCell::new(HashSet::new()),
        оценки: RefCell::new(HashMap::new()),
        известные: RefCell::new(Vec::new()),
        обновления: RefCell::new(Vec::new()),
        системных: Cell::new(0),
        желаемое: RefCell::new(желаемое),
        история: RefCell::new(история),
        статус: подпись_статуса,
        статус_полоса,
        витрина: Cell::new(0),
        сортировка: RefCell::new("popular".into()),
        сеть: Cell::new(None),
    });
    // Подсказки знают наши подборки с первой же буквы
    for прога in вся_подборка(&п) {
        знаю(&п, &прога);
    }

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
            if поле.has_focus() {
                подскажи(&п1, &поле.text());
            }
        });
    }
    // Работа могла начаться не здесь: щёлкнули по .deb, а Магазин закрыт
    {
        let п1 = п.clone();
        позови("pkg.job", json!({}), move |о| {
            if о.is_ok_and(|j| j["running"].as_bool() == Some(true)) {
                статус(&п1, "работа системы");
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
                    Экран::Загрузки
                } else if вкладка == "installed" {
                    Экран::Библиотека
                } else {
                    continue;
                };
                иди(&п, куда);
                п.окно.present();
            }
        });
    }
    // Источник и «только свободные» поменяли в Параметрах — перерисоваться
    {
        let файл = gio::File::for_path(glib::user_config_dir().join("glower").join("settings.json"));
        if let Ok(слежка) = файл.monitor_file(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE) {
            let п1 = Rc::downgrade(&п);
            let было = Rc::new(RefCell::new((источник(), только_свободные())));
            слежка.connect_changed(move |_, _, _, _| {
                let Some(п) = п1.upgrade() else { return };
                // Файл — главный: что бы ни выбрали здесь, слово за ним
                if настройки()["storeSource"].as_str().is_some() {
                    ВЫБРАН.with(|в| *в.borrow_mut() = None);
                }
                let стало = (источник(), только_свободные());
                if *было.borrow() != стало {
                    *было.borrow_mut() = стало;
                    рисуй(&п);
                }
            });
            // Слежка живёт, пока живёт окно
            unsafe { окно.set_data("слежка", слежка) };
        }
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
    {
        let п1 = Rc::downgrade(&п);
        окно.connect_size_allocate(move |о, _| {
            let узко = о.allocated_width() < ПОРОГ_ОКНА;
            if УЗКО.with(|у| у.replace(узко)) != узко {
                let п2 = п1.clone();
                // Не посреди раскладки — следующим шагом
                glib::idle_add_local_once(move || {
                    if let Some(п) = п2.upgrade() {
                        рисуй_бок(&п);
                    }
                });
            }
        });
    }
    окно.show_all();
    рисуй(&п);
    проверь_сеть(&п);
    узнай_поставленное(&п);
    узнай_обновления(&п, |_| {});
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
        assert_eq!(сколько(3_966_300), "4,0 млн");
        assert_eq!(сколько(52_953), "53 тыс.");
        assert_eq!(сколько(17), "17");
        assert_eq!(звёзды_текст((4.25, 10)), "★ 4,2");
    }

    #[test]
    fn оценки_весят_по_числу_голосов() {
        // 4,7 из тысячи — выше трёх пятёрок
        assert!(вес(Some((4.7, 1000))) > вес(Some((5.0, 3))));
        assert_eq!(вес(None), 0.0);
    }

    #[test]
    fn команда_словами_системы() {
        assert_eq!(команда("install", "org.gimp.GIMP", "flatpak"), "flatpak install flathub org.gimp.GIMP");
        assert_eq!(команда("install", "vlc", ""), "sudo apt install vlc");
        assert_eq!(команда("update", "", "flatpak"), "flatpak update");
        assert_eq!(команда("remove", "vlc", "apt"), "sudo apt remove vlc");
    }

    #[test]
    fn желаемое_туда_и_обратно() {
        let п = Прога { пакет: "org.gimp.GIMP".into(), имя: "GIMP".into(), флатхаб: "org.gimp.GIMP".into(), источник: "flatpak".into(), ..Default::default() };
        let н = из_записи(&п.в_json());
        assert_eq!(н.ключ(), "org.gimp.GIMP");
        assert!(н.с_flathub());
        assert_eq!(раздел_по_категориям(&[json!("Graphics"), json!("2DGraphics")]), Some("графика"));
        assert_eq!(раздел_по_категориям(&[json!("Other")]), None);
    }
}
