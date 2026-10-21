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
window.glower-app { background-color: #161821; }
window.glower-app, window.glower-app * {
  color: #e8eaf2;
  font-family: sans-serif;
  text-shadow: none;
  -gtk-icon-shadow: none;
}
.glower-app .toolbar { padding: 8px 10px; border-bottom: 1px solid rgba(255, 255, 255, 0.07); }
.glower-app .findbar { padding: 6px 10px; border-bottom: 1px solid rgba(255, 255, 255, 0.07); background-color: #1b1e28; }
.glower-app button {
  background-image: none;
  background-color: rgba(255, 255, 255, 0.07);
  border: none;
  border-radius: 9px;
  box-shadow: none;
  padding: 5px 11px;
  min-height: 0;
}
.glower-app button:hover { background-color: rgba(255, 255, 255, 0.13); }
.glower-app button.accent { background-image: linear-gradient(140deg, #5b8cff, #8b5cf6); }
.glower-app button.tab-close { background: none; padding: 0 2px; }
.glower-app entry {
  background-color: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 9px;
  box-shadow: none;
  padding: 4px 9px;
  min-height: 0;
}
.glower-app entry:focus { border-color: rgba(120, 150, 255, 0.8); }
.glower-app .muted { opacity: 0.6; }
.glower-app textview, .glower-app textview text { background-color: #12141b; }
.glower-app .statusbar { padding: 4px 12px; font-size: 12px; opacity: 0.65; border-top: 1px solid rgba(255, 255, 255, 0.07); }
.glower-app notebook header { background-color: #161821; border: none; box-shadow: none; }
.glower-app notebook tab { padding: 4px 10px; border: none; }
.glower-app notebook tab:checked { background-color: #12141b; border-radius: 8px 8px 0 0; }
.glower-app notebook stack { background-color: #12141b; }
.glower-app switch { background-color: rgba(255, 255, 255, 0.15); border: none; }
.glower-app switch:checked { background-image: linear-gradient(140deg, #5b8cff, #8b5cf6); }
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
    if let Some(н) = gtk::Settings::default() {
        н.set_gtk_application_prefer_dark_theme(true);
    }
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
