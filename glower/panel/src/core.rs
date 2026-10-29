//! Ядро оболочки — то, что прежде делала страница рабочего стола.
//!
//! Страница на браузерном движке была мозгом системы: она хранила
//! настройки, знала, что лежит на столе и что закреплено в доке, считала
//! рабочие столы, проверяла обновления, следила за флешками и зарядом и
//! выполняла просьбы панели, дока, меню и Параметров. Всё это теперь здесь,
//! в той же программе, что панель.
//!
//! Говорим тем же языком, что говорила страница, — теми же темами шины.
//! Поэтому док, меню программ, стол и Параметры переезда не заметили:
//! они слышат «док», «значки», «программы», «параметры» как и прежде, а
//! просьбы шлют туда же — «обзор-дело», «значки-дело», «параметры-дело».
//!
//! Настройки лежат в ~/.config/glower/settings.json. Пишет их только ядро:
//! остальные просят.

use crate::agent::{зов, позови, скажи, слушай};
use crate::notify;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use serde_json::{json, Map, Value};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::time::Duration;

const ДАННЫЕ: &str = include_str!("shell-data.json");

/// Программы системы. Окна у них настоящие, имя окна — app_id
pub struct Наша {
    pub id: &'static str,
    pub имя: &'static str,
    pub знак: &'static str,
    pub фон: &'static str,
    pub окно: &'static str,
    pub одна: bool,
}

pub const НАШИ: &[Наша] = &[
    Наша { id: "files", имя: "Проводник", знак: "📁", фон: "linear-gradient(140deg,#fcd34d,#f59e0b)", окно: "glower-files", одна: false },
    Наша { id: "browser", имя: "Браузер", знак: "🌐", фон: "linear-gradient(140deg,#38bdf8,#2563eb)", окно: "", одна: false },
    Наша { id: "notepad", имя: "Блокнот", знак: "📝", фон: "linear-gradient(140deg,#7dd3fc,#3b82f6)", окно: "glower-notepad", одна: false },
    Наша { id: "settings", имя: "Параметры", знак: "⚙️", фон: "linear-gradient(140deg,#94a3b8,#334155)", окно: "glower-settings", одна: true },
    Наша { id: "store", имя: "Магазин", знак: "🛍️", фон: "linear-gradient(140deg,#c4b5fd,#7c3aed)", окно: "glower-store", одна: true },
    Наша { id: "music", имя: "Музыка", знак: "🎵", фон: "linear-gradient(140deg,#f472b6,#a855f7)", окно: "glower-music", одна: true },
    Наша { id: "term", имя: "Терминал", знак: "⌨️", фон: "linear-gradient(140deg,#334155,#0f172a)", окно: "foot", одна: false },
    Наша { id: "calc", имя: "Калькулятор", знак: "🧮", фон: "linear-gradient(140deg,#a5b4fc,#6366f1)", окно: "glower-calc", одна: true },
    Наша { id: "photos", имя: "Фотографии", знак: "🖼️", фон: "linear-gradient(140deg,#6ee7b7,#0ea5e9)", окно: "glower-photos", одна: false },
    Наша { id: "taskmgr", имя: "Диспетчер задач", знак: "📊", фон: "linear-gradient(140deg,#cbd5e1,#475569)", окно: "glower-taskmgr", одна: true },
    Наша { id: "snip", имя: "Ножницы", знак: "✂️", фон: "linear-gradient(140deg,#7dd3fc,#0ea5e9)", окно: "glower-snip", одна: true },
    Наша { id: "paint", имя: "Paint", знак: "🎨", фон: "linear-gradient(140deg,#f9a8d4,#a855f7)", окно: "glower-paint", одна: false },
    Наша { id: "calendar", имя: "Календарь", знак: "📅", фон: "linear-gradient(140deg,#fca5a5,#ef4444)", окно: "glower-calendar", одна: true },
    Наша { id: "clock", имя: "Часы", знак: "🕐", фон: "linear-gradient(140deg,#93c5fd,#1d4ed8)", окно: "glower-clock", одна: true },
    Наша { id: "todo", имя: "Дела", знак: "✅", фон: "linear-gradient(140deg,#86efac,#16a34a)", окно: "glower-todo", одна: true },
    Наша { id: "installer", имя: "Установка GlowerOS", знак: "💽", фон: "linear-gradient(140deg,#a78bfa,#4c1d95)", окно: "glower-installer", одна: true },
];

pub fn наша(id: &str) -> Option<&'static Наша> {
    НАШИ.iter().find(|н| н.id == id)
}

/// Браузеры машины — в порядке, в каком их предлагаем
const БРАУЗЕРЫ: &[&str] = &[
    "firefox.desktop", "firefox-esr.desktop", "org.mozilla.firefox.desktop", "chromium.desktop", "chromium-browser.desktop",
    "org.chromium.Chromium.desktop", "google-chrome.desktop", "org.gnome.Epiphany.desktop", "epiphany-browser.desktop",
];

/// Языки системы: что пишем в ~/.config/glower/locale
pub const ЯЗЫКИ: &[(&str, &str, &str)] = &[
    ("ru", "Русский", "ru_RU.UTF-8"),
    ("en", "English", "en_US.UTF-8"),
    ("uk", "Українська", "uk_UA.UTF-8"),
    ("be", "Беларуская", "be_BY.UTF-8"),
    ("kk", "Қазақша", "kk_KZ.UTF-8"),
    ("de", "Deutsch", "de_DE.UTF-8"),
    ("fr", "Français", "fr_FR.UTF-8"),
    ("es", "Español", "es_ES.UTF-8"),
    ("it", "Italiano", "it_IT.UTF-8"),
    ("pt", "Português", "pt_BR.UTF-8"),
    ("pl", "Polski", "pl_PL.UTF-8"),
    ("tr", "Türkçe", "tr_TR.UTF-8"),
];

pub const ВИДЖЕТЫ: &[(&str, &str, &str)] = &[
    ("clock", "Часы", "🕐"),
    ("weather", "Погода", "🌤"),
    ("calendar", "Календарь", "📅"),
    ("system", "Система", "📊"),
    ("todo", "Дела", "✅"),
    ("note", "Заметка", "📝"),
];

pub fn умолчания() -> Value {
    json!({
        "theme": "dark", "accent": 0, "accentCustom": null,
        "wallpaper": "glower", "wallShuffle": false,
        "clock24": true, "showSeconds": false, "tz": "", "city": "Москва",
        "dockApps": ["browser", "files", "notepad", "settings", "store", "music", "term", "calc"],
        "showDesktopIcons": true, "showDeskWidgets": true,
        "deskWidgets": [{ "t": "clock" }, { "t": "weather" }],
        "deskApps": [], "deskNative": [], "iconPos": {},
        "dnd": false, "notif": true, "soundNotif": true, "nightLight": false,
        "autoLock": 0, "lockOnWake": true, "autostart": [], "focusLen": "25",
        "userName": "", "userEmoji": "", "desks": 2, "note": "",
        "setupDone": false, "updatesAsked": 0
    })
}

/* ---------- состояние ---------- */

#[derive(Default)]
struct Ядро {
    s: Value,
    данные: Value,
    машина: Vec<Value>,
    окна: Vec<Value>,
    стол: u64,
    раскладки: (Vec<String>, Vec<Value>),
    фокус_до: Option<i64>,
    носители: Option<Vec<String>>,
    заряд_сказан: u8,
    обновления: Value,
    свёрнутые: Vec<(String, String)>,
    погода: Value,
    можно_ставить: bool,
    /// На каком столе какое окно. Оконный сервер этого не говорит, поэтому
    /// помним сами: окно живёт на том столе, где появилось, пока его не
    /// отправили на другой
    где_окно: std::collections::HashMap<String, u64>,
}

thread_local! {
    static Я: RefCell<Option<Ядро>> = const { RefCell::new(None) };
    static ЖДЁТ: Cell<bool> = const { Cell::new(false) };
}

fn с_ядром<R>(дело: impl FnOnce(&mut Ядро) -> R) -> Option<R> {
    Я.with(|я| я.borrow_mut().as_mut().map(дело))
}

pub fn настройка(k: &str) -> Value {
    с_ядром(|я| я.s[k].clone()).unwrap_or(Value::Null)
}

fn путь_настроек() -> PathBuf {
    glib::user_config_dir().join("glower").join("settings.json")
}

pub fn корень() -> PathBuf {
    std::env::var("GLOWER_ROOT").map(PathBuf::from).unwrap_or_else(|_| glib::home_dir().join("GlowerOS"))
}

pub fn папка_стола() -> PathBuf {
    корень().join("Рабочий стол")
}

fn загрузи() -> Value {
    let mut s = умолчания();
    let прочитано = std::fs::read_to_string(путь_настроек()).ok().and_then(|т| serde_json::from_str::<Value>(&т).ok());
    match прочитано {
        Some(Value::Object(свои)) => {
            for (к, в) in свои {
                s[&к] = в;
            }
        }
        _ => {
            // Настроек ещё нет. Если система уже жила здесь при странице —
            // мастер второй раз не нужен: человек его проходил
            let жил = glib::user_config_dir().join("glower").join("keyboard").exists()
                || glib::user_data_dir().join("glower").join("browser").exists()
                || glib::user_config_dir().join("glower").join("locale").exists();
            s["setupDone"] = json!(жил);
        }
    }
    if s["userName"].as_str().unwrap_or("").is_empty() {
        let имя = glib::real_name().to_string_lossy().to_string();
        s["userName"] = json!(if имя.is_empty() || имя == "Unknown" { glib::user_name().to_string_lossy().to_string() } else { имя });
    }
    s
}

fn сохрани() {
    let Some(s) = с_ядром(|я| я.s.clone()) else { return };
    let путь = путь_настроек();
    if let Some(р) = путь.parent() {
        let _ = std::fs::create_dir_all(р);
    }
    // Сперва во временный файл, потом подменой: выключение посреди записи
    // не должно оставить пустые настройки
    let временный = путь.with_extension("json.new");
    if std::fs::write(&временный, serde_json::to_vec_pretty(&s).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(&временный, &путь);
    }
}

pub fn задай(k: &str, v: Value) {
    с_ядром(|я| я.s[k] = v);
    сохрани();
    после(k);
    расскажи_скоро();
}

/// Сохранить без рассылки: заметка на виджете пишется на каждый знак
pub fn задай_тихо(k: &str, v: Value) {
    с_ядром(|я| я.s[k] = v);
    сохрани();
}

/// Убрать значок программы со стола
pub fn убери_со_стола(id: &str, вид: &str) {
    if вид == "машина" {
        let с: Vec<Value> = настройка("deskNative").as_array().cloned().unwrap_or_default().into_iter().filter(|п| п["id"].as_str() != Some(id)).collect();
        задай("deskNative", Value::Array(с));
    } else {
        let с: Vec<Value> = настройка("deskApps").as_array().cloned().unwrap_or_default().into_iter().filter(|п| п.as_str() != Some(id)).collect();
        задай("deskApps", Value::Array(с));
    }
}

