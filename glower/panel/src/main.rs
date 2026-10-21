//! Панель GlowerOS — полоса наверху экрана, нативная.
//!
//! Прежде панель была страницей, и её рисовал целый браузерный движок:
//! сотни мегабайт памяти и отдельные процессы ради полосы в тридцать точек
//! с часами. Здесь та же панель — с теми же кнопками, словами и разговором
//! с агентом — нарисована GTK напрямую.
//!
//! Устроена она так же, как страничная (js/панель.js), и говорит на той же
//! шине: стол не знает и не должен знать, какая из двух панелей сейчас
//! над ним. Оболочка (linux/glower-shell) запускает эту, а если её нет или
//! она падает — возвращается к страничной.

mod agent;
mod icons;
mod menus;
mod overview;
mod popup;
mod tray;

use agent::{скажи, слушай};
use gtk::glib;
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use icons::{Вид, Заряд, Значок};
use serde_json::{json, Value};
use std::time::Duration;

/// Высота полосы — та же, что у страничной (--полоса-высота).
const ВЫСОТА: i32 = 30;

const СТИЛЬ: &str = r#"
window.bar {
  background-color: rgba(20, 22, 30, 0.82);
  box-shadow: inset 0 -1px rgba(255, 255, 255, 0.10);
}
window.bar, window.bar *, .menu, .menu * {
  color: rgba(255, 255, 255, 0.95);
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
  background-color: rgba(24, 26, 34, 0.96);
  border-radius: 14px;
  padding: 8px;
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.10);
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
.menu button:hover { background-color: rgba(255, 255, 255, 0.12); }
.menu button.danger:hover { background-color: rgba(220, 70, 70, 0.35); }
.menu button.tile {
  background-color: rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  padding: 9px 11px;
}
.menu button.tile:hover { background-color: rgba(255, 255, 255, 0.14); }
.menu button.tile.on { background-color: #3d7bf0; }
.menu button.tile.on:hover { background-color: #4f89f5; }
.menu button.tile:disabled { opacity: 0.45; }
.menu .tile-name { font-weight: 600; }
.menu .tile-sub { font-size: 11px; opacity: 0.7; }
.menu button.action {
  background-color: rgba(255, 255, 255, 0.08);
  border-radius: 10px;
  padding: 7px 10px;
}
.menu button.action.main { background-color: #3d7bf0; }
.menu .title { font-weight: 700; font-size: 14px; padding: 4px 6px 6px; }
.menu .quiet { opacity: 0.65; padding: 12px; }
.menu .small { font-size: 11px; opacity: 0.55; padding: 0 10px 6px; }
.menu .pair { padding: 8px 10px; }
.menu .value { opacity: 0.75; }
.menu .slider { padding: 6px 10px 2px; }
.menu scale { padding: 4px 0; }
.menu scale trough {
  background-color: rgba(255, 255, 255, 0.15);
  border: none;
  border-radius: 3px;
  min-height: 5px;
}
.menu scale highlight { background-color: #5b93f5; border: none; border-radius: 3px; }
.menu scale slider {
  background-color: #ffffff;
  border: none;
  box-shadow: none;
  border-radius: 8px;
  min-width: 15px;
  min-height: 15px;
}
.menu separator { background-color: rgba(255, 255, 255, 0.12); margin: 4px 6px; min-height: 1px; }
.menu calendar { background: none; border: none; padding: 4px; }
.menu calendar:selected { background-color: #3d7bf0; border-radius: 6px; }
.menu calendar.header { border: none; background: none; }
.menu calendar:indeterminate { color: rgba(255, 255, 255, 0.3); }
window.overview { background-color: rgba(12, 14, 22, 0.80); }
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
.overview entry.search:focus { box-shadow: inset 0 0 0 1px rgba(120, 160, 255, 0.8); }
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
.overview button.desk.on { box-shadow: inset 0 0 0 2px #5b93f5; }
.overview button.desk.drop { background-color: rgba(91, 147, 245, 0.35); box-shadow: inset 0 0 0 2px #8fb6ff; }
.overview button.desk-plus { min-width: 72px; font-size: 22px; }
.overview button.desk-x { padding: 0 7px; margin: 4px; border-radius: 9px; font-size: 13px; }
.overview button.win { background-color: rgba(255, 255, 255, 0.07); border-radius: 10px; padding: 6px 12px; }
.overview .empty { opacity: 0.6; padding: 30px; }
window.curtain { background-color: #000000; }
window.curtain label { color: rgba(255, 255, 255, 0.8); font-size: 18px; }
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
    if gtk::init().is_err() {
        eprintln!("glower-panel: нет экрана — панель показывать негде");
        std::process::exit(1);
    }
    if !gtk_layer_shell::is_supported() {
        // Без слоёв панель не встанет над окнами. Оболочка увидит, что мы
        // вышли, и покажет страничную — она умеет жить и так.
        eprintln!("glower-panel: оконный сервер не умеет слоёв");
        std::process::exit(3);
    }

    if let Some(н) = gtk::Settings::default() {
        н.set_gtk_application_prefer_dark_theme(true);
    }
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль панели");
    gtk::StyleContext::add_provider_for_screen(
        &gtk::gdk::Screen::default().expect("экран"),
        &стиль,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

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
    let (быстрые, _) = кнопка_со_значком(Вид::Ползунки, "Быстрые настройки");
    быстрые.connect_clicked(|к| menus::быстрые(к));
    полоса.pack_end(&быстрые, false, false, 0);
    let язык = gtk::Button::with_label("ENG");
    язык.set_tooltip_text(Some("Раскладка"));
    язык.connect_clicked(|_| скажи("покажи", json!("раскладка")));
    полоса.pack_end(&язык, false, false, 0);
    полоса.pack_end(&tray::лоток(), false, false, 2);

    окно.show_all();

    // ---------- что говорит стол ----------
    let вести = слушай(
        &["раскладка", "часы", "батарея", "обновления", "настройки", "стол-занят", "затемни",
          "программы", "обзор-данные"],
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
                            let т: String = т.to_uppercase().chars().take(3).collect();
                            язык.set_label(&т);
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
                            окно.set_exclusive_zone(0);
                            окно.hide();
                        } else {
                            окно.show();
                            окно.set_exclusive_zone(ВЫСОТА);
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

    gtk::main();
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
