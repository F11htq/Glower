//! Лоток: значки, которые кладут мессенджеры, качалки и VPN.
//!
//! Приёмник живёт в агенте (sys.tray), мы показываем, что он собрал.
//! Левой кнопкой — открыть программу, правой — наше меню: в нём всегда есть
//! «Закрыть программу», ради которого человек сюда чаще всего и приходит.

use crate::agent::позови;
use base64::Engine;
use gtk::gdk_pixbuf::{InterpType, Pixbuf, PixbufLoader};
use gtk::glib;
use gtk::prelude::*;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

/// Значок из data: URL — так агент отдаёт картинки программ.
fn картинка(адрес: &str) -> Option<Pixbuf> {
    let (_, данные) = адрес.split_once(";base64,")?;
    let байты = base64::engine::general_purpose::STANDARD.decode(данные.trim()).ok()?;
    let чтец = PixbufLoader::new();
    чтец.write(&байты).ok()?;
    чтец.close().ok()?;
    чтец.pixbuf()?.scale_simple(16, 16, InterpType::Bilinear)
}

fn нажми(з: &Value, действие: &str) {
    позови(
        "sys.tray.нажать",
        json!({ "служба": з["service"], "путь": з["path"], "действие": действие }),
        |_| {},
    );
}

fn меню_значка(кнопка: &gtk::Button, з: &Value, событие: &gtk::gdk::EventButton) {
    let меню = gtk::Menu::new();
    let имя = з["name"].as_str().or(з["tooltip"].as_str()).unwrap_or("Программа").to_string();
    let заголовок = gtk::MenuItem::with_label(&имя);
    заголовок.set_sensitive(false);
    меню.append(&заголовок);
    меню.append(&gtk::SeparatorMenuItem::new());

    let пункт = |текст: &str| {
        let п = gtk::MenuItem::with_label(текст);
        меню.append(&п);
        п
    };
    let з1 = з.clone();
    пункт("Открыть").connect_activate(move |_| нажми(&з1, "Activate"));
    if з["menu"].as_bool() == Some(true) || з["itemIsMenu"].as_bool() == Some(true) || з["menu"].is_string() {
        let з2 = з.clone();
        пункт("Меню программы").connect_activate(move |_| нажми(&з2, "ContextMenu"));
    }
    let з3 = з.clone();
    пункт("Закрыть программу").connect_activate(move |_| {
        // Имя отдаём вместе с адресом: если шина назовёт не тот процесс,
        // агент будет искать настоящий по имени
        let имя = з3["name"].as_str().or(з3["tooltip"].as_str()).or(з3["id"].as_str()).unwrap_or("");
        позови("sys.tray.закрыть", json!({ "служба": з3["service"], "имя": имя }), |о| {
            if let Err(е) = о {
                eprintln!("glower-panel: не вышло закрыть программу: {е}");
            }
        });
    });
    меню.show_all();
    let событие: &gtk::gdk::Event = событие;
    меню.popup_at_widget(кнопка, gtk::gdk::Gravity::SouthWest, gtk::gdk::Gravity::NorthWest, Some(событие));
}

fn нарисуй(место: &gtk::Box, список: &[Value]) {
    for ребёнок in место.children() {
        место.remove(&ребёнок);
    }
    for з in список {
        let б = gtk::Button::new();
        б.style_context().add_class("tray");
        let подсказка = з["tooltip"].as_str().or(з["name"].as_str()).unwrap_or("");
        if !подсказка.is_empty() {
            б.set_tooltip_text(Some(подсказка));
        }
        match з["iconData"].as_str().and_then(картинка) {
            Some(к) => б.add(&gtk::Image::from_pixbuf(Some(&к))),
            None => {
                let буква: String = з["name"].as_str().unwrap_or("?").trim().chars().take(1).collect();
                б.add(&gtk::Label::new(Some(&буква.to_uppercase())));
            }
        }
        let з1 = з.clone();
        б.connect_clicked(move |_| нажми(&з1, "Activate"));
        let з2 = з.clone();
        б.connect_button_press_event(move |к, с| {
            if с.button() == 3 {
                меню_значка(к, &з2, с);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        место.add(&б);
    }
    место.show_all();
}

/// Лоток в полосе. Опрашиваем раз в две секунды и перерисовываем, только
/// если набор значков изменился.
pub fn лоток() -> gtk::Box {
    let место = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let было = Rc::new(RefCell::new(String::new()));
    let опрос = {
        let место = место.clone();
        move || {
            let место = место.clone();
            let было = было.clone();
            позови("sys.tray", json!({}), move |о| {
                let Ok(д) = о else { return };
                let список = д["значки"].as_array().cloned().unwrap_or_default();
                let строкой = serde_json::to_string(&список).unwrap_or_default();
                if *было.borrow() != строкой {
                    *было.borrow_mut() = строкой;
                    нарисуй(&место, &список);
                }
            });
        }
    };
    опрос();
    glib::timeout_add_local(Duration::from_secs(2), move || {
        опрос();
        glib::ControlFlow::Continue
    });
    место
}