/// Что ещё сделать, когда сменилась настройка
fn после(k: &str) {
    match k {
        "nightLight" => ночной_свет(),
        "autoLock" => авто_замок(),
        "desks" => {}
        _ => {}
    }
}

/// Рассказать всем — один раз на пачку перемен
fn расскажи_скоро() {
    if ЖДЁТ.with(|ж| ж.replace(true)) {
        return;
    }
    glib::timeout_add_local_once(Duration::from_millis(120), || {
        ЖДЁТ.with(|ж| ж.set(false));
        расскажи_всё();
    });
}

/* ---------- что рассказываем ---------- */

fn ссылка_на_файл(путь: &std::path::Path) -> String {
    format!("url(\"file://{}\")", путь.to_string_lossy().replace('"', "%22"))
}

/// Фон стола словами CSS. Свои обои — картинки в assets/wallpapers; нет
/// файла (стенд без картинок, урезанная сборка) — запасной градиент тех же
/// цветов из того же списка
fn обои_css(id: &str, данные: &Value) -> String {
    if let Some(путь) = id.strip_prefix("файл:") {
        return ссылка_на_файл(std::path::Path::new(путь));
    }
    let Some(в) = данные["обои"].as_array().and_then(|о| о.iter().find(|в| в["id"].as_str() == Some(id)).or(о.first())) else {
        return "linear-gradient(#0b0d1a, #05060c)".into();
    };
    if let Some(путь) = в["файл"].as_str().and_then(crate::apps::файл_системы) {
        return ссылка_на_файл(&путь);
    }
    в["css"].as_str().unwrap_or("linear-gradient(#0b0d1a, #05060c)").to_string()
}

/// Обои по имени — для замка и экрана входа: они живут отдельно от стола
pub fn обои_по_имени(id: &str) -> String {
    обои_css(id, &serde_json::from_str(ДАННЫЕ).unwrap_or_default())
}

/// Список своих обоев для выбора: вместо полной картинки — маленькая (её
/// и показывает плитка), чтобы окно выбора не грузило десять больших файлов
pub fn обои_для_выбора() -> Value {
    let данные: Value = serde_json::from_str(ДАННЫЕ).unwrap_or_default();
    let mut список = данные["обои"].as_array().cloned().unwrap_or_default();
    for в in &mut список {
        let мини = в["файл"].as_str().map(|ф| ф.replacen("wallpapers/", "wallpapers/mini/", 1));
        if let Some(путь) = мини.as_deref().and_then(crate::apps::файл_системы) {
            в["css"] = json!(ссылка_на_файл(&путь));
        }
    }
    Value::Array(список)
}

fn сведения_нашей(н: &Наша) -> Value {
    json!({ "id": н.id, "имя": н.имя, "знак": н.знак, "фон": н.фон, "вид": "приложение" })
}

fn программы_меню(я: &Ядро) -> Vec<Value> {
    let mut список: Vec<Value> = НАШИ
        .iter()
        .filter(|н| н.id != "installer" || я.можно_ставить)
        .map(сведения_нашей)
        .collect();
    let mut машина: Vec<Value> = я
        .машина
        .iter()
        .map(|а| json!({ "id": а["id"], "имя": а["name"], "знак": "", "фон": "", "вид": "машина", "значок": а["icon"] }))
        .collect();
    машина.sort_by_key(|а| а["имя"].as_str().unwrap_or("").to_lowercase());
    список.extend(машина);
    список
}

fn значки_стола(я: &Ядро) -> Vec<Value> {
    if я.s["showDesktopIcons"].as_bool() == Some(false) {
        return Vec::new();
    }
    let места = я.s["iconPos"].clone();
    // «Этот компьютер» — по умолчанию на столе, но и его можно убрать
    let mut значки: Vec<Value> = if я.s["deskThisPC"].as_bool() == Some(false) {
        Vec::new()
    } else {
        vec![json!({ "n": "Этот компьютер", "g": "💻", "вид": "система" })]
    };
    for id in я.s["deskApps"].as_array().cloned().unwrap_or_default() {
        if let Some(н) = id.as_str().and_then(наша) {
            значки.push(json!({ "n": н.имя, "g": н.знак, "вид": "приложение", "ярлык": н.id }));
        }
    }
    for п in я.s["deskNative"].as_array().cloned().unwrap_or_default() {
        let id = п["id"].as_str().unwrap_or("").to_string();
        let имя = п["имя"].as_str().map(String::from).or_else(|| gio::DesktopAppInfo::new(&id).map(|д| д.name().to_string())).unwrap_or_else(|| id.clone());
        значки.push(json!({ "n": имя, "g": "", "вид": "машина", "ярлык": id }));
    }
    let mut файлы: Vec<(String, bool)> = std::fs::read_dir(папка_стола())
        .map(|ч| {
            ч.filter_map(|з| з.ok())
                .filter_map(|з| {
                    let имя = з.file_name().to_string_lossy().to_string();
                    (!имя.starts_with('.')).then(|| (имя, з.path().is_dir()))
                })
                .collect()
        })
        .unwrap_or_default();
    файлы.sort_by_key(|(и, папка)| (!*папка, и.to_lowercase()));
    for (имя, папка) in файлы {
        let тип = if папка { "inode/directory".into() } else { тип_по_имени(&имя) };
        let значок = gio::content_type_get_icon(&тип).downcast::<gio::ThemedIcon>().ok().and_then(|т| т.names().first().map(|н| н.to_string())).unwrap_or_default();
        // Время изменения — чтобы стол заметил пересохранённую картинку и
        // обновил миниатюру
        let когда = std::fs::metadata(папка_стола().join(&имя)).and_then(|м| м.modified()).ok().and_then(|в| в.duration_since(std::time::UNIX_EPOCH).ok()).map(|д| д.as_secs()).unwrap_or(0);
        значки.push(json!({ "n": имя, "g": if папка { "📁" } else { знак_файла(&тип) }, "вид": if папка { "папка" } else { "файл" }, "значок": значок, "тип": тип, "t": когда }));
    }
    // Места: запомненные — как есть, остальные — столбиками сверху вниз
    let (ширина, высота) = gtk::gdk::Display::default()
        .and_then(|д| д.primary_monitor().or_else(|| д.monitor(0)))
        .map(|м| (м.geometry().width() as i64, м.geometry().height() as i64))
        .unwrap_or((1280, 800));
    let в_столбце = ((высота - 140) / 104).max(3);
    let mut занято: Vec<(i64, i64)> = Vec::new();
    for з in &значки {
        if let Some(м) = места.get(з["n"].as_str().unwrap_or("")) {
            // Места пишутся дробными (970.0): as_i64 их не читает, и значок
            // будто стоял в углу — первое место столбца пустело зря
            занято.push((м["x"].as_f64().unwrap_or(0.0) as i64, м["y"].as_f64().unwrap_or(0.0) as i64));
        }
    }
    let mut номер = 0i64;
    for з in значки.iter_mut() {
        let имя = з["n"].as_str().unwrap_or("").to_string();
        if let Some(м) = места.get(&имя) {
            // Значок целиком на экране, даже если место запомнено с
            // экрана побольше
            з["x"] = json!(м["x"].as_f64().unwrap_or(0.0).clamp(0.0, (ширина - 96) as f64));
            з["y"] = json!(м["y"].as_f64().unwrap_or(0.0).clamp(30.0, (высота - 100) as f64));
            continue;
        }
        loop {
            let (x, y) = (16 + (номер / в_столбце) * 100, 44 + (номер % в_столбце) * 104);
            номер += 1;
            if !занято.iter().any(|(зx, зy)| (зx - x).abs() < 60 && (зy - y).abs() < 60) {
                з["x"] = json!(x);
                з["y"] = json!(y);
                break;
            }
        }
    }
    значки
}

fn знак_файла(тип: &str) -> &'static str {
    match тип {
        т if т.starts_with("image/") => "🖼️",
        т if т.starts_with("audio/") => "🎵",
        т if т.starts_with("video/") => "🎬",
        т if т.contains("zip") || т.contains("tar") || т.contains("compressed") || т.contains("x-7z") => "🗜️",
        т if т.contains("pdf") => "📕",
        т if т.contains("x-ms-dos-executable") || т.contains("msdownload") || т.contains("msi") => "🪟",
        т if т.contains("debian") || т.contains("x-deb") => "📦",
        _ => "📄",
    }
}

fn расскажи_всё() {
    let Some((s, данные, программы, значки, стол, раскладки, фокус, можно)) = с_ядром(|я| {
        (я.s.clone(), я.данные.clone(), программы_меню(я), значки_стола(я), я.стол, я.раскладки.clone(), я.фокус_до, я.можно_ставить)
    }) else {
        return;
    };
    let _ = можно;
    crate::desktop::обнови_виджеты();
    скажи("программы", Value::Array(программы.clone()));
    скажи("значки", Value::Array(значки));
    скажи("обои", json!({ "css": обои_css(s["wallpaper"].as_str().unwrap_or(""), &данные) }));
    скажи("часы", json!({ "пояс": s["tz"], "ч24": s["clock24"] }));
    скажи("настройки", json!({ "dnd": s["dnd"], "nightLight": s["nightLight"], "эконом": false }));
    let закреплены: Vec<Value> = s["dockApps"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|id| id.as_str().and_then(наша))
        .map(сведения_нашей)
        .collect();
    скажи("док", json!({ "закреплены": закреплены, "сведения": [], "запущены": [], "активное": "", "окон": 0 }));
    скажи("обзор-данные", данные_обзора(&s, стол));
    let коды: Vec<String> = раскладки.0.iter().map(|к| if к == "us" { "EN".into() } else { к.to_uppercase() }).collect();
    if !коды.is_empty() {
        скажи("раскладка", json!(коды.join("·")));
    }
    // Параметрам — в том виде, в каком им рассказывала страница
    let осталось = фокус.map(|до| ((до - glib::real_time() / 1_000_000).max(0)) as u64).unwrap_or(0);
    let mut kv = Map::new();
    for к in ["notif", "autoLock", "focusLen"] {
        kv.insert(к.into(), s[к].clone());
    }
    скажи(
        "параметры",
        json!({
            "s": s,
            "kv": kv,
            "обои": обои_для_выбора(),
            "акценты": данные["акценты"],
            "программы": НАШИ.iter().filter(|н| н.id != "installer").map(сведения_нашей).collect::<Vec<_>>(),
            "виджеты": ВИДЖЕТЫ.iter().map(|(t, и, з)| json!({ "t": t, "имя": и, "знак": з })).collect::<Vec<_>>(),
            "столов": s["desks"],
            "языки": ЯЗЫКИ.iter().map(|(k, и, _)| json!({ "k": k, "имя": и })).collect::<Vec<_>>(),
            "язык": язык_сейчас(),
            "раскладки": раскладки.0,
            "все_раскладки": раскладки.1,
            "бренд": { "имя": "GlowerOS", "полное": бренд_полное(), "версия": версия() },
            "профиль": { "имя": s["userName"], "знак": s["userEmoji"] },
            "фокус": осталось,
            "нативно": true
        }),
    );
    let _ = программы;
}

