//! Рабочий стол GlowerOS — одна нативная программа.
//!
//! Прежде стол, панель и окна системы были страницей, и рисовал их целый
//! браузерный движок: сотни мегабайт памяти ради обоев и полосы с часами.
//! Теперь всё своё: стол с обоями, значками и виджетами (desktop), полоса
//! наверху, док, меню и просмотр задач, уведомления, а за ними — ядро
//! (core), которое хранит настройки и отвечает Параметрам. Сеанс
//! (linux/glower-session) запускает эту программу внутри labwc.
//!
//! Та же программа под другими именами — программы системы: Блокнот,
//! Проводник, Параметры, Магазин и остальные.

// Имена здесь русские, как во всей системе, а у GTK и Rust — латинские.
// Буквы «а», «с», «х», «у» в них неизбежно встречаются рядом с «a», «c»,
// «x», «y» — это не опечатки, и предупреждение о похожих буквах только шумит.
#![allow(confusable_idents)]

mod agent;
mod apps;
mod calc;
mod calendar;
mod clock;
mod core;
mod desktop;
mod dock;
mod explorer;
mod i18n;
mod icons;
mod installer;
mod lock;
mod media;
mod menus;
mod music;
mod notify;
mod notepad;
mod overview;
mod paint;
mod popup;
mod props;
mod settings;
mod setup;
mod store;
mod snip;
mod taskmgr;
mod photos;
mod theme;
mod todo;
mod tray;

use agent::{скажи, слушай};
use gtk::glib;
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use icons::{Вид, Заряд, Значок};
use serde_json::{json, Value};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

/// Высота полосы — та же, что у страничной (--полоса-высота).
const ВЫСОТА: i32 = 30;

/// Поверх основного стиля, когда сервер размывает фон (glower-wm)
const АКРИЛ: &str = r#"
.menu, .toast { background-color: alpha(@glower_bg, 0.72); }
window.overview { background-color: rgba(10, 12, 20, 0.55); }
.overview button.task { background-color: rgba(30, 33, 46, 0.70); }
"#;

