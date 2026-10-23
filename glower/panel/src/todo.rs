//! Дела — простой список задач (glower-todo).
//!
//! Список лежит в ~/.config/glower/todo.json; его же показывает виджет
//! «Дела» на рабочем столе.

use crate::apps::{кнопка, подготовь, пиши_данные, читай_данные};
use gtk::glib;
use gtk::prelude::*;
use serde_json::{json, Value};
use std::rc::Rc;

const СТИЛЬ: &str = r#"
.todo .row { padding: 8px 10px; border-radius: 10px; background-color: alpha(@glower_fg, 0.05); }
.todo .done { opacity: 0.45; }
.todo .count { opacity: 0.6; }
"#;

pub const ФАЙЛ: &str = "todo.json";

pub fn дела() -> Vec<Value> {
    читай_данные(ФАЙЛ).as_array().cloned().unwrap_or_default()
}

fn сохрани(с: &[Value]) {
    пиши_данные(ФАЙЛ, &Value::Array(с.to_vec()));
}

pub fn запусти(_доводы: Vec<String>) -> ! {
    подготовь("glower-todo", "Дела");
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль дел");
    gtk::StyleContext::add_provider_for_screen(&gtk::gdk::Screen::default().expect("экран"), &стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_title("Дела");
    окно.set_default_size(480, 600);
    окно.set_icon_name(Some("emblem-default"));
    окно.style_context().add_class("glower-app");
    окно.style_context().add_class("todo");
    окно.connect_destroy(|_| gtk::main_quit());

    let колонка = gtk::Box::new(gtk::Orientation::Vertical, 0);
    окно.add(&колонка);
    let верх = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    верх.style_context().add_class("toolbar");
    let поле = gtk::Entry::new();
    поле.set_placeholder_text(Some("Новое дело…"));
    поле.set_hexpand(true);
    let плюс = кнопка("＋", "Добавить");
    плюс.style_context().add_class("accent");
    верх.add(&поле);
    верх.add(&плюс);
    колонка.add(&верх);
    let список = gtk::Box::new(gtk::Orientation::Vertical, 6);
    список.set_margin(12);
    let листание = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    листание.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    листание.set_vexpand(true);
    листание.add(&список);
    колонка.add(&листание);
    let низ = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    низ.style_context().add_class("statusbar");
    let счёт = gtk::Label::new(None);
    счёт.style_context().add_class("count");
    счёт.set_hexpand(true);
    счёт.set_xalign(0.0);
    let убрать = кнопка("Убрать сделанные", "Удалить все отмеченные дела");
    низ.add(&счёт);
    низ.add(&убрать);
    колонка.add(&низ);

    let рисуй: Rc<std::cell::RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(std::cell::RefCell::new(None));
    let сам = рисуй.clone();
    let р: Rc<dyn Fn()> = Rc::new(move || {
        for р in список.children() {
            список.remove(&р);
        }
        let все = дела();
        let осталось = все.iter().filter(|д| д["d"].as_bool() != Some(true)).count();
        счёт.set_text(&if все.is_empty() { "Дел нет".into() } else { format!("Осталось: {осталось} из {}", все.len()) });
        if все.is_empty() {
            let п = gtk::Label::new(Some("Дел нет 🎉"));
            п.style_context().add_class("muted");
            п.set_margin_top(40);
            список.add(&п);
        }
        for (i, д) in все.iter().enumerate() {
            let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            ряд.style_context().add_class("row");
            let галка = gtk::CheckButton::new();
            галка.set_active(д["d"].as_bool() == Some(true));
            ряд.add(&галка);
            let текст = gtk::Label::new(Some(д["t"].as_str().unwrap_or("")));
            текст.set_xalign(0.0);
            текст.set_hexpand(true);
            текст.set_line_wrap(true);
            if д["d"].as_bool() == Some(true) {
                текст.style_context().add_class("done");
                текст.set_markup(&format!("<s>{}</s>", glib::markup_escape_text(д["t"].as_str().unwrap_or(""))));
            }
            ряд.add(&текст);
            let у = кнопка("×", "Удалить");
            ряд.add(&у);
            let с2 = сам.clone();
            галка.connect_toggled(move |г| {
                let mut все = дела();
                if let Some(д) = все.get_mut(i) {
                    д["d"] = json!(г.is_active());
                }
                сохрани(&все);
                let с3 = с2.clone();
                glib::idle_add_local_once(move || {
                    if let Some(р) = с3.borrow().clone() {
                        р();
                    }
                });
            });
            let с2 = сам.clone();
            у.connect_clicked(move |_| {
                let mut все = дела();
                if i < все.len() {
                    все.remove(i);
                }
                сохрани(&все);
                if let Some(р) = с2.borrow().clone() {
                    р();
                }
            });
            список.add(&ряд);
        }
        список.show_all();
    });
    *рисуй.borrow_mut() = Some(р.clone());
    р();
    let добавь = {
        let (поле, р) = (поле.clone(), р.clone());
        Rc::new(move || {
            let т = поле.text().trim().to_string();
            if т.is_empty() {
                return;
            }
            let mut все = дела();
            все.push(json!({ "t": т, "d": false }));
            сохрани(&все);
            поле.set_text("");
            р();
        })
    };
    {
        let д = добавь.clone();
        плюс.connect_clicked(move |_| д());
    }
    поле.connect_activate(move |_| добавь());
    {
        let р = р.clone();
        убрать.connect_clicked(move |_| {
            let все: Vec<Value> = дела().into_iter().filter(|д| д["d"].as_bool() != Some(true)).collect();
            сохрани(&все);
            р();
        });
    }
    окно.show_all();
    gtk::main();
    std::process::exit(0);
}