fn данные_обзора(s: &Value, стол: u64) -> Value {
    let окна = окна_этого_стола();
    let столов = s["desks"].as_u64().unwrap_or(2).clamp(1, 4);
    let на_столах: Vec<u64> = с_ядром(|я| {
        (0..столов).map(|n| я.окна.iter().filter(|о| я.где_окно.get(&ключ_окна(о)).copied().unwrap_or(я.стол) == n).count() as u64).collect()
    })
    .unwrap_or_default();
    let окна: Vec<Value> = окна
        .iter()
        .map(|о| {
            let app = о["appId"].as_str().unwrap_or("");
            let наша = НАШИ.iter().find(|н| !н.окно.is_empty() && н.окно == app);
            json!({
                "id": format!("{}\t{}", app, о["title"].as_str().unwrap_or("")),
                "имя": о["title"],
                "знак": наша.map(|н| н.знак).unwrap_or(""),
                "фон": наша.map(|н| н.фон).unwrap_or(""),
                "приложение": app,
                "свёрнуто": о["состояние"]["свёрнуто"].as_bool() == Some(true),
            })
        })
        .collect();
    let на_столе: Vec<Value> = s["deskNative"].as_array().cloned().unwrap_or_default().iter().map(|п| п["id"].clone()).collect();
    json!({
        "столы": s["desks"].as_u64().unwrap_or(2).clamp(1, 4),
        "текущий": стол,
        "можно-ещё": s["desks"].as_u64().unwrap_or(2) < 4,
        "можно-убрать": столов > 1,
        "на-столах": на_столах,
        "окна": окна,
        "на-столе": на_столе,
        "в-доке": s["dockApps"],
    })
}

fn бренд_полное() -> String {
    format!("GlowerOS {}", версия())
}

pub fn версия() -> String {
    // Версию системы знает пакет: dpkg пишет её в свою базу. Спрашиваем
    // один раз: рассказ о состоянии идёт при каждой смене настройки, и
    // запуск dpkg-query каждый раз подвешивал панель на десятки миллисекунд.
    // За сеанс версия не меняется — новая заработает после нового входа
    static ВЕРСИЯ: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ВЕРСИЯ
        .get_or_init(|| {
            std::process::Command::new("dpkg-query")
                .args(["-W", "-f=${Version}", "glower"])
                .output()
                .ok()
                .filter(|в| в.status.success())
                .map(|в| String::from_utf8_lossy(&в.stdout).trim().to_string())
                .filter(|в| !в.is_empty())
                .unwrap_or_else(|| "12".into())
        })
        .clone()
}

pub(crate) fn язык_сейчас() -> String {
    let файл = std::fs::read_to_string(glib::user_config_dir().join("glower").join("locale")).unwrap_or_default();
    let локаль = if файл.trim().is_empty() { std::env::var("LANG").unwrap_or_default() } else { файл.trim().to_string() };
    ЯЗЫКИ.iter().find(|(_, _, л)| локаль.starts_with(&л[..5])).map(|(k, _, _)| k.to_string()).unwrap_or_else(|| "ru".into())
}

/* ---------- запуск программ ---------- */

/// Запустить и забыть. Ждём выхода в стороне, иначе панель, которая живёт
/// весь сеанс, копила бы зомби от каждой открытой программы
pub fn запусти_команду(команда: &str, доводы: &[&str]) -> bool {
    match std::process::Command::new(команда).args(доводы).spawn() {
        Ok(mut ребёнок) => {
            std::thread::spawn(move || ребёнок.wait());
            true
        }
        Err(_) => false,
    }
}

fn окно_есть(app: &str) -> Option<Value> {
    с_ядром(|я| я.окна.iter().find(|о| о["appId"].as_str() == Some(app)).cloned()).flatten()
}

fn к_окну(о: &Value) {
    позови("sys.window", json!({ "action": "focus", "appId": о["appId"], "title": о["title"] }), |_| {});
}

/// Параметры на нужном разделе: уже открытые просто листаем туда
pub fn открой_параметры(раздел: &str) {
    match окно_есть("glower-settings") {
        Some(о) => {
            скажи("параметры-раздел", json!(раздел));
            к_окну(&о);
        }
        None => {
            запусти_команду("glower-settings", &[раздел]);
        }
    }
}

/// Открыть файлы программой — их бросили на её значок, как в Windows.
/// false — эта программа файлов не открывает
pub fn открой_файлы_программой(id: &str, вид: &str, пути: &[std::path::PathBuf]) -> bool {
    if пути.is_empty() {
        return false;
    }
    let файлы: Vec<gio::File> = пути.iter().map(gio::File::for_path).collect();
    let чужой = |д: gio::DesktopAppInfo| д.launch(&файлы, None::<&gio::AppLaunchContext>).is_ok();
    if вид == "машина" {
        return gio::DesktopAppInfo::new(id).or_else(|| gio::DesktopAppInfo::new(&format!("{id}.desktop"))).is_some_and(чужой);
    }
    let доводы: Vec<String> = пути.iter().map(|п| п.to_string_lossy().to_string()).collect();
    let доводы: Vec<&str> = доводы.iter().map(String::as_str).collect();
    match id {
        "browser" => БРАУЗЕРЫ.iter().find_map(|я| gio::DesktopAppInfo::new(я)).is_some_and(чужой),
        "files" | "notepad" | "photos" | "paint" | "music" => наша(id).is_some_and(|н| запусти_команду(н.окно, &доводы)),
        _ => false,
    }
}

/// Открыть программу: свою, машины или по имени ярлыка
pub fn запусти(id: &str, вид: &str) {
    if вид == "машина" {
        match gio::DesktopAppInfo::new(id) {
            Some(д) => {
                if let Err(е) = д.launch(&[], None::<&gio::AppLaunchContext>) {
                    notify::покажи(&д.name(), &format!("Не запустилась: {е}"), "⚠️");
                }
            }
            None => notify::покажи("Программы машины", "Такой программы на машине больше нет", "⚠️"),
        }
        return;
    }
    let Some(н) = наша(id) else {
        if id == "trash" {
            запусти_команду("glower-files", &["--trash"]);
        }
        return;
    };
    if н.одна {
        if let Some(о) = окно_есть(н.окно) {
            return к_окну(&о);
        }
    }
    let удалось = match id {
        "term" => {
            позови("sys.terminal", json!({}), |о| {
                if о.is_err() && !запусти_команду("foot", &[]) {
                    notify::покажи("Терминал", "На машине нет программы терминала", "⚠️");
                }
            });
            true
        }
        "browser" => match БРАУЗЕРЫ.iter().find_map(|я| gio::DesktopAppInfo::new(я)) {
            Some(д) => д.launch(&[], None::<&gio::AppLaunchContext>).is_ok(),
            // Браузера на машине нет — Магазин сразу на нём
            None => запусти_команду("glower-store", &["--find=firefox"]),
        },
        _ => запусти_команду(н.окно, &[]),
    };
    if !удалось {
        notify::покажи(н.имя, "Не запустилась", "⚠️");
    }
}

/// Открыть файл тем, чем его открывает система
/// Тип файла по имени. Обёртка gtk-rs передаёт пустые данные не пустым
/// указателем, и GLib решает, что файл нулевой длины: выходило
/// application/x-zerosize для любого файла — стол не узнавал картинки,
/// отдавал их «программе по умолчанию» и показывал им значок документа
pub fn тип_по_имени(имя: &str) -> String {
    use glib::translate::{from_glib_full, ToGlibPtr};
    unsafe {
        let mut неточно = 0;
        let т: glib::GString = from_glib_full(gio::ffi::g_content_type_guess(имя.to_glib_none().0, std::ptr::null(), 0, &mut неточно));
        т.to_string()
    }
}

/// Тип файла: по содержимому, а не получится — по имени. Так узнаётся и
/// картинка без расширения, и файл с чужим расширением
pub fn тип_файла(путь: &std::path::Path) -> String {
    gio::File::for_path(путь)
        .query_info("standard::content-type", gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE)
        .ok()
        .and_then(|и| и.content_type())
        .map(|т| т.to_string())
        .filter(|т| т != "application/octet-stream" && т != "application/x-zerosize")
        .unwrap_or_else(|| тип_по_имени(&путь.to_string_lossy()))
}

/// Программа, которую человек сам назначил для типа: строка в его
/// ~/.config/mimeapps.list. Системные умолчания сюда не входят
fn выбрано_человеком(тип: &str) -> Option<gio::AppInfo> {
    let текст = std::fs::read_to_string(glib::user_config_dir().join("mimeapps.list")).ok()?;
    let mut в_разделе = false;
    for с in текст.lines() {
        let с = с.trim();
        if с.starts_with('[') {
            в_разделе = с == "[Default Applications]";
            continue;
        }
        if !в_разделе {
            continue;
        }
        if let Some(значение) = с.strip_prefix(тип).and_then(|х| х.strip_prefix('=')) {
            return значение.split(';').filter(|и| !и.is_empty()).find_map(|и| gio::DesktopAppInfo::new(и)).map(|д| д.upcast());
        }
    }
    None
}

/// Открыть своей программой, если файл ей по силам: текст — Блокнотом,
/// картинку — Фото, звук — Музыкой, пакет — установщиком программ.
/// false — своей нет, пусть открывает программа, назначенная в системе
pub fn открой_своей(путь: &std::path::Path) -> bool {
    let тип = тип_файла(путь);
    let строка = путь.to_string_lossy();
    // Человек сам выбрал программу («Открыть с помощью» → «Всегда») —
    // слушаемся его, как Windows, а не своих правил
    if let Some(п) = выбрано_человеком(&тип) {
        return п.launch(&[gio::File::for_path(путь)], None::<&gio::AppLaunchContext>).is_ok();
    }
    let пакет = тип.contains("debian") || строка.ends_with(".deb") || строка.ends_with(".flatpakref") || строка.ends_with(".AppImage");
    let чем = if пакет {
        "glower-open-package"
    } else if gio::content_type_is_a(&тип, "text/plain") {
        "glower-notepad"
    } else if тип.starts_with("image/") {
        "glower-photos"
    } else if тип.starts_with("audio/") {
        "glower-music"
    } else {
        return false;
    };
    запусти_команду(чем, &[&строка])
}

pub fn открой_путь(путь: &std::path::Path) {
    if путь.is_dir() {
        запусти_команду("glower-files", &[&путь.to_string_lossy()]);
        return;
    }
    if crate::fileops::архив_ли(путь) {
        crate::fileops::распакуй(путь.to_path_buf(), true, |итог| match итог {
            Ok(п) => {
                запусти_команду("glower-files", &[&п.to_string_lossy()]);
            }
            Err(е) => notify::покажи("С архивом не вышло", &е, "⚠️"),
        });
        return;
    }
    if открой_своей(путь) {
        return;
    }
    let uri = gio::File::for_path(путь).uri();
    if gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>).is_err() {
        notify::покажи(&путь.file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_default(), "Нечем открыть такой файл", "⚠️");
    }
}

