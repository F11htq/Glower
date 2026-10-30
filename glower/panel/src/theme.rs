//! Тема и цвет акцента — общие для всех программ системы.
//!
//! Стили программ пишут не цвета, а их имена: @glower_bg, @glower_fg,
//! @glower_accent… Значения имён задаёт этот модуль, отдельным источником
//! стиля на весь экран: GTK ищет именованный цвет во всех источниках
//! разом. Так тёмная тема меняется на светлую и синий акцент на зелёный
//! во всех окнах сразу, без перезапуска — каждая программа следит за
//! файлом настроек и перекрашивается, как только он изменился.

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use serde_json::Value;
use std::cell::RefCell;

const ДАННЫЕ: &str = include_str!("shell-data.json");

thread_local! {
    static ИСТОЧНИК: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
    static СЛЕЖКА: RefCell<Option<gio::FileMonitor>> = const { RefCell::new(None) };
    static БЫЛО: RefCell<String> = const { RefCell::new(String::new()) };
    static ПЛОТНО: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
    static ПРОЗРАЧНО: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// Без прозрачности: панель, меню, док, виджеты и уведомления — плотные.
/// Кому-то полупрозрачное мешает читать, а на слабой видеокарте размытие
/// под ним ещё и стоит кадров
const ПЛОТНЫЙ: &str = r#"
window.bar { background-color: rgb(20, 22, 30); }
.menu, .toast { background-color: @glower_bg; }
.widget { background-color: rgb(22, 24, 34); }
.overview button.task { background-color: rgb(30, 33, 46); }
"#;

/// Включена ли прозрачность оболочки (по умолчанию — да). Док и меню
/// программ рисуют фон сами и спрашивают здесь
pub fn прозрачно() -> bool {
    ПРОЗРАЧНО.with(|п| п.get())
}

fn примени_прозрачность(s: &Value) {
    let да = s["transparency"].as_bool() != Some(false);
    if ПРОЗРАЧНО.with(|п| п.replace(да)) == да && (да || ПЛОТНО.with(|п| п.borrow().is_some())) {
        return;
    }
    let Some(э) = gtk::gdk::Screen::default() else { return };
    ПЛОТНО.with(|п| {
        let mut п = п.borrow_mut();
        if let Some(старый) = п.take() {
            gtk::StyleContext::remove_provider_for_screen(&э, &старый);
        }
        if !да {
            let новый = gtk::CssProvider::new();
            if новый.load_from_data(ПЛОТНЫЙ.as_bytes()).is_ok() {
                gtk::StyleContext::add_provider_for_screen(&э, &новый, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 3);
                *п = Some(новый);
            }
        }
    });
    // Фон, нарисованный руками (док, меню программ), — перерисовать
    for о in gtk::Window::list_toplevels() {
        о.queue_draw();
    }
}

fn путь_настроек() -> std::path::PathBuf {
    glib::user_config_dir().join("glower").join("settings.json")
}

fn настройки() -> Value {
    std::fs::read_to_string(путь_настроек()).ok().and_then(|т| serde_json::from_str(&т).ok()).unwrap_or_default()
}

/// Тёмная ли сейчас тема (по умолчанию — да)
pub fn тёмная() -> bool {
    настройки()["theme"].as_str() != Some("light")
}

/// Два цвета акцента: свой цвет человека или один из готовых
pub fn акцент(s: &Value) -> (String, String) {
    if let Some(свой) = s["accentCustom"].as_str().filter(|с| с.starts_with('#') && с.len() == 7) {
        return (свой.to_string(), свой.to_string());
    }
    let данные: Value = serde_json::from_str(ДАННЫЕ).unwrap_or_default();
    let i = s["accent"].as_u64().unwrap_or(0) as usize;
    данные["акценты"]
        .as_array()
        .and_then(|а| а.get(i).or(а.first()))
        .map(|а| (а["a"].as_str().unwrap_or("#3a86ff").to_string(), а["b"].as_str().unwrap_or("#8b5cf6").to_string()))
        .unwrap_or(("#3a86ff".into(), "#8b5cf6".into()))
}

/// Именованные цвета для выбранной темы и акцента
pub fn цвета(светлая: bool, акцент: &(String, String)) -> String {
    let (фон, фон2, фон3, текст, линия) = if светлая {
        ("#f4f5f9", "#ffffff", "#e9ebf2", "#1b1e27", "rgba(0,0,0,0.10)")
    } else {
        ("#161821", "#12141b", "#1b1e28", "#e8eaf2", "rgba(255,255,255,0.08)")
    };
    format!(
        "@define-color glower_bg {фон};\n@define-color glower_bg2 {фон2};\n@define-color glower_bg3 {фон3};\n\
         @define-color glower_fg {текст};\n@define-color glower_line {линия};\n\
         @define-color glower_accent {};\n@define-color glower_accent2 {};\n",
        акцент.0, акцент.1
    )
}

fn примени() {
    let s = настройки();
    примени_прозрачность(&s);
    let светлая = s["theme"].as_str() == Some("light");
    let css = цвета(светлая, &акцент(&s));
    if БЫЛО.with(|б| *б.borrow() == css) {
        return;
    }
    БЫЛО.with(|б| *б.borrow_mut() = css.clone());
    if let Some(н) = gtk::Settings::default() {
        н.set_gtk_application_prefer_dark_theme(!светлая);
    }
    ИСТОЧНИК.with(|и| {
        let mut и = и.borrow_mut();
        let э = gtk::gdk::Screen::default();
        if let (Some(старый), Some(э)) = (и.take(), э.as_ref()) {
            gtk::StyleContext::remove_provider_for_screen(э, &старый);
        }
        let новый = gtk::CssProvider::new();
        if новый.load_from_data(css.as_bytes()).is_ok() {
            if let Some(э) = э {
                gtk::StyleContext::add_provider_for_screen(&э, &новый, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
            }
            *и = Some(новый);
        }
    });
}

/// Сказать о теме чужим программам — всем, кто её спрашивает по-своему.
///
/// Свои программы перекрашиваются по файлу настроек, а чужие о нём не
/// знают. Каждая спрашивает своё место:
///   — libadwaita, GTK4, а через портал рабочего стола Firefox, Chrome,
///     Electron (Discord, VS Code, Telegram) и всё из Flathub — ключ
///     color-scheme в org.gnome.desktop.interface;
///   — старые программы GTK3 — gtk-theme (Adwaita или Adwaita-dark);
///   — GTK, запущенный без gsettings, — settings.ini;
///   — Qt через свой модуль gtk3 — тоже gtk-theme.
/// Пишет это только ядро панели, раз при запуске и при каждой смене темы.
pub fn сообщи_программам(светлая: bool) {
    let схема = if светлая { "prefer-light" } else { "prefer-dark" };
    let тема = if светлая { "Adwaita" } else { "Adwaita-dark" };
    if let Some(источник) = gio::SettingsSchemaSource::default() {
        if let Some(схема_) = источник.lookup("org.gnome.desktop.interface", true) {
            let н = gio::Settings::new("org.gnome.desktop.interface");
            if схема_.has_key("color-scheme") {
                // prefer-light есть не во всех версиях: где его нет — default
                if н.set_string("color-scheme", схема).is_err() && светлая {
                    let _ = н.set_string("color-scheme", "default");
                }
            }
            if схема_.has_key("gtk-theme") {
                let _ = н.set_string("gtk-theme", тема);
            }
            gio::Settings::sync();
        }
    }
    for папка in ["gtk-3.0", "gtk-4.0"] {
        let файл = glib::user_config_dir().join(папка).join("settings.ini");
        let было = std::fs::read_to_string(&файл).unwrap_or_default();
        let стало = settings_ini(&было, !светлая);
        if стало != было {
            let _ = std::fs::create_dir_all(файл.parent().unwrap_or(std::path::Path::new("/")));
            let _ = std::fs::write(&файл, стало);
        }
    }
}

/// settings.ini с нужным gtk-application-prefer-dark-theme; прочее не трогаем
pub fn settings_ini(было: &str, тёмная: bool) -> String {
    let строка = format!("gtk-application-prefer-dark-theme={}", if тёмная { 1 } else { 0 });
    let mut строки: Vec<String> = было.lines().filter(|с| !с.trim_start().starts_with("gtk-application-prefer-dark-theme")).map(String::from).collect();
    match строки.iter().position(|с| с.trim() == "[Settings]") {
        Some(i) => строки.insert(i + 1, строка),
        None => {
            if !строки.is_empty() && !строки.last().is_some_and(|с| с.is_empty()) {
                строки.push(String::new());
            }
            строки.push("[Settings]".into());
            строки.push(строка);
        }
    }
    строки.join("\n") + "\n"
}

/// Подключить тему и следить за ней. Звать после gtk::init и до того,
/// как программа загрузит свой стиль: имена цветов должны быть известны
pub fn подключи() {
    // Имена нужны даже до первого чтения настроек: стиль с неизвестным
    // именем GTK не примет вовсе
    примени();
    let файл = gio::File::for_path(путь_настроек());
    if let Ok(с) = файл.monitor_file(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE) {
        с.connect_changed(|_, _, _, _| примени());
        СЛЕЖКА.with(|м| *м.borrow_mut() = Some(с));
    }
}

#[cfg(test)]
mod проверки {
    use super::*;

    #[test]
    fn цвета_для_обеих_тем() {
        let а = ("#22c55e".to_string(), "#14b8a6".to_string());
        assert!(цвета(true, &а).contains("glower_bg #f4f5f9"));
        assert!(цвета(false, &а).contains("glower_bg #161821"));
        assert!(цвета(false, &а).contains("glower_accent #22c55e"));
    }

    #[test]
    fn settings_ini_меняет_только_свою_строку() {
        assert_eq!(settings_ini("", true), "[Settings]\ngtk-application-prefer-dark-theme=1\n");
        let было = "[Settings]\ngtk-font-name=Noto Sans 11\ngtk-application-prefer-dark-theme=1\n";
        assert_eq!(settings_ini(было, false), "[Settings]\ngtk-application-prefer-dark-theme=0\ngtk-font-name=Noto Sans 11\n");
    }

    #[test]
    fn акцент_свой_и_готовый() {
        assert_eq!(акцент(&serde_json::json!({ "accent": 5 })).0, "#22c55e");
        assert_eq!(акцент(&serde_json::json!({ "accent": 0, "accentCustom": "#123456" })).0, "#123456");
        assert_eq!(акцент(&serde_json::json!({})).0, "#3a86ff");
    }
}
