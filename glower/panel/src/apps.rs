//! Общее для нативных программ системы: Блокнот, Калькулятор и следующие.
//!
//! Каждая — та же программа, что панель, под своим именем (glower-notepad,
//! glower-calc), и настоящее окно оконного сервера: Alt+Tab, Alt+F4,
//! сворачивание — всё как у любой программы.

use gtk::glib;
use gtk::prelude::*;
use std::path::PathBuf;

/// Вид программ — те же тёмные поверхности, что у окон системы на странице.
const СТИЛЬ: &str = r#"
window.glower-app { background-color: @glower_bg; }
window.glower-app, window.glower-app * {
  color: @glower_fg;
  font-family: sans-serif;
  text-shadow: none;
  -gtk-icon-shadow: none;
}
.glower-app .toolbar { padding: 8px 10px; border-bottom: 1px solid alpha(@glower_fg, 0.07); }
.glower-app .findbar { padding: 6px 10px; border-bottom: 1px solid alpha(@glower_fg, 0.07); background-color: @glower_bg3; }
.glower-app button {
  background-image: none;
  background-color: alpha(@glower_fg, 0.07);
  border: none;
  border-radius: 9px;
  box-shadow: none;
  padding: 5px 11px;
  min-height: 0;
}
.glower-app button:hover { background-color: alpha(@glower_fg, 0.13); }
.glower-app button.accent { background-image: linear-gradient(140deg, @glower_accent, @glower_accent2); }
.glower-app button.tab-close { background: none; padding: 0 2px; }
.glower-app entry {
  background-color: alpha(@glower_fg, 0.06);
  border: 1px solid alpha(@glower_fg, 0.12);
  border-radius: 9px;
  box-shadow: none;
  padding: 4px 9px;
  min-height: 0;
}
.glower-app entry:focus { border-color: alpha(@glower_accent, 0.8); }
.glower-app .muted { opacity: 0.6; }
.glower-app textview, .glower-app textview text { background-color: @glower_bg2; }
.glower-app .statusbar { padding: 4px 12px; font-size: 12px; opacity: 0.65; border-top: 1px solid alpha(@glower_fg, 0.07); }
.glower-app notebook header { background-color: @glower_bg; border: none; box-shadow: none; }
.glower-app notebook tab { padding: 4px 10px; border: none; }
.glower-app notebook tab:checked { background-color: @glower_bg2; border-radius: 8px 8px 0 0; }
.glower-app notebook stack { background-color: @glower_bg2; }
.glower-app switch { background-color: alpha(@glower_fg, 0.15); border: none; }
.glower-app switch:checked { background-image: linear-gradient(140deg, @glower_accent, @glower_accent2); }
.glower-app switch slider { background-color: #ffffff; border: none; }
"#;

/// Имя программы для оконного сервера (по нему её узнают док и Alt+Tab),
/// GTK, тёмная тема и общий стиль.
pub fn подготовь(имя: &str, название: &str) {
    glib::set_prgname(Some(имя));
    glib::set_application_name(название);
    if gtk::init().is_err() {
        eprintln!("{имя}: нет экрана");
        std::process::exit(1);
    }
    crate::i18n::подключи();
    // Тема и акцент — до своего стиля: в нём имена цветов, а не цвета
    crate::theme::подключи();
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль программ");
    gtk::StyleContext::add_provider_for_screen(
        &gtk::gdk::Screen::default().expect("экран"),
        &стиль,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

pub fn кнопка(текст: &str, подсказка: &str) -> gtk::Button {
    let б = gtk::Button::with_label(текст);
    б.set_tooltip_text(Some(подсказка));
    б.set_valign(gtk::Align::Center);
    б
}

/// Где по умолчанию открывать и сохранять: документы системы
/// (~/GlowerOS/Документы), а нет их — дом.
pub fn папка_документов() -> PathBuf {
    let дом = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()));
    for п in [дом.join("GlowerOS").join("Документы"), дом.join("Документы"), дом.join("Documents")] {
        if п.is_dir() {
            return п;
        }
    }
    дом
}

/// Данные программ — файлом в ~/.config/glower: календарь, будильники,
/// дела. Их читает и рабочий стол (напоминания, виджеты), поэтому не в
/// памяти программы, а на диске
pub fn файл_данных(имя: &str) -> PathBuf {
    glib::user_config_dir().join("glower").join(имя)
}

pub fn читай_данные(имя: &str) -> serde_json::Value {
    std::fs::read_to_string(файл_данных(имя)).ok().and_then(|т| serde_json::from_str(&т).ok()).unwrap_or(serde_json::Value::Null)
}

/// Записать целиком: сперва во временный файл, потом подменой
pub fn пиши_данные(имя: &str, что: &serde_json::Value) {
    let путь = файл_данных(имя);
    if let Some(р) = путь.parent() {
        let _ = std::fs::create_dir_all(р);
    }
    let временный = путь.with_extension("new");
    if std::fs::write(&временный, serde_json::to_vec_pretty(что).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(&временный, &путь);
    }
}

/// Папка «Изображения» системы
pub fn папка_изображений() -> PathBuf {
    let дом = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()));
    let корень = std::env::var("GLOWER_ROOT").map(PathBuf::from).unwrap_or_else(|_| дом.join("GlowerOS"));
    for п in [корень.join("Изображения"), дом.join("Изображения"), дом.join("Pictures")] {
        if п.is_dir() {
            return п;
        }
    }
    корень.join("Изображения")
}