/* ---------- рабочие столы ----------

   Столы — настоящие, оконного сервера: окна других столов он прячет сам.
   Попросить его перейти можно только сочетанием клавиш, поэтому в rc.xml
   на Win+Ctrl+Alt+F1…F4 висят «перейти на стол», а с Shift — «отправить
   окно на стол», и мы нажимаем их через wtype. */

fn клавиши(доводы: &[&str]) -> bool {
    // Под своим оконным сервером (glower-wm) просим словами через его
    // сокет: нажатия wtype уходят прямо программе в фокусе, мимо сервера
    if let Some(просьба) = просьба_серверу(доводы) {
        if скажи_серверу(&просьба).is_some() {
            return true;
        }
    }
    std::process::Command::new("wtype").args(доводы).status().map(|с| с.success()).unwrap_or(false)
}

/// Служебное сочетание → строка для сокета glower-wm: «desk 2», «send 3»
fn просьба_серверу(доводы: &[&str]) -> Option<String> {
    let клавиша = доводы.iter().position(|д| *д == "-k").and_then(|i| доводы.get(i + 1))?;
    let n: u64 = клавиша.strip_prefix('F')?.parse().ok()?;
    let отправить = доводы.windows(2).any(|п| п[0] == "-M" && п[1] == "shift");
    Some(format!("{} {n}", if отправить { "send" } else { "desk" }))
}

/// Строка серверу glower-wm и его ответ. None — сервера нет (сеанс под labwc)
pub fn скажи_серверу(просьба: &str) -> Option<String> {
    use std::io::{Read, Write};
    let путь = std::env::var_os("GLOWER_WM_SOCKET")?;
    let mut с = std::os::unix::net::UnixStream::connect(путь).ok()?;
    let _ = с.set_read_timeout(Some(Duration::from_millis(500)));
    с.write_all(format!("{просьба}\n").as_bytes()).ok()?;
    let mut ответ = String::new();
    let _ = с.read_to_string(&mut ответ);
    Some(ответ.trim().to_string())
}

pub fn на_стол(n: u64) {
    let всего = настройка("desks").as_u64().unwrap_or(2).clamp(1, 4);
    if n >= всего {
        return;
    }
    let f = format!("F{}", n + 1);
    if клавиши(&["-M", "logo", "-M", "ctrl", "-M", "alt", "-k", &f, "-m", "alt", "-m", "ctrl", "-m", "logo"]) {
        с_ядром(|я| я.стол = n);
        расскажи_скоро();
    }
}

/// Убрать рабочий стол. Окна на нём не теряются: переезжают на соседний
/// слева, а столы правее сдвигаются на место убранного
fn убери_стол(n: u64) {
    let всего = настройка("desks").as_u64().unwrap_or(2).clamp(1, 4);
    if всего <= 1 || n >= всего {
        return;
    }
    let куда = n.saturating_sub(1);
    let переезды: Vec<(String, u64)> = с_ядром(|я| {
        let стол = я.стол;
        я.окна
            .iter()
            .map(ключ_окна)
            .filter_map(|к| {
                let где = я.где_окно.get(&к).copied().unwrap_or(стол);
                if где == n {
                    Some((к, куда))
                } else if где > n {
                    Some((к, где - 1))
                } else {
                    None
                }
            })
            .collect()
    })
    .unwrap_or_default();
    // Отправлять окна можно только по одному и в фокусе — с паузами
    for (i, (к, на)) in переезды.into_iter().enumerate() {
        glib::timeout_add_local_once(Duration::from_millis(300 * i as u64), move || окно_на_стол(&к, на));
    }
    let текущий = с_ядром(|я| я.стол).unwrap_or(0);
    задай("desks", json!(всего - 1));
    if текущий >= n && текущий > 0 {
        на_стол(текущий - 1);
    }
}

fn ключ_окна(о: &Value) -> String {
    format!("{}\t{}", о["appId"].as_str().unwrap_or(""), о["title"].as_str().unwrap_or(""))
}

/// Стоит ли окно на текущем столе. Окна, которых ядро ещё не видело, —
/// здесь: они только что открылись
pub fn на_этом_столе(о: &Value) -> bool {
    с_ядром(|я| я.где_окно.get(&ключ_окна(о)).map_or(true, |n| *n == я.стол)).unwrap_or(true)
}

/// Окна текущего стола — их видят док, полоса и просмотр задач
pub fn окна_этого_стола() -> Vec<Value> {
    с_ядром(|я| {
        let стол = я.стол;
        я.окна.iter().filter(|о| я.где_окно.get(&ключ_окна(о)).copied().unwrap_or(стол) == стол).cloned().collect()
    })
    .unwrap_or_default()
}

fn окно_на_стол(id: &str, n: u64) {
    с_ядром(|я| {
        я.где_окно.insert(id.to_string(), n);
    });
    расскажи_скоро();
    let (app, title) = id.split_once('\t').unwrap_or((id, ""));
    let (app, title, f) = (app.to_string(), title.to_string(), format!("F{}", n + 1));
    // Отправить можно только окно в фокусе: сперва переходим к нему
    позови("sys.window", json!({ "action": "focus", "appId": app, "title": title }), move |_| {
        glib::timeout_add_local_once(Duration::from_millis(150), move || {
            клавиши(&["-M", "logo", "-M", "ctrl", "-M", "alt", "-M", "shift", "-k", &f, "-m", "shift", "-m", "alt", "-m", "ctrl", "-m", "logo"]);
        });
    });
}

/* ---------- просьбы ---------- */

fn переключи_в_списке(k: &str, id: &str) {
    let mut с: Vec<Value> = настройка(k).as_array().cloned().unwrap_or_default();
    if let Some(i) = с.iter().position(|х| х.as_str() == Some(id)) {
        с.remove(i);
    } else {
        с.push(json!(id));
    }
    задай(k, Value::Array(с));
}

fn обзор_дело(д: &Value) {
    let id = д["id"].as_str().unwrap_or("");
    let вид = д["вид"].as_str().unwrap_or("приложение");
    match д["что"].as_str().unwrap_or("") {
        "запусти" => запусти(id, вид),
        "стол" => на_стол(д["n"].as_u64().unwrap_or(0)),
        "стол-добавь" => {
            let n = (настройка("desks").as_u64().unwrap_or(2) + 1).min(4);
            задай("desks", json!(n));
        }
        "стол-убери" => убери_стол(д["n"].as_u64().unwrap_or(u64::MAX)),
        "окно" => {
            let (app, title) = id.split_once('\t').unwrap_or((id, ""));
            let свёрнуто = с_ядром(|я| я.окна.iter().any(|о| о["appId"].as_str() == Some(app) && о["title"].as_str() == Some(title) && о["состояние"]["свёрнуто"].as_bool() == Some(true))).unwrap_or(false);
            позови("sys.window", json!({ "action": if свёрнуто { "restore" } else { "focus" }, "appId": app, "title": title }), |_| {});
        }
        "окно-закрой" => {
            let (app, title) = id.split_once('\t').unwrap_or((id, ""));
            позови("sys.window", json!({ "action": "close", "appId": app, "title": title }), |_| {});
            glib::timeout_add_local_once(Duration::from_millis(400), опроси_окна);
        }
        "окно-на-стол" => окно_на_стол(id, д["n"].as_u64().unwrap_or(0)),
        "окно-на-новый-стол" => {
            let n = настройка("desks").as_u64().unwrap_or(2);
            if n < 4 {
                задай("desks", json!(n + 1));
                окно_на_стол(id, n);
            }
        }
        "на-стол" | "на-новый-стол" => {
            let n = if д["что"] == "на-стол" {
                д["n"].as_u64().unwrap_or(0)
            } else {
                let n = настройка("desks").as_u64().unwrap_or(2);
                if n >= 4 {
                    return;
                }
                задай("desks", json!(n + 1));
                n
            };
            на_стол(n);
            let (id, вид) = (id.to_string(), вид.to_string());
            glib::timeout_add_local_once(Duration::from_millis(250), move || запусти(&id, &вид));
        }
        "вынеси" => {
            if вид == "машина" {
                let mut с: Vec<Value> = настройка("deskNative").as_array().cloned().unwrap_or_default();
                if let Some(i) = с.iter().position(|п| п["id"].as_str() == Some(id)) {
                    с.remove(i);
                } else {
                    с.push(json!({ "id": id, "имя": д["имя"] }));
                }
                задай("deskNative", Value::Array(с));
            } else {
                переключи_в_списке("deskApps", id);
            }
        }
        "док" => переключи_в_списке("dockApps", id),
        "магазин" => {
            let имя = д["имя"].as_str().unwrap_or(id).to_string();
            запусти_команду("glower-store", &[&format!("--find={имя}")]);
        }
        "закрой-все" => {
            if let Some(н) = наша(id) {
                let окна = с_ядром(|я| я.окна.clone()).unwrap_or_default();
                for о in окна.iter().filter(|о| о["appId"].as_str() == Some(н.окно)) {
                    позови("sys.window", json!({ "action": "close", "appId": о["appId"], "title": о["title"] }), |_| {});
                }
            }
        }
        "очисти-корзину" => позови("fs.trash.empty", json!({}), |_| {}),
        "удали" => удали_программу(id, д["имя"].as_str().unwrap_or(id)),
        _ => {}
    }
    скажи("обзор-данные", данные_обзора(&с_ядром(|я| я.s.clone()).unwrap_or_default(), с_ядром(|я| я.стол).unwrap_or(0)));
}

fn мегабайты(б: f64) -> String {
    if б >= 1073741824.0 { format!("{:.1} ГБ", б / 1073741824.0).replace('.', ",") } else { format!("{} МБ", (б / 1048576.0).round().max(1.0)) }
}