const СТИЛЬ: &str = r#"
window.bar {
  background-color: rgba(20, 22, 30, 0.82);
  box-shadow: inset 0 -1px rgba(255, 255, 255, 0.10);
}
window.bar, window.bar * { color: rgba(255, 255, 255, 0.95); }
.menu, .menu * { color: alpha(@glower_fg, 0.95); }
window.bar, window.bar *, .menu, .menu * {
  font-family: sans-serif;
  font-size: 13px;
  text-shadow: none;
  -gtk-icon-shadow: none;
}
.bar-box.dim { opacity: 0.4; }
window.bar button {
  background: none;
  border: none;
  box-shadow: none;
  padding: 0 10px;
  margin: 3px 1px;
  border-radius: 7px;
  min-height: 0;
  min-width: 0;
  transition: background-color 200ms ease-out;
}
window.bar button:hover { background-color: rgba(255, 255, 255, 0.14); }
window.bar button:active, window.bar button.open { background-color: rgba(255, 255, 255, 0.22); }
window.bar button.programs { padding-left: 12px; }
window.bar button.programs label { font-weight: 600; }
window.bar button.tray { padding: 0 5px; }
window.bar button.tray label { font-size: 11px; font-weight: 600; }
window.bar .date { opacity: 0.6; }
window.bar button.updates { color: #8cc8ff; }

.menu {
  background-color: alpha(@glower_bg, 0.96);
  border-radius: 14px;
  padding: 8px;
  box-shadow: inset 0 0 0 1px alpha(@glower_fg, 0.10);
}
.menu button {
  background: none;
  border: none;
  box-shadow: none;
  border-radius: 8px;
  padding: 8px 10px;
  min-height: 0;
  transition: background-color 180ms ease-out;
}
.menu button:hover { background-color: alpha(@glower_fg, 0.12); }
.menu button.danger:hover { background-color: rgba(220, 70, 70, 0.35); }
.menu button.tile {
  background-color: alpha(@glower_fg, 0.08);
  border-radius: 12px;
  padding: 9px 11px;
}
.menu button.tile:hover { background-color: alpha(@glower_fg, 0.14); }
.menu button.tile.on { background-color: @glower_accent; }
.menu button.tile.on:hover { background-color: @glower_accent; }
.menu button.tile:disabled { opacity: 0.45; }
.menu .tile-name { font-weight: 600; }
.menu .tile-sub { font-size: 11px; opacity: 0.7; }
.menu button.action {
  background-color: alpha(@glower_fg, 0.08);
  border-radius: 10px;
  padding: 7px 10px;
}
.menu button.action.main { background-color: @glower_accent; }
.menu .now-playing { padding: 6px 8px; margin: 4px 0; border-radius: 12px; background-color: alpha(@glower_fg, 0.06); }
.menu .now-playing button { padding: 4px 8px; }
.menu .title { font-weight: 700; font-size: 14px; padding: 4px 6px 6px; }
.menu .quiet { opacity: 0.65; padding: 12px; }
.menu .kb-code {
  font-weight: 700; font-size: 11px; min-width: 30px; padding: 3px 0;
  border-radius: 6px; background-color: alpha(@glower_fg, 0.10);
}
.menu button.kb-on .kb-code { background-color: @glower_accent; color: #ffffff; }
.menu button.kb-on { background-color: alpha(@glower_fg, 0.07); }
.menu .kb-check { color: @glower_accent; font-weight: 700; }
.menu .small { font-size: 11px; opacity: 0.55; padding: 0 10px 6px; }
.menu .pair { padding: 8px 10px; }
.menu .value { opacity: 0.75; }
.menu .slider { padding: 6px 10px 2px; }
.menu scale { padding: 4px 0; }
.menu scale trough {
  background-color: alpha(@glower_fg, 0.15);
  border: none;
  border-radius: 3px;
  min-height: 5px;
}
.menu scale highlight { background-color: @glower_accent; border: none; border-radius: 3px; }
.menu scale slider {
  background-color: #ffffff;
  border: none;
  box-shadow: none;
  border-radius: 8px;
  min-width: 15px;
  min-height: 15px;
}
.menu separator { background-color: alpha(@glower_fg, 0.12); margin: 4px 6px; min-height: 1px; }
.menu calendar { background: none; border: none; padding: 4px; }
.menu calendar:selected { background-color: @glower_accent; border-radius: 6px; }
.menu calendar.header { border: none; background: none; }
.menu calendar:indeterminate { color: alpha(@glower_fg, 0.3); }
window.overview { background-color: rgba(10, 12, 20, 0.86); }
.overview, .overview * {
  color: rgba(255, 255, 255, 0.95);
  font-family: sans-serif;
  text-shadow: none;
  -gtk-icon-shadow: none;
}
.overview scrolledwindow, .overview viewport, .overview flowbox, .overview flowboxchild {
  background: none;
  border: none;
  padding: 0;
}
.overview entry.search {
  background-color: rgba(255, 255, 255, 0.10);
  border: none;
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.12);
  border-radius: 12px;
  padding: 9px 14px;
  font-size: 15px;
  caret-color: #ffffff;
}
.overview entry.search:focus { box-shadow: inset 0 0 0 1px alpha(@glower_accent, 0.8); }
.overview button {
  background: none;
  border: none;
  box-shadow: none;
  border-radius: 14px;
  padding: 8px;
  transition: background-color 150ms ease-out;
}
.overview button:hover { background-color: rgba(255, 255, 255, 0.10); }
.overview button.app { min-width: 104px; padding: 10px 6px; }
.overview .app-name { font-size: 12px; }
.overview .glyph {
  font-size: 28px;
  border-radius: 14px;
  min-width: 52px;
  min-height: 52px;
  background-color: rgba(255, 255, 255, 0.12);
}
.overview .glyph.small { font-size: 14px; border-radius: 7px; min-width: 24px; min-height: 24px; }
.overview button.desk {
  background-color: rgba(255, 255, 255, 0.08);
  min-width: 128px;
  min-height: 72px;
  border-radius: 12px;
  font-weight: 600;
}
.overview button.desk:hover { background-color: rgba(255, 255, 255, 0.14); }
.overview button.desk.on { box-shadow: inset 0 0 0 2px @glower_accent; }
.overview button.desk.drop { background-color: alpha(@glower_accent, 0.35); box-shadow: inset 0 0 0 2px #8fb6ff; }
.overview button.desk-plus { min-width: 72px; font-size: 22px; }
.overview button.desk-x { padding: 0 7px; margin: 4px; border-radius: 9px; font-size: 13px; }
.overview button.win { background-color: rgba(255, 255, 255, 0.07); border-radius: 10px; padding: 6px 12px; }
.overview .empty { opacity: 0.6; padding: 30px; }
.overview button.task {
  background-color: rgba(30, 33, 46, 0.95);
  border: 1px solid rgba(255, 255, 255, 0.10);
  border-radius: 16px;
  min-width: 220px;
  min-height: 150px;
  padding: 14px;
}
.overview button.task:hover { background-color: rgba(42, 46, 62, 0.97); box-shadow: inset 0 0 0 2px @glower_accent; }
.overview button.task.min { opacity: 0.6; }
.overview .task-empty { opacity: 0.6; font-size: 15px; }
window.dock, window.dock * {
  color: rgba(255, 255, 255, 0.95);
  font-family: sans-serif;
  text-shadow: none;
  -gtk-icon-shadow: none;
}
window.dock button.dock-item {
  background: none;
  border: none;
  box-shadow: none;
  border-radius: 12px;
  padding: 4px 5px 0;
  min-width: 0;
  min-height: 0;
  transition: background-color 150ms ease-out;
}
window.dock button.dock-item:hover { background-color: rgba(255, 255, 255, 0.12); }
window.dock button.dock-item.active { background-color: rgba(255, 255, 255, 0.16); }
window.dock button.dock-item.minimized .dock-ico { opacity: 0.55; }
window.dock .glyph {
  font-size: 22px;
  border-radius: 11px;
  min-width: 40px;
  min-height: 40px;
  background-color: rgba(255, 255, 255, 0.12);
}
window.dock .dot { background-color: rgba(255, 255, 255, 0.85); border-radius: 3px; min-width: 5px; min-height: 5px; }
window.dock .no-dot { min-width: 5px; min-height: 5px; }
window.dock separator.dock-sep { background-color: rgba(255, 255, 255, 0.16); min-width: 1px; margin: 8px 5px 10px; }
window.desktop .di { border-radius: 10px; padding: 6px 4px; }
window.desktop .di.sel { background-color: alpha(@glower_accent, 0.28); box-shadow: inset 0 0 0 1px rgba(160, 190, 255, 0.55); }
window.desktop .di-glyph { font-size: 40px; min-height: 48px; }
window.desktop .di-label {
  color: #ffffff;
  font-family: sans-serif;
  font-size: 12px;
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.85);
}
window.curtain { background-color: #000000; }
window.curtain label { color: rgba(255, 255, 255, 0.8); font-size: 18px; }
/* уведомления */
window.toasts, window.toasts * { color: alpha(@glower_fg, 0.95); font-family: sans-serif; font-size: 13px; }
.toast {
  background-color: alpha(@glower_bg, 0.96);
  border: 1px solid alpha(@glower_fg, 0.10);
  border-radius: 14px;
  padding: 12px 10px 12px 14px;
  min-width: 320px;
  margin: 0;
}
.note-row {
  background-color: alpha(@glower_fg, 0.05);
  border-radius: 12px;
  padding: 10px 12px;
}
.toast-title { font-weight: bold; }
.toast-text { color: alpha(@glower_fg, 0.75); }
.toast-time { color: alpha(@glower_fg, 0.5); font-size: 11px; }
.toast-emoji { font-size: 24px; }
button.toast-action {
  background-color: alpha(@glower_fg, 0.10);
  border: none; border-radius: 8px; padding: 3px 12px; margin-top: 6px; box-shadow: none;
}
button.toast-close { background: none; border: none; box-shadow: none; padding: 0 6px; color: alpha(@glower_fg, 0.5); }
button.toast-close:hover { color: white; }
window.bar button.bell .unread {
  background-color: #e5484d; border-radius: 8px; font-size: 10px; padding: 0 5px; min-width: 8px;
}
/* виджеты стола */
.widgets { margin: 0; }
.widget {
  background-color: rgba(16, 18, 28, 0.55);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 16px;
  padding: 14px 16px;
}
.widget, .widget * { color: rgba(255, 255, 255, 0.95); }
.widget .title { font-size: 11px; color: rgba(255, 255, 255, 0.6); font-weight: bold; }
.w-big { font-size: 34px; font-weight: 300; }
.w-sub { color: rgba(255, 255, 255, 0.7); }
.w-emoji { font-size: 26px; }
.widget checkbutton label { color: rgba(255, 255, 255, 0.9); }
.widget button.w-link { background: none; border: none; box-shadow: none; padding: 2px 0; color: rgba(160, 190, 255, 0.95); }
.w-today { background-color: @glower_accent; border-radius: 6px; color: white; font-weight: bold; }
textview.w-note, textview.w-note text { background-color: transparent; color: rgba(255,255,255,0.9); }
.widget levelbar block.filled { background-color: @glower_accent; border-radius: 3px; }
.widget levelbar block.empty { background-color: rgba(255,255,255,0.12); border-radius: 3px; }
"#;

/// «Правда» так, как её понимает страница: !!что.
fn правда(что: &Value) -> bool {
    match что {
        Value::Null => false,
        Value::Bool(б) => *б,
        Value::Number(ч) => ч.as_f64().is_some_and(|ч| ч != 0.0),
        Value::String(с) => !с.is_empty(),
        _ => true,
    }
}

fn кнопка_со_значком(вид: Вид, подсказка: &str) -> (gtk::Button, Значок) {
    let б = gtk::Button::new();
    let з = Значок::новый(вид);
    б.add(&з.холст);
    б.set_tooltip_text(Some(подсказка));
    (б, з)
}

fn main() {
    // Та же программа под другими именами: glower-lock — экран блокировки,
    // glower-login — экран входа. Это один и тот же экран
    let имя = std::env::args().next().unwrap_or_default();
    let имя = имя.rsplit('/').next().unwrap_or("");
    if имя == "glower-lock" || std::env::args().any(|д| д == "--lock") {
        lock::запусти(lock::Режим::Замок);
    }
    if имя == "glower-login" || std::env::args().any(|д| д == "--login") {
        lock::запусти(lock::Режим::Вход);
    }
    // Программы системы — тоже эта программа, своими именами
    if имя == "glower-notepad" {
        notepad::запусти(std::env::args().skip(1).collect());
    }
    if имя == "glower-calc" {
        calc::запусти();
    }
    if имя == "glower-taskmgr" {
        taskmgr::запусти();
    }
    if имя == "glower-photos" {
        photos::запусти(std::env::args().skip(1).collect());
    }
    if имя == "glower-snip" {
        snip::запусти(std::env::args().skip(1).collect());
    }
    if имя == "glower-store" {
        store::запусти(std::env::args().skip(1).collect());
    }
    if имя == "glower-settings" {
        settings::запусти(std::env::args().skip(1).collect());
    }
    // Программы, которые прежде жили только на странице
    match имя {
        "glower-paint" => paint::запусти(std::env::args().skip(1).collect()),
        "glower-calendar" => calendar::запусти(std::env::args().skip(1).collect()),
        "glower-clock" => clock::запусти(std::env::args().skip(1).collect()),
        "glower-todo" => todo::запусти(std::env::args().skip(1).collect()),
        "glower-props" => props::запусти(std::env::args().skip(1).collect()),
        _ => {}
    }
    if имя == "glower-music" {
        music::запусти(std::env::args().skip(1).collect());
    }
    if имя == "glower-installer" {
        installer::запусти(std::env::args().skip(1).collect());
    }
    if имя == "glower-files" {
        explorer::запусти(std::env::args().skip(1).collect());
    }
    if gtk::init().is_err() {
        eprintln!("glower-panel: нет экрана — панель показывать негде");
        std::process::exit(1);
    }
    i18n::подключи();
    if !gtk_layer_shell::is_supported() {
        // Без слоёв рабочий стол не встанет ни под окнами, ни над ними:
        // такой оконный сервер нам не годится
        eprintln!("glower-panel: оконный сервер не умеет слоёв");
        std::process::exit(3);
    }

    crate::theme::подключи();
    if let Some(н) = gtk::Settings::default() {
        н.set_gtk_icon_theme_name(Some("Adwaita"));
    }
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль панели");
    gtk::StyleContext::add_provider_for_screen(
        &gtk::gdk::Screen::default().expect("экран"),
        &стиль,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    // Свой оконный сервер размывает то, что под меню и панелью, — тогда
    // меню можно сделать заметно прозрачнее, как «акрил» в Windows. Без
    // размытия прозрачное меню читалось бы плохо, поэтому только при нём
    if std::env::var_os("GLOWER_WM_BLUR").is_some() {
        let акрил = gtk::CssProvider::new();
        if акрил.load_from_data(АКРИЛ.as_bytes()).is_ok() {
            gtk::StyleContext::add_provider_for_screen(
                &gtk::gdk::Screen::default().expect("экран"),
                &акрил,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
            );
        }
    }

    // Стол — первым: он в самом низу, и пока его нет, под прозрачной
    // страницей стола был бы виден пустой экран
    desktop::создай();

    // ---------- полоса ----------
    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_decorated(false);
    окно.set_app_paintable(true);
    if let Some(в) = gtk::prelude::GtkWindowExt::screen(&окно).and_then(|э| э.rgba_visual()) {
        окно.set_visual(Some(&в));
    }
    окно.style_context().add_class("bar");
    окно.init_layer_shell();
    окно.set_layer(Layer::Top);
    окно.set_namespace("glower-panel");
    for край in [Edge::Top, Edge::Left, Edge::Right] {
        окно.set_anchor(край, true);
    }
    окно.set_size_request(-1, ВЫСОТА);
    окно.set_exclusive_zone(ВЫСОТА);
    // Клавиатура полосе не нужна: печатать в ней негде
    окно.set_keyboard_mode(KeyboardMode::None);

    // Пока на столе нет окон, полоса прозрачна и обои видны целиком. Открыли
    // программу — полоса темнеет, как строка меню в macOS: текст над окном
    // читается, а не теряется в чужих цветах
    let темнота = Rc::new(Cell::new(0.0f64));
    {
        let темнота = темнота.clone();
        окно.connect_draw(move |_, к| {
            к.set_operator(gtk::cairo::Operator::Source);
            к.set_source_rgba(0.055, 0.063, 0.086, 0.94 * темнота.get());
            let _ = к.paint();
            glib::Propagation::Proceed
        });
    }
    {
        // Одни часы на всё: окна могут прийти и уйти быстрее, чем полоса
        // дотемнеет, — тогда она просто поворачивает к новой цели
        let (окно2, темнота) = (окно.clone(), темнота.clone());
        let цель = Rc::new(Cell::new(0.0f64));
        let идёт = Rc::new(Cell::new(false));
        dock::следи_за_окнами(move |есть| {
            цель.set(if есть { 1.0 } else { 0.0 });
            if идёт.replace(true) {
                return;
            }
            let (окно3, темнота2, цель2, идёт2) = (окно2.clone(), темнота.clone(), цель.clone(), идёт.clone());
            glib::timeout_add_local(Duration::from_millis(16), move || {
                let было = темнота2.get();
                темнота2.set(было + (цель2.get() - было).clamp(-0.08, 0.08));
                окно3.queue_draw();
                if (цель2.get() - темнота2.get()).abs() < 1e-3 {
                    идёт2.set(false);
                    glib::ControlFlow::Break
                } else {
                    glib::ControlFlow::Continue
                }
            });
        });
    }

    let полоса = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    полоса.style_context().add_class("bar-box");
    полоса.set_margin_start(4);
    полоса.set_margin_end(4);
    окно.add(&полоса);

    // Слева — «Программы»: меню открывает стол, во весь экран
    let программы = gtk::Button::new();
    программы.style_context().add_class("programs");
    let внутри = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    внутри.add(&Значок::новый(Вид::Сетка).холст);
    внутри.add(&gtk::Label::new(Some("Программы")));
    программы.add(&внутри);
    программы.connect_clicked(|_| {
        popup::закрой();
        скажи("открой-пуск", json!("переключи"));
    });
    полоса.pack_start(&программы, false, false, 0);

    // Посередине — часы
    let часы = gtk::Button::new();
    let часы_ряд = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let время = gtk::Label::new(Some("00:00"));
    let дата = gtk::Label::new(None);
    дата.style_context().add_class("date");
    часы_ряд.add(&время);
    часы_ряд.add(&дата);
    часы.add(&часы_ряд);
    часы.connect_clicked(|к| menus::календарь(к));
    полоса.set_center_widget(Some(&часы));

    let обнови_часы = {
        let (время, дата) = (время.clone(), дата.clone());
        move || {
            let (в, д) = menus::время_и_дата();
            время.set_text(&в);
            дата.set_text(&д);
        }
    };
    обнови_часы();
    {
        let обнови = обнови_часы.clone();
        glib::timeout_add_local(Duration::from_secs(5), move || {
            обнови();
            glib::ControlFlow::Continue
        });
    }

    // Справа — лоток, язык, быстрые настройки, батарея, питание
    let (питание, _) = кнопка_со_значком(Вид::Питание, "Завершение работы");
    питание.connect_clicked(|к| menus::питание(к));
    полоса.pack_end(&питание, false, false, 0);
    let (батарея, значок_батареи) = кнопка_со_значком(Вид::Батарея, "Батарея");
    батарея.connect_clicked(|к| menus::батарея(к));
    полоса.pack_end(&батарея, false, false, 0);
    полоса.pack_end(&notify::колокол(), false, false, 0);
    let (быстрые, _) = кнопка_со_значком(Вид::Ползунки, "Быстрые настройки");
    быстрые.connect_clicked(|к| menus::быстрые(к));
    полоса.pack_end(&быстрые, false, false, 0);
    let язык = gtk::Button::with_label("ENG");
    язык.set_tooltip_text(Some("Раскладка"));
    язык.connect_clicked(|к| menus::раскладки(к));
    полоса.pack_end(&язык, false, false, 0);
    полоса.pack_end(&tray::лоток(), false, false, 2);
    следи_за_раскладкой(&язык);

    окно.show_all();
    dock::создай();
    notify::служба();
    core::начни();

    // ---------- что говорит стол ----------
    let вести = слушай(
        &["раскладка", "часы", "батарея", "обновления", "настройки", "стол-занят", "затемни",
          "программы", "обзор-данные", "док", "значки", "обои"],
        &["открой-пуск"],
    );
    {
        let окно = окно.clone();
        let полоса = полоса.clone();
        glib::MainContext::default().spawn_local(async move {
            while let Ok(в) = вести.recv().await {
                let что = &в.что;
                match в.тема.as_str() {
                    "раскладка" => {
                        if let Some(т) = что.as_str() {
                            let т: String = т.to_uppercase().chars().take(5).collect();
                            // Свой сервер знает текущую раскладку — значок показывает её
                            if файл_раскладки().filter(|ф| ф.exists()).is_none() {
                                язык.set_label(&т);
                            }
                            menus::ЗНАЮ.with(|з| з.borrow_mut().раскладка = т);
                        }
                    }
                    "часы" => {
                        menus::ПОЯС.with(|п| {
                            let mut п = п.borrow_mut();
                            п.где = что["пояс"].as_str().unwrap_or("").to_string();
                            п.ч12 = что["ч24"].as_bool() == Some(false);
                        });
                        обнови_часы();
                    }
                    "батарея" => {
                        let есть = что["есть"].as_bool() != Some(false);
                        let уровень = что["уровень"]
                            .as_f64()
                            .filter(|_| есть)
                            .map(|у| у.round().clamp(0.0, 100.0) as u8);
                        let заряжается = что["заряжается"].as_bool() == Some(true);
                        let от_сети = что["сеть"].as_bool() == Some(true);
                        значок_батареи.задай_заряд(Заряд { уровень, заряжается });
                        батарея.set_tooltip_text(Some(&match уровень {
                            Some(у) => format!(
                                "{у}% · {}",
                                if заряжается { "заряжается" } else if от_сети { "от розетки" } else { "от батареи" }
                            ),
                            None if от_сети => "Питание от розетки".into(),
                            None => "Батареи нет".into(),
                        }));
                    }
                    "обновления" => {
                        let н = что["система"].as_u64().unwrap_or(0);
                        menus::ЗНАЮ.with(|з| з.borrow_mut().обновлений = н);
                        if н > 0 {
                            питание.style_context().add_class("updates");
                            питание.set_tooltip_text(Some("Есть обновления системы"));
                        } else {
                            питание.style_context().remove_class("updates");
                            питание.set_tooltip_text(Some("Завершение работы"));
                        }
                    }
                    "настройки" => menus::ЗНАЮ.with(|з| {
                        let mut з = з.borrow_mut();
                        if let Some(в) = что["dnd"].as_bool() {
                            з.dnd = в;
                        }
                        if let Some(в) = что["nightLight"].as_bool() {
                            з.ночной = в;
                        }
                        if let Some(в) = что["эконом"].as_bool() {
                            з.эконом = в;
                        }
                    }),
                    // Заставка, приветствие, мастер, экран входа и программы во
                    // весь экран — панели над ними быть не должно, вместе с
                    // отведённой полосой
                    "стол-занят" => {
                        if правда(что) {
                            popup::закрой();
                            overview::закрой();
                            dock::занят(true);
                            окно.set_exclusive_zone(0);
                            окно.hide();
                        } else {
                            окно.show();
                            окно.set_exclusive_zone(ВЫСОТА);
                            dock::занят(false);
                        }
                    }
                    // Пока стол ждёт выделения области для снимка, темнеем и
                    // мы: живые часы на застывшем кадре выдают, что замер не всё
                    "затемни" => {
                        if правда(что) {
                            полоса.style_context().add_class("dim");
                        } else {
                            полоса.style_context().remove_class("dim");
                        }
                    }
                    // Клавиша Win, кнопка «Программы» или просьба стола: меню
                    // программ теперь наше. Пока стол занят — мастером,
                    // приветствием, программой во весь экран — не открываем
                    "открой-пуск" => {
                        if окно.is_visible() || что.as_bool() == Some(false) {
                            overview::переключи(что);
                        }
                    }
                    "программы" => overview::программы(что),
                    "обзор-данные" => overview::данные(что),
                    "док" => dock::данные(что),
                    "значки" => desktop::значки(что),
                    "обои" => desktop::обои(что),
                    _ => {}
                }
            }
        });
    }

    // Стол держит заставку, пока панель не скажет, что готова, и ему нужен
    // наш рост — на сколько опускать развёрнутые окна
    скажи("панель-готова", json!(true));
    скажи("полоса-высота", json!(ВЫСОТА));
    // Меню программ показываем мы, а не страница стола: она, услышав это,
    // перестаёт открывать своё (страничная панель при запуске говорит
    // обратное — на случай, если оболочка вернулась к ней)
    скажи("обзор-свой", json!(true));
    // То же с доком: внизу теперь наш, а свой стол прячет
    скажи("док-свой", json!(true));
    // И стол: обои и значки рисуем мы, страница становится прозрачной
    скажи("стол-свой", json!(true));

    gtk::main();
}

/// Где glower-wm пишет текущую раскладку: первая строка — «EN», «RU»
fn файл_раскладки() -> Option<std::path::PathBuf> {
    std::env::var_os("GLOWER_WM_SOCKET")?;
    Some(std::path::PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?).join("glower-wm").join("layout"))
}

/// Под labwc текущей раскладки панели не узнать: показываем их список.
/// Свой сервер пишет текущую в файл — значок меняется вместе с Alt+Shift
fn следи_за_раскладкой(значок: &gtk::Button) {
    let Some(путь) = файл_раскладки() else { return };
    let прочти = {
        let значок = значок.clone();
        let путь = путь.clone();
        move || {
            if let Ok(т) = std::fs::read_to_string(&путь) {
                if let Some(коротко) = т.lines().next().filter(|к| !к.is_empty()) {
                    значок.set_label(коротко);
                    значок.set_tooltip_text(т.lines().nth(1));
                }
            }
        }
    };
    прочти();
    let Some(папка) = путь.parent() else { return };
    let _ = std::fs::create_dir_all(папка);
    if let Ok(с) = gtk::gio::File::for_path(папка).monitor_directory(gtk::gio::FileMonitorFlags::WATCH_MOVES, gtk::gio::Cancellable::NONE) {
        с.connect_changed(move |_, _, _, _| прочти());
        // Следящий должен жить весь сеанс
        std::mem::forget(с);
    }
}

#[cfg(test)]
mod проверки {
    use super::правда;
    use serde_json::json;

    // «Стол занят» приходит и словом («#setup»), и флагом — как !!что
    #[test]
    fn правда_как_у_страницы() {
        for (что, ждём) in [(json!("#setup"), true), (json!(true), true), (json!(""), false),
                            (json!(false), false), (json!(null), false), (json!(0), false), (json!({}), true)] {
            assert_eq!(правда(&что), ждём, "{что}");
        }
    }
}