/// Удалить программу машины. Агент сам выясняет, кто её хозяин и что уйдёт
/// вместе с ней, — человек видит это словами до того, как согласится
fn удали_программу(id: &str, имя: &str) {
    let (id, имя) = (id.to_string(), имя.to_string());
    позови("pkg.owner", json!({ "id": id }), move |о| {
        let о = match о {
            Ok(о) => о,
            Err(е) => return notify::покажи(&имя, &format!("Не удалось удалить: {е}"), "⚠️"),
        };
        if о["можно"].as_bool() != Some(true) {
            return notify::покажи(&format!("«{имя}» удалить нельзя"), о["почему"].as_str().unwrap_or(""), "🔒");
        }
        if о["разрешено"].as_bool() != Some(true) {
            return notify::покажи("Удаление выключено", "Системе не дали права ставить и удалять программы.", "🔒");
        }
        let пакет = о["пакет"].as_str().unwrap_or("").to_string();
        let (текст, кнопка) = match о["вид"].as_str().unwrap_or("") {
            "apt" => {
                let ещё = о["уйдёт"].as_array().map(|у| у.iter().filter(|п| п.as_str() != Some(&пакет)).count()).unwrap_or(0);
                let mut т = format!("Будет удалён пакет «{пакет}»{}.", if ещё > 0 { format!(" и ещё {ещё}, которые были нужны только ему") } else { String::new() });
                if let Some(б) = о["освободит"].as_f64() {
                    т.push_str(&format!("\nОсвободится около {}.", мегабайты(б)));
                }
                т.push_str("\n\nНастройки программы в вашей папке останутся.");
                (т, "Удалить")
            }
            "flatpak" => ("Программа из Flathub будет удалена. Её настройки в вашей папке останутся.".into(), "Удалить"),
            "windows" if о["ключ"].is_string() => ("Откроется окно удаления самой программы — как в Windows. Следуйте ему.".into(), "Открыть удаление"),
            "windows" => ("Эту программу ставили без установщика. Можно убрать её из меню — файлы останутся в папке Windows.".into(), "Убрать ярлык"),
            "appimage" => (format!("Программа одним файлом будет удалена вместе с ярлыком:\n{}", о["образ"].as_str().unwrap_or("")), "Удалить"),
            _ => ("За этим ярлыком нет пакета — будет удалён только сам ярлык.".into(), "Убрать ярлык"),
        };
        let д = gtk::MessageDialog::new(None::<&gtk::Window>, gtk::DialogFlags::MODAL, gtk::MessageType::Question, gtk::ButtonsType::None, &format!("Удалить «{имя}»?"));
        д.set_secondary_text(Some(&текст));
        д.add_button("Отмена", gtk::ResponseType::Cancel);
        д.add_button(кнопка, gtk::ResponseType::Accept);
        let ответ = д.run();
        unsafe { д.destroy() };
        if ответ != gtk::ResponseType::Accept {
            return;
        }
        let windows = о["вид"].as_str() == Some("windows");
        let (id2, имя2) = (id.clone(), имя.clone());
        позови("pkg.uninstall", json!({ "id": id }), move |о| match о {
            Err(е) => notify::покажи(&имя2, &format!("Не удалось удалить: {е}"), "⚠️"),
            Ok(о) if о["удалено"].as_bool() == Some(true) => удалено(&id2, &имя2),
            Ok(_) => следи_за_удалением(id2, имя2, windows),
        });
    });
}

fn удалено(id: &str, имя: &str) {
    // Программы больше нет — нет и её значков: на столе, в меню, в поиске
    let с: Vec<Value> = настройка("deskNative").as_array().cloned().unwrap_or_default().into_iter().filter(|п| п["id"].as_str() != Some(id)).collect();
    задай("deskNative", Value::Array(с));
    notify::покажи(имя, "Удалено", "🗑️");
    обнови_программы();
}

fn следи_за_удалением(id: String, имя: String, windows: bool) {
    let кончено = std::rc::Rc::new(Cell::new(false));
    glib::timeout_add_local(Duration::from_millis(if windows { 3000 } else { 1500 }), move || {
        if кончено.get() {
            return glib::ControlFlow::Break;
        }
        let (id2, имя2) = (id.clone(), имя.clone());
        let к2 = кончено.clone();
        let (метод, доводы) = if windows { ("pkg.uninstall.windows", json!({ "id": id2 })) } else { ("pkg.job", json!({})) };
        позови(метод, доводы, move |о| {
            let Ok(с) = о else { return };
            if с["идёт"].as_bool() == Some(true) || с["running"].as_bool() == Some(true) {
                return;
            }
            к2.set(true);
            if с["удалено"].as_bool() == Some(true) || с["ok"].as_bool() == Some(true) {
                удалено(&id2, &имя2);
            } else {
                let почему = [с["почему"].as_str(), с["error"].as_str()].into_iter().flatten().next().unwrap_or("система не сказала, что пошло не так").to_string();
                notify::покажи(&имя2, &format!("Не удалено: {почему}"), "⚠️");
            }
        });
        // Ответ придёт после этого шага; следующий шаг его увидит
        glib::ControlFlow::Continue
    });
}

fn параметры_дело(д: &Value) {
    let k = д["k"].as_str().unwrap_or("");
    match д["что"].as_str().unwrap_or("") {
        "set" | "kv" => задай(k, д["v"].clone()),
        "список" => переключи_в_списке(k, д["id"].as_str().unwrap_or("")),
        "акцент" => {
            с_ядром(|я| я.s["accentCustom"] = Value::Null);
            задай("accent", д["i"].clone());
        }
        "акцент-свой" => задай("accentCustom", д["цвет"].clone()),
        "виджет+" => {
            let mut с: Vec<Value> = настройка("deskWidgets").as_array().cloned().unwrap_or_default();
            с.push(json!({ "t": д["t"] }));
            задай("deskWidgets", Value::Array(с));
        }
        "виджет-" => {
            let mut с: Vec<Value> = настройка("deskWidgets").as_array().cloned().unwrap_or_default();
            let i = д["i"].as_u64().unwrap_or(u64::MAX) as usize;
            if i < с.len() {
                с.remove(i);
            }
            задай("deskWidgets", Value::Array(с));
        }
        "столы" => задай("desks", json!(д["n"].as_u64().unwrap_or(2).clamp(1, 4))),
        "уведомление" => notify::покажи("Тестовое уведомление", "Так выглядят уведомления GlowerOS", "🔔"),
        "фокус" => фокус(д["минут"].as_u64().unwrap_or(0)),
        "имя" => {
            if let Some(и) = д["имя"].as_str().filter(|и| !и.trim().is_empty()) {
                задай("userName", json!(и.trim()));
            }
            if let Some(з) = д["знак"].as_str() {
                задай("userEmoji", json!(з));
            }
        }
        "язык" => {
            if let Some((_, _, л)) = ЯЗЫКИ.iter().find(|(k, _, _)| Some(*k) == д["k"].as_str()) {
                let _ = л;
                позови("sys.locale.set", json!({ "язык": д["k"] }), |о| {
                    if let Err(е) = о {
                        notify::покажи("Язык", &format!("Не вышло сменить язык: {е}"), "⚠️");
                    } else {
                        notify::покажи("Язык", "Язык сменится со следующего входа в систему", "🗣");
                    }
                });
                расскажи_скоро();
            }
        }
        "раскладки" => {
            let список: Vec<String> = д["список"].as_array().map(|а| а.iter().filter_map(|х| х.as_str().map(String::from)).collect()).unwrap_or_default();
            let список2 = список.clone();
            позови("sys.kb.set", json!({ "раскладки": список }), move |о| {
                let (ошибка, сразу) = match &о {
                    Ok(в) => (String::new(), в["сразу"].as_bool() == Some(true)),
                    Err(е) => (е.clone(), false),
                };
                if о.is_ok() {
                    с_ядром(|я| я.раскладки.0 = список2.clone());
                    расскажи_скоро();
                }
                скажи("параметры-итог", json!({ "что": "раскладки", "ошибка": ошибка, "сразу": сразу }));
            });
        }
        "питание" => {
            let действие = д["action"].as_str().unwrap_or("").to_string();
            match действие.strip_prefix("update-") {
                Some(потом) => обнови_и(потом.to_string()),
                None => позови("sys.power", json!({ "action": действие }), |_| {}),
            }
        }
        "погода" => обнови_погоду(),
        "настройка" => crate::setup::покажи(),
        "сброс" => {
            let сохранить = ["setupDone", "userName", "iconPos", "deskApps", "deskNative"];
            let mut новые = умолчания();
            for к in сохранить {
                новые[к] = настройка(к);
            }
            с_ядром(|я| я.s = новые);
            сохрани();
            ночной_свет();
            расскажи_скоро();
        }
        _ => {}
    }
}

/// Фокусировка: «Не беспокоить» на заданное время
fn фокус(минут: u64) {
    if минут == 0 {
        с_ядром(|я| я.фокус_до = None);
        задай("dnd", json!(false));
        return;
    }
    let до = glib::real_time() / 1_000_000 + минут as i64 * 60;
    с_ядром(|я| я.фокус_до = Some(до));
    задай("dnd", json!(true));
    glib::timeout_add_local(Duration::from_secs(5), move || {
        let сейчас = glib::real_time() / 1_000_000;
        match с_ядром(|я| я.фокус_до).flatten() {
            Some(д) if д == до && сейчас >= д => {
                с_ядром(|я| я.фокус_до = None);
                задай("dnd", json!(false));
                notify::покажи("Фокусировка", "Сессия завершена", "🎯");
                glib::ControlFlow::Break
            }
            Some(д) if д == до => glib::ControlFlow::Continue,
            _ => glib::ControlFlow::Break,
        }
    });
}

/// Ночной свет — тёплые тона на весь день: wlsunset с одинаковым днём
fn ночной_свет() {
    let вкл = настройка("nightLight").as_bool() == Some(true);
    let _ = std::process::Command::new("pkill").args(["-x", "wlsunset"]).status();
    if вкл && !запусти_команду("wlsunset", &["-S", "00:00", "-s", "23:59", "-T", "4000", "-t", "3999"]) {
        notify::покажи("Ночной свет", "На машине нет wlsunset — тёплые тона включить нечем", "⚠️");
    }
}

/// Замок после N минут без дела. Бездействие видит оконный сервер, а не
/// мы: swayidle спрашивает его и сам зовёт замок, когда время вышло
fn авто_замок() {
    let _ = std::process::Command::new("pkill").args(["-x", "swayidle"]).status();
    let минут = настройка("autoLock").as_u64().or_else(|| настройка("autoLock").as_str().and_then(|т| т.parse().ok())).unwrap_or(0);
    if минут > 0 {
        запусти_команду("swayidle", &["-w", "timeout", &(минут * 60).to_string(), "glower-menu замок"]);
    }
}

/// Win+D: свернуть всё, второй раз — вернуть
fn покажи_стол() {
    let свёрнутые = с_ядром(|я| я.свёрнутые.clone()).unwrap_or_default();
    let окна = окна_этого_стола();
    let открытые: Vec<(String, String)> = окна
        .iter()
        .filter(|о| о["оболочка"].as_bool() != Some(true) && о["состояние"]["свёрнуто"].as_bool() != Some(true))
        .map(|о| (о["appId"].as_str().unwrap_or("").to_string(), о["title"].as_str().unwrap_or("").to_string()))
        .collect();
    if открытые.is_empty() {
        for (app, title) in &свёрнутые {
            позови("sys.window", json!({ "action": "restore", "appId": app, "title": title }), |_| {});
        }
        с_ядром(|я| я.свёрнутые.clear());
        return;
    }
    for (app, title) in &открытые {
        позови("sys.window", json!({ "action": "minimize", "appId": app, "title": title }), |_| {});
    }
    с_ядром(|я| я.свёрнутые = открытые);
}

/* ---------- опросы ---------- */

/// Ярлыки, которым не место в меню программ: наш Терминал — это и есть
/// foot, так что три его ярлыка (сам, клиент, сервер) были бы тремя
/// дублями; просмотрщик ImageMagick и консольный Vim без терминала не
/// нужны никому, кто ищет программу в меню
fn служебная(id: &str) -> bool {
    let id = id.trim_end_matches(".desktop");
    matches!(id, "org.codeberg.dnkl.foot" | "org.codeberg.dnkl.footclient" | "org.codeberg.dnkl.foot-server" | "foot" | "footclient" | "foot-server" | "vim" | "nvim" | "debian-xterm" | "debian-uxterm" | "xterm")
        || id.starts_with("display-im")
        || id.starts_with("python3")
}

pub fn обнови_программы() {
    позови("sys.apps", json!({}), |о| {
        if let Ok(о) = о {
            let список: Vec<Value> = о["list"].as_array().cloned().unwrap_or_default().into_iter().filter(|п| !служебная(п["id"].as_str().unwrap_or(""))).collect();
            с_ядром(|я| я.машина = список);
            расскажи_скоро();
        }
    });
    позови("install.can", json!({}), |о| {
        let можно = о.map(|о| о["allowed"].as_bool() == Some(true) && о["reason"].is_null()).unwrap_or(false);
        с_ядром(|я| я.можно_ставить = можно);
        расскажи_скоро();
    });
}

fn опроси_окна() {
    позови("sys.windows", json!({}), |о| {
        if let Ok(о) = о {
            // Свои диалоги панели (имя для папки и т. п.) — не программы
            let список: Vec<Value> = о["list"].as_array().cloned().unwrap_or_default().into_iter().filter(|w| w["appId"].as_str() != Some("glower-panel")).collect();
            let менялось = с_ядром(|я| {
                let было = std::mem::replace(&mut я.окна, список);
                let стол = я.стол;
                let есть: std::collections::HashSet<String> = я.окна.iter().map(ключ_окна).collect();
                я.где_окно.retain(|к, _| есть.contains(к));
                for к in есть {
                    я.где_окно.entry(к).or_insert(стол);
                }
                было != я.окна
            });
            if менялось == Some(true) {
                let (s, стол) = с_ядром(|я| (я.s.clone(), я.стол)).unwrap_or_default();
                скажи("обзор-данные", данные_обзора(&s, стол));
            }
        }
    });
}

fn проверь_обновления(читать: bool) {
    let (куда, откуда) = async_channel::bounded::<Option<Value>>(1);
    std::thread::spawn(move || {
        if читать && зов("pkg.update", json!({}), 30).is_ok() {
            for _ in 0..60 {
                std::thread::sleep(Duration::from_secs(3));
                match зов("pkg.job", json!({}), 10) {
                    Ok(j) if j["running"].as_bool() == Some(true) => continue,
                    _ => break,
                }
            }
        }
        let _ = куда.send_blocking(зов("pkg.upgrade.check", json!({}), 120).ok());
    });
    glib::MainContext::default().spawn_local(async move {
        let Ok(Some(д)) = откуда.recv().await else { return };
        let список = д["list"].as_array().cloned().unwrap_or_default();
        let наш = список.iter().find(|х| х["name"].as_str() == Some("glower")).cloned();
        let итог = json!({ "система": список.len(), "приложения": д["flathub"].as_array().map(|а| а.len()).unwrap_or(0), "наш": наш });
        let было = с_ядром(|я| std::mem::replace(&mut я.обновления, итог.clone())).unwrap_or_default();
        скажи("обновления", итог);
        if let Some(н) = &наш {
            if было["наш"]["станет"] != н["станет"] {
                notify::покажи(
                    "Новая версия системы",
                    &format!("GlowerOS {} установится при перезагрузке: меню питания → «Обновить и перезагрузить»", н["станет"].as_str().unwrap_or("")),
                    "✨",
                );
            }
        }
    });
}

fn опроси_носители() {
    позови("sys.drives", json!({}), |о| {
        let Ok(д) = о else { return };
        if д["есть"].as_bool() != Some(true) {
            return;
        }
        let список = д["list"].as_array().cloned().unwrap_or_default();
        let сейчас: Vec<String> = список.iter().filter_map(|н| н["dev"].as_str().map(String::from)).collect();
        let было = с_ядром(|я| я.носители.replace(сейчас.clone())).flatten();
        // При запуске не тревожим: флешка могла торчать и раньше
        let Some(было) = было else { return };
        for н in список.iter().filter(|н| !было.iter().any(|б| Some(б.as_str()) == н["dev"].as_str())) {
            let имя = [н["метка"].as_str(), н["модель"].as_str(), н["dev"].as_str()].into_iter().flatten().find(|с| !с.is_empty()).unwrap_or("Носитель").to_string();
            let dev = н["dev"].as_str().unwrap_or("").to_string();
            let гб = н["размер"].as_f64().unwrap_or(0.0) / 1073741824.0;
            notify::покажи_с_делом(
                "Носитель подключён",
                &format!("{имя} · {:.1} ГБ", гб).replace('.', ","),
                "💾",
                "Открыть",
                Box::new(move || {
                    позови("sys.drive", json!({ "action": "open", "dev": dev }), |о| {
                        if let Ok(о) = о {
                            if let Some(где) = о["где"].as_str() {
                                запусти_команду("glower-files", &[где]);
                            }
                        }
                    });
                }),
            );
        }
    });
}

fn опроси_заряд() {
    позови("sys.battery", json!({}), |о| {
        let Ok(б) = о else { return };
        let есть = б["present"].as_bool() == Some(true);
        let уровень = б["battery"]["level"].as_f64();
        let заряжается = б["charging"].as_bool() == Some(true);
        let сеть = б["ac"]["online"].as_bool() == Some(true);
        скажи("батарея", json!({ "есть": есть, "уровень": уровень, "заряжается": заряжается, "сеть": сеть }));
        if !есть || заряжается {
            с_ядром(|я| я.заряд_сказан = 0);
            return;
        }
        let Some(у) = уровень else { return };
        let порог = if у <= 5.0 { 5 } else if у <= 10.0 { 10 } else { 0 };
        let сказан = с_ядром(|я| я.заряд_сказан).unwrap_or(0);
        if порог > 0 && (сказан == 0 || порог < сказан) {
            с_ядром(|я| я.заряд_сказан = порог);
            notify::покажи(
                if порог <= 5 { "Заряд почти кончился" } else { "Низкий заряд" },
                &format!("Осталось {у:.0}% — подключите зарядку"),
                "🪫",
            );
        }
    });
}

/// Погоду спрашиваем при входе, когда сеть часто ещё не поднялась, — и
/// раньше на этом всё заканчивалось на полчаса: виджет писал «нужна сеть»
/// при работающем интернете. Теперь неудачу повторяем через полминуты,
/// пока не получится. Агент ходит в сеть дважды (город и прогноз), по
/// десять секунд на каждый, поэтому и ждём его дольше обычного.
pub fn обнови_погоду() {
    thread_local!(static ПОВТОР: std::cell::Cell<bool> = const { std::cell::Cell::new(false) });
    let город = настройка("city").as_str().unwrap_or("Москва").to_string();
    crate::agent::позови_за("weather", json!({ "город": город }), 30, |о| match о {
        Ok(п) => {
            с_ядром(|я| я.погода = п.clone());
            скажи("погода", п);
            crate::desktop::обнови_виджеты();
        }
        Err(е) => {
            eprintln!("glower-panel: погода: {е}");
            if !ПОВТОР.with(|п| п.replace(true)) {
                glib::timeout_add_seconds_local_once(30, || {
                    ПОВТОР.with(|п| п.set(false));
                    обнови_погоду();
                });
            }
        }
    });
}

pub fn погода() -> Value {
    с_ядром(|я| я.погода.clone()).unwrap_or(Value::Null)
}

fn смени_обои() {
    if настройка("wallShuffle").as_bool() != Some(true) {
        return;
    }
    let данные = с_ядром(|я| я.данные.clone()).unwrap_or_default();
    let обои = данные["обои"].as_array().cloned().unwrap_or_default();
    let сейчас = настройка("wallpaper");
    let i = обои.iter().position(|в| в["id"] == сейчас).map(|i| i + 1).unwrap_or(0) % обои.len().max(1);
    if let Some(в) = обои.get(i) {
        задай("wallpaper", в["id"].clone());
    }
}

/// «Обновить и перезагрузить»: поверх всего — просьба не выключать,
/// обновления ставятся, потом машина перезагружается или выключается
fn обнови_и(потом: String) {
    if !["reboot", "poweroff"].contains(&потом.as_str()) {
        return;
    }
    let занавес = crate::menus::занавес("Ставлю обновления. Не выключайте компьютер…");
    позови("pkg.upgrade.run", json!({}), move |о| {
        if let Err(е) = о {
            unsafe { занавес.destroy() };
            notify::покажи("Обновления не поставились", &е, "⚠️");
            return;
        }
        let кончено = std::rc::Rc::new(Cell::new(false));
        glib::timeout_add_local(Duration::from_secs(2), move || {
            if кончено.get() {
                return glib::ControlFlow::Break;
            }
            let (кончено2, занавес2, потом2) = (кончено.clone(), занавес.clone(), потом.clone());
            позови("pkg.job", json!({}), move |о| {
                let Ok(ход) = о else { return };
                if ход["running"].as_bool() == Some(true) || кончено2.get() {
                    return;
                }
                кончено2.set(true);
                if ход["ok"].as_bool() == Some(false) && ход["error"].is_string() {
                    unsafe { занавес2.destroy() };
                    notify::покажи("Обновления не поставились", ход["error"].as_str().unwrap_or(""), "⚠️");
                    return;
                }
                скажи("питание", json!(потом2));
                позови("sys.power", json!({ "action": потом2 }), |_| {});
            });
            glib::ControlFlow::Continue
        });
    });
}

/* ---------- напоминания: календарь и будильники ----------

   Календарь и Часы — отдельные программы, их можно закрыть. Звонит
   поэтому рабочий стол: раз в двадцать секунд смотрит в их файлы. */

thread_local! {
    static СЛЕЖКА_ДЕЛ: RefCell<Vec<gio::FileMonitor>> = const { RefCell::new(Vec::new()) };
}

fn следи_за_делами() {
    for имя in [crate::todo::ФАЙЛ, crate::calendar::ФАЙЛ] {
        let ф = gio::File::for_path(crate::apps::файл_данных(имя));
        if let Ok(м) = ф.monitor_file(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE) {
            м.connect_changed(|_, _, _, _| crate::desktop::обнови_виджеты());
            СЛЕЖКА_ДЕЛ.with(|с| с.borrow_mut().push(м));
        }
    }
    glib::timeout_add_local(Duration::from_secs(20), || {
        напомни();
        glib::ControlFlow::Continue
    });
    // В полночь — новый день на виджетах
    glib::timeout_add_local(Duration::from_secs(60), || {
        crate::desktop::обнови_виджеты();
        glib::ControlFlow::Continue
    });
}

/// Сработавшее ли сейчас: срок прошёл, но не больше десяти минут назад
pub fn пора(срок: &str, сейчас_мин: i64) -> bool {
    let mut ч = срок.split(':');
    match (ч.next().and_then(|х| х.parse::<i64>().ok()), ч.next().and_then(|х| х.parse::<i64>().ok())) {
        (Some(h), Some(m)) => {
            let м = h * 60 + m;
            сейчас_мин >= м && сейчас_мин - м < 10
        }
        _ => false,
    }
}

fn напомни() {
    let Ok(н) = glib::DateTime::now_local() else { return };
    let мин = (н.hour() * 60 + н.minute()) as i64;
    let сегодня = crate::calendar::ключ(н.year(), н.month() as u32, н.day_of_month() as u32);

    // События календаря со временем — в срок или заранее (remind — за
    // сколько минут, −1 — не напоминать), с повторами. Напомнить «за день»
    // значит напомнить сегодня о завтрашнем, поэтому смотрим оба дня.
    // Отметка «done_on» — день, о котором уже напомнили: у повторяющегося
    // события их много, и каждый раз — свой
    let mut кал = crate::apps::читай_данные(crate::calendar::ФАЙЛ);
    let mut менял = false;
    let сейчас = н.to_unix() / 60;
    let завтра = н.add_days(1).map(|з| crate::calendar::ключ(з.year(), з.month() as u32, з.day_of_month() as u32)).unwrap_or_default();
    let _ = мин;
    if let Some(дни) = кал.as_object_mut() {
        for (начало, список) in дни.iter_mut() {
            let Some(список) = список.as_array_mut() else { continue };
            for е in список.iter_mut() {
                let срок = е["time"].as_str().unwrap_or("").to_string();
                let за = е["remind"].as_i64().unwrap_or(0);
                if !crate::calendar::время_годно(&срок) || за < 0 {
                    continue;
                }
                for день in [&сегодня, &завтра] {
                    if !crate::calendar::бывает(начало, е, день) {
                        continue;
                    }
                    // Старый формат: «done» без дня — значит, о дне начала
                    if е["done_on"].as_str() == Some(день.as_str()) || (е["done"].as_bool() == Some(true) && день == начало && е["done_on"].is_null()) {
                        continue;
                    }
                    let Some((г, м, д)) = crate::calendar::разбери(день) else { continue };
                    let (ч, мн) = срок.split_once(':').map(|(ч, м)| (ч.parse().unwrap_or(0), м.parse().unwrap_or(0))).unwrap_or((0, 0));
                    let Ok(когда) = glib::DateTime::from_local(г, м as i32, д as i32, ч, мн, 0.0) else { continue };
                    let в_срок = когда.to_unix() / 60;
                    let момент = в_срок - за;
                    if сейчас >= момент && сейчас - момент < 10 && сейчас <= в_срок + 10 {
                        е["done_on"] = json!(день.as_str());
                        менял = true;
                        let текст = match в_срок - сейчас {
                            о if о >= 1440 - 10 && за >= 1440 => format!("Завтра в {срок}"),
                            о if о >= 1 => format!("Через {о} мин, в {срок}"),
                            _ => format!("Событие в {срок}"),
                        };
                        notify::из_шины(&json!({ "заголовок": е["t"], "текст": текст, "значок": "📅", "откуда": "Календарь", "звук": true }));
                    }
                }
            }
        }
    }
    if менял {
        crate::apps::пиши_данные(crate::calendar::ФАЙЛ, &кал);
    }

    // Будильники: в свой день недели (0 — понедельник) или каждый день
    let mut часы = crate::apps::читай_данные(crate::clock::ФАЙЛ);
    let день = (н.day_of_week() - 1) as u64;
    let метка = format!("{сегодня} {:02}:{:02}", н.hour(), н.minute());
    let mut менял = false;
    let mut звенят = Vec::new();
    if let Some(список) = часы["alarms"].as_array_mut() {
        for б in список.iter_mut() {
            let дни: Vec<u64> = б["days"].as_array().map(|а| а.iter().filter_map(|х| х.as_u64()).collect()).unwrap_or_default();
            let срок = б["time"].as_str().unwrap_or("").to_string();
            let уже = б["last"].as_str().is_some_and(|п| п.starts_with(&сегодня));
            if б["on"].as_bool() != Some(false) && !уже && (дни.is_empty() || дни.contains(&день)) && пора(&срок, мин) {
                б["last"] = json!(метка);
                // Разовый (без дней недели) — после звонка выключается
                if дни.is_empty() {
                    б["on"] = json!(false);
                }
                менял = true;
                звенят.push((срок, б["label"].as_str().unwrap_or("").to_string()));
            }
        }
    }
    if менял {
        crate::apps::пиши_данные(crate::clock::ФАЙЛ, &часы);
    }
    for (срок, подпись) in звенят {
        notify::звук();
        let заголовок = if подпись.is_empty() { "Будильник".to_string() } else { подпись.clone() };
        notify::покажи_с_делом(&заголовок, &срок, "⏰", "Отложить на 10 минут", Box::new(move || {
            let п = подпись.clone();
            glib::timeout_add_local_once(Duration::from_secs(600), move || {
                notify::звук();
                notify::покажи(if п.is_empty() { "Будильник" } else { &п }, "Отложенный звонок", "⏰");
            });
        }));
    }
}

/* ---------- установка из файла и программы Windows ---------- */

fn спроси(заголовок: &str, текст: &str, да: &str) -> bool {
    // Без значка «?»: вопрос и так понятен из слов
    let д = gtk::MessageDialog::new(None::<&gtk::Window>, gtk::DialogFlags::MODAL, gtk::MessageType::Other, gtk::ButtonsType::None, "");
    д.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(заголовок)));
    if !текст.is_empty() {
        д.set_secondary_text(Some(текст));
    }
    д.add_button("Отмена", gtk::ResponseType::Cancel);
    д.add_button(да, gtk::ResponseType::Accept);
    д.set_default_response(gtk::ResponseType::Accept);
    let ответ = д.run();
    unsafe { д.destroy() };
    ответ == gtk::ResponseType::Accept
}

fn имя_файла(путь: &str) -> String {
    путь.rsplit('/').next().unwrap_or(путь).to_string()
}

/// Скачанный .deb, .flatpakref или AppImage открыли двойным щелчком —
/// спрашиваем и ставим
fn спроси_про_установку(путь: String) {
    if путь.to_lowercase().ends_with(".appimage") {
        let имя = имя_файла(&путь);
        if спроси(&format!("Запустить «{имя}»?"), "Это программа одним файлом: она ничего не ставит в систему, а просто запускается. За то, что внутри, отвечает тот, кто её сделал.", "Запустить") {
            позови("sys.appimage", json!({ "путь": путь }), move |о| {
                if let Err(е) = о {
                    notify::покажи(&имя, &format!("Не запустилась: {е}"), "⚠️");
                }
            });
        }
        return;
    }
    позови("pkg.file.info", json!({ "путь": путь }), move |о| {
        let про = match о {
            Ok(п) => п,
            Err(е) => return notify::покажи("Не удалось прочитать файл", &е, "⚠️"),
        };
        let имя = про["название"].as_str().filter(|н| !н.is_empty()).or(про["имя"].as_str()).unwrap_or("программу").to_string();
        // Только вопрос и две кнопки — как в Windows: что за программа, видно
        // по имени, а описание из пакета никто не читает
        if !спроси(&format!("Установить «{имя}»?"), "", "Установить") {
            return;
        }
        позови("pkg.file.install", json!({ "путь": путь }), move |о| match о {
            Err(е) => notify::покажи("Не удалось начать установку", &е, "⚠️"),
            Ok(_) => {
                notify::покажи("Установка", &format!("Устанавливаю «{имя}»"), "📦");
                следи_за_установкой(имя, None);
            }
        });
    });
}

/// Ход установки: ждём конца работы агента и говорим, чем кончилось
fn следи_за_установкой(имя: String, потом: Option<Box<dyn Fn()>>) {
    let потом = std::rc::Rc::new(потом);
    let ждём = std::rc::Rc::new(Cell::new(false));
    let кончено = std::rc::Rc::new(Cell::new(false));
    glib::timeout_add_local(Duration::from_millis(1500), move || {
        if кончено.get() {
            return glib::ControlFlow::Break;
        }
        if ждём.replace(true) {
            return glib::ControlFlow::Continue;
        }
        let (имя, потом, ждём, кончено) = (имя.clone(), потом.clone(), ждём.clone(), кончено.clone());
        позови("pkg.job", json!({}), move |о| {
            ждём.set(false);
            let Ok(ход) = о else { return };
            if ход["running"].as_bool() == Some(true) {
                return;
            }
            кончено.set(true);
            if ход["ok"].as_bool() == Some(true) {
                notify::покажи(&имя, "Установлено", "✅");
                обнови_программы();
                if let Some(п) = потом.as_ref() {
                    п();
                }
            } else if ход["error"].as_str() != Some("работа остановлена") {
                notify::покажи(&format!("{имя}: установка не удалась"), ход["error"].as_str().unwrap_or("система не сказала, что пошло не так"), "⚠️");
            }
        });
        glib::ControlFlow::Continue
    });
}

/// Вести о программах Windows: «запускаю» и «нет Wine — поставить?»
fn программа_windows(что: &Value) {
    let путь = что["путь"].as_str().unwrap_or("").to_string();
    let имя = имя_файла(&путь);
    match что["событие"].as_str() {
        Some("запуск") => notify::покажи(&имя, "Запускаю программу Windows", "🪟"),
        Some("нет-wine") => {
            if спроси(
                "Установить поддержку программ Windows?",
                &format!("Чтобы открыть «{имя}», нужен Wine — прослойка, на которой в Linux работают программы для Windows. Он займёт около 700 МБ и поставится один раз. Работает многое, но не всё: тяжёлым программам и играм с защитой от читов Wine часто не подходит."),
                "Установить",
            ) {
                позови("pkg.windows.install", json!({}), move |о| match о {
                    Err(е) => notify::покажи("Не удалось начать установку", &е, "⚠️"),
                    Ok(_) => следи_за_установкой(
                        "Поддержка программ Windows".into(),
                        Some(Box::new(move || позови("sys.windows.run", json!({ "путь": путь }), |_| {}))),
                    ),
                });
            }
        }
        _ => {}
    }
}

/* ---------- начало ---------- */

pub fn начни() {
    let данные: Value = serde_json::from_str(ДАННЫЕ).unwrap_or_default();
    Я.with(|я| *я.borrow_mut() = Some(Ядро { s: загрузи(), данные, ..Default::default() }));

    // Видеокарта без своего драйвера — напоминаем, как Windows: раз за сеанс,
    // через минуту после входа, чтобы не мешать первым шагам
    glib::timeout_add_local_once(std::time::Duration::from_secs(60), || {
        позови("sys.drivers", json!({}), |о| {
            let Ok(д) = о else { return };
            let стоит: Vec<&str> = д["стоит"].as_array().map(|а| а.iter().filter_map(|х| х.as_str()).collect()).unwrap_or_default();
            for у in д["устройства"].as_array().into_iter().flatten() {
                let варианты = у["драйверы"].as_array().cloned().unwrap_or_default();
                let нужен = варианты.iter().any(|в| в["рекомендован"].as_bool() == Some(true) && в["свободный"].as_bool() != Some(true));
                let уже = варианты.iter().any(|в| в["пакет"].as_str().is_some_and(|п| стоит.contains(&п)));
                if нужен && !уже {
                    notify::покажи(
                        "Есть драйвер для видеокарты",
                        &format!("{} будет работать быстрее. Поставить: Параметры → Обновления", у["модель"].as_str().unwrap_or("Видеокарта")),
                        "🎮",
                    );
                    break;
                }
            }
        });
    });

    // Слушаем просьбы всех, кто прежде просил страницу
    let вести = слушай(
        &[],
        &[
            "обзор-дело", "обзор-дай", "значки-дело", "параметры-дело", "дай-док", "открой", "настройка", "клавиша", "стол",
            "пробуждение", "уведомление", "уведомление-закрыть", "файл-установщик", "windows", "снимок", "покажи", "медиа", "питание",
        ],
    );
    glib::MainContext::default().spawn_local(async move {
        while let Ok(в) = вести.recv().await {
            let что = &в.что;
            match в.тема.as_str() {
                "обзор-дело" => обзор_дело(что),
                "обзор-дай" => {
                    let (s, стол) = с_ядром(|я| (я.s.clone(), я.стол)).unwrap_or_default();
                    скажи("обзор-данные", данные_обзора(&s, стол));
                }
                "значки-дело" => crate::desktop::дело_ядра(что),
                "параметры-дело" => параметры_дело(что),
                "дай-док" => расскажи_скоро(),
                "открой" => {
                    let вид = что["вид"].as_str().unwrap_or("приложение");
                    let id = что["id"].as_str().unwrap_or("");
                    if id == "settings" {
                        if let Some(раздел) = что["довод"]["section"].as_str() {
                            открой_параметры(раздел);
                            continue;
                        }
                    }
                    запусти(id, вид);
                }
                "настройка" => {
                    if let Some(к) = что["ключ"].as_str().filter(|к| ["dnd", "nightLight"].contains(к)) {
                        задай(к, что["что"].clone());
                    }
                }
                "клавиша" => {
                    match что.as_str() {
                        Some("W-d") => покажи_стол(),
                        Some("W-v") => crate::cliphist::переключи(),
                        _ => {}
                    }
                }
                "стол" => {
                    let текущий = с_ядром(|я| я.стол).unwrap_or(0);
                    let всего = настройка("desks").as_u64().unwrap_or(2);
                    let куда = match что.as_str() {
                        Some("вперёд") => текущий + 1,
                        Some("назад") => текущий.saturating_sub(1),
                        Some(н) => н.parse::<u64>().unwrap_or(1).saturating_sub(1),
                        None => continue,
                    };
                    if куда < всего && куда != текущий {
                        на_стол(куда);
                    }
                }
                "покажи" if что.as_str() == Some("раскладка") => открой_параметры("time"),
                "питание" => {
                    if let Some(потом) = что.as_str().and_then(|д| д.strip_prefix("update-")) {
                        обнови_и(потом.to_string());
                    }
                }
                // Клавиши мультимедиа: оконный сервер зовёт glower-menu медиа …
                "медиа" => {
                    if let Some(м) = что.as_str().filter(|м| ["PlayPause", "Next", "Previous", "Stop"].contains(м)) {
                        let м = м.to_string();
                        std::thread::spawn(move || crate::media::нажми(&м));
                    }
                }
                "пробуждение" => {
                    if настройка("lockOnWake").as_bool() != Some(false) {
                        позови("sys.power", json!({ "action": "lock" }), |_| {});
                    }
                }
                "уведомление" => notify::из_шины(что),
                "уведомление-закрыть" => notify::закрой_номер(что["номер"].as_u64().unwrap_or(0) as u32),
                // Оба спрашивают окном — не изнутри задачи (см. apps::вне_задачи)
                "файл-установщик" => {
                    if let Some(путь) = что["путь"].as_str().map(String::from) {
                        crate::apps::вне_задачи(move || спроси_про_установку(путь));
                    }
                }
                "windows" => {
                    let что = что.clone();
                    crate::apps::вне_задачи(move || программа_windows(&что));
                }
                "снимок" => {
                    let режим = что.as_str().unwrap_or("экран").to_string();
                    запусти_команду("glower-snip", &[&режим]);
                }
                _ => {}
            }
        }
    });

    // Раскладки системы — их знает агент
    позови("sys.kb.list", json!({}), |о| {
        if let Ok(о) = о {
            let выбрано: Vec<String> = о["выбрано"].as_array().map(|а| а.iter().filter_map(|х| х.as_str().map(String::from)).collect()).unwrap_or_default();
            let выбрано = if выбрано.is_empty() { vec!["us".to_string(), "ru".to_string()] } else { выбрано };
            let все = о["список"].as_array().cloned().unwrap_or_default();
            с_ядром(|я| я.раскладки = (выбрано, все));
            расскажи_скоро();
        }
    });

    // Стол следит за своей папкой: новый файл — новый значок
    if let Ok(н) = gio::File::for_path(папка_стола()).monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
        н.connect_changed(|_, _, _, _| расскажи_скоро());
        // Наблюдатель живёт, пока живёт программа
        std::mem::forget(н);
    }

    обнови_программы();
    опроси_окна();
    опроси_заряд();
    обнови_погоду();
    ночной_свет();
    расскажи_всё();
    glib::timeout_add_local(Duration::from_secs(2), || {
        опроси_окна();
        glib::ControlFlow::Continue
    });
    glib::timeout_add_local(Duration::from_secs(5), || {
        опроси_носители();
        glib::ControlFlow::Continue
    });
    glib::timeout_add_local(Duration::from_secs(30), || {
        опроси_заряд();
        смени_обои();
        glib::ControlFlow::Continue
    });
    glib::timeout_add_local(Duration::from_secs(1800), || {
        обнови_погоду();
        glib::ControlFlow::Continue
    });
    glib::timeout_add_local(Duration::from_secs(120), || {
        обнови_программы();
        glib::ControlFlow::Continue
    });
    // Обновления: раз в шесть часов читаем списки, а в каждом сеансе
    // хотя бы спрашиваем, что уже известно
    glib::timeout_add_local_once(Duration::from_secs(40), || {
        let сейчас = glib::real_time() / 1_000_000;
        let давно = сейчас - настройка("updatesAsked").as_i64().unwrap_or(0) >= 6 * 3600;
        if давно {
            задай("updatesAsked", json!(сейчас));
        }
        проверь_обновления(давно);
    });
    glib::timeout_add_local(Duration::from_secs(6 * 3600), || {
        задай("updatesAsked", json!(glib::real_time() / 1_000_000));
        проверь_обновления(true);
        glib::ControlFlow::Continue
    });

    авто_замок();
    следи_за_делами();

    // Установочная среда: мастер установки во весь экран. Обычно сеанс
    // запускает его сам, раньше панели (GLOWER_INSTALLER_SHOWN), и панель
    // поднимается только для пробной системы — тогда второй раз его не
    // открываем. Мастер первого запуска здесь не нужен — его пройдут на
    // установленной системе
    let ядро = std::fs::read_to_string("/proc/cmdline").unwrap_or_default();
    if ядро.split_whitespace().any(|ч| ч == "glower.install=1" || ч == "glower.repair=1") {
        if std::env::var_os("GLOWER_INSTALLER_SHOWN").is_none() && !std::path::Path::new("/run/glower-try").exists() {
            запусти_команду("glower-installer", &["--install"]);
        }
    } else if настройка("setupDone").as_bool() != Some(true) {
        // Первый вход — мастер, а не стол
        crate::setup::покажи();
    } else {
        // Автозапуск — один раз за сеанс
        for id in настройка("autostart").as_array().cloned().unwrap_or_default() {
            if let Some(id) = id.as_str() {
                запусти(id, "приложение");
            }
        }
    }
}

#[cfg(test)]
mod проверки {
    use super::*;

    #[test]
    fn служебные_не_в_меню() {
        assert!(служебная("org.codeberg.dnkl.footclient.desktop"));
        assert!(служебная("display-im6.q16.desktop"));
        assert!(!служебная("firefox.desktop"));
        assert!(!служебная("libreoffice-writer.desktop"));
    }

    #[test]
    fn тип_файла_по_имени_не_нулевой() {
        // Раньше любой файл выходил application/x-zerosize, и стол не
        // открывал картинки своими Фото
        assert_eq!(тип_по_имени("Рисунок.png"), "image/png");
        assert!(gio::content_type_is_a(&тип_по_имени("заметка.txt"), "text/plain"));
        let п = std::env::temp_dir().join(format!("glower-tip-{}.png", std::process::id()));
        std::fs::write(&п, b"\x89PNG\r\n\x1a\n").unwrap();
        assert_eq!(тип_файла(&п), "image/png");
        let _ = std::fs::remove_file(&п);
    }

    #[test]
    fn служебные_сочетания_словами() {
        assert_eq!(просьба_серверу(&["-M", "logo", "-M", "ctrl", "-M", "alt", "-k", "F2", "-m", "alt"]).as_deref(), Some("desk 2"));
        assert_eq!(просьба_серверу(&["-M", "logo", "-M", "shift", "-k", "F4"]).as_deref(), Some("send 4"));
        assert_eq!(просьба_серверу(&["-k", "Return"]), None);
    }

    #[test]
    fn напоминание_в_свой_срок() {
        assert!(пора("09:30", 9 * 60 + 30));
        assert!(пора("09:30", 9 * 60 + 35));
        assert!(!пора("09:30", 9 * 60 + 29));
        assert!(!пора("09:30", 9 * 60 + 41));
        assert!(!пора("", 0));
    }

    #[test]
    fn данные_оболочки_читаются() {
        let д: Value = serde_json::from_str(ДАННЫЕ).unwrap();
        assert_eq!(д["обои"].as_array().unwrap().len(), 10);
        assert_eq!(д["акценты"].as_array().unwrap().len(), 10);
        // Картинка из assets, а без неё — запасной градиент
        assert!(обои_css("dyuny", &д).contains("dyuny.jpg") || обои_css("dyuny", &д).contains("gradient"));
        assert_eq!(обои_css("файл:/x/кот.png", &д), "url(\"file:///x/кот.png\")");
        // Неизвестные обои — первые, а не пустота
        assert!(обои_css("нет-таких", &д).contains("gradient"));
    }

    #[test]
    fn в_доке_только_свои_программы() {
        for id in умолчания()["dockApps"].as_array().unwrap() {
            assert!(наша(id.as_str().unwrap()).is_some(), "{id}");
        }
    }

    #[test]
    fn знаки_файлов() {
        assert_eq!(знак_файла("image/png"), "🖼️");
        assert_eq!(знак_файла("application/zip"), "🗜️");
        assert_eq!(знак_файла("text/plain"), "📄");
    }
}
