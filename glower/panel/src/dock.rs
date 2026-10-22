//! Док — островок внизу экрана, нативный, в той же программе, что панель.
//!
//! Прежде док рисовала страница рабочего стола. Стол лежит под всеми
//! окнами, и это было видно: над развёрнутым Firefox подвести курсор к
//! нижнему краю было бесполезно — до стола курсор не доходил, и док не
//! выезжал. Здесь док — поверхность над окнами.
//!
//! Ведёт он себя по-прежнему: пока открыто хоть одно окно — своё или
//! программы машины, — он прячется и не отнимает у программы низ экрана; у
//! нижнего края лежит невидимая полоска в пару точек, и стоит курсору её
//! коснуться, док выезжает.
//!
//! Что в нём: закреплённые программы, запущенные окна системы, окна
//! программ машины и корзина. О своих окнах рассказывает стол («док»), о
//! чужих мы спрашиваем агента сами.

use crate::agent::{позови, скажи};
use crate::overview;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::time::Duration;

struct Док {
    окно: gtk::Window,
    ряд: gtk::Box,
    полоска: gtk::Window,
    стол: Value,
    чужие: Vec<Value>,
    вовесь: bool,
    занят: bool,
    у_курсора: bool,
    меню_открыто: bool,
    уход: Option<glib::SourceId>,
    слепок: String,
}

thread_local! {
    static ДОК: RefCell<Option<Док>> = const { RefCell::new(None) };
}

fn с_доком<R>(дело: impl FnOnce(&mut Док) -> R) -> Option<R> {
    ДОК.with(|д| д.borrow_mut().as_mut().map(дело))
}

/// Корзина — системная: её показывает Проводник. Нет его — просим стол
fn открой_корзину() {
    if std::process::Command::new("glower-files").arg("--trash").spawn().is_err() {
        дело_стола("запусти", json!({ "id": "trash", "вид": "приложение" }));
    }
}

fn дело_стола(что: &str, доводы: Value) {
    let mut тело = доводы;
    тело["что"] = json!(что);
    скажи("обзор-дело", тело);
}

// ------------------------------------------------------------- поверхности

fn прозрачное(окно: &gtk::Window) {
    окно.set_decorated(false);
    окно.set_app_paintable(true);
    if let Some(в) = gtk::prelude::GtkWindowExt::screen(окно).and_then(|э| э.rgba_visual()) {
        окно.set_visual(Some(&в));
    }
}

pub fn создай() {
    // Сам док: по центру у нижнего края, над окнами
    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    прозрачное(&окно);
    окно.init_layer_shell();
    окно.set_layer(Layer::Top);
    окно.set_namespace("glower-dock");
    окно.set_anchor(Edge::Bottom, true);
    окно.set_layer_shell_margin(Edge::Bottom, 10);
    // Места не держим: окна разворачиваются до самого низа, док над ними
    окно.set_exclusive_zone(0);
    окно.set_keyboard_mode(KeyboardMode::None);
    окно.connect_draw(|о, к| {
        // Островок со скруглёнными углами — рисуем сами: окно прозрачное
        let (ш, в) = (о.allocated_width() as f64, о.allocated_height() as f64);
        let р = 18.0;
        к.set_operator(gtk::cairo::Operator::Source);
        к.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        let _ = к.paint();
        к.set_operator(gtk::cairo::Operator::Over);
        к.new_sub_path();
        к.arc(ш - р, р, р, -std::f64::consts::FRAC_PI_2, 0.0);
        к.arc(ш - р, в - р, р, 0.0, std::f64::consts::FRAC_PI_2);
        к.arc(р, в - р, р, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
        к.arc(р, р, р, std::f64::consts::PI, 1.5 * std::f64::consts::PI);
        к.close_path();
        к.set_source_rgba(0.08, 0.09, 0.12, 0.84);
        let _ = к.fill_preserve();
        к.set_source_rgba(1.0, 1.0, 1.0, 0.10);
        к.set_line_width(1.0);
        let _ = к.stroke();
        glib::Propagation::Proceed
    });
    окно.style_context().add_class("dock");
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    ряд.set_margin_start(10);
    ряд.set_margin_end(10);
    ряд.set_margin_top(7);
    ряд.set_margin_bottom(5);
    окно.add(&ряд);
    окно.add_events(gtk::gdk::EventMask::ENTER_NOTIFY_MASK | gtk::gdk::EventMask::LEAVE_NOTIFY_MASK);
    окно.connect_enter_notify_event(|_, _| {
        с_доком(|д| {
            д.у_курсора = true;
            if let Some(у) = д.уход.take() {
                у.remove();
            }
        });
        glib::Propagation::Proceed
    });
    окно.connect_leave_notify_event(|_, с| {
        // Курсор ушёл на значок внутри дока — это не уход
        if с.detail() == gtk::gdk::NotifyType::Inferior {
            return glib::Propagation::Proceed;
        }
        с_доком(|д| {
            if let Some(у) = д.уход.take() {
                у.remove();
            }
            // Небольшая пауза: курсор, соскользнувший с края на миг, не
            // должен прятать док, к которому человек тянулся
            д.уход = Some(glib::timeout_add_local_once(Duration::from_millis(700), || {
                с_доком(|д| {
                    д.уход = None;
                    д.у_курсора = false;
                });
                обнови_видимость();
            }));
        });
        glib::Propagation::Proceed
    });

    // Полоска у нижнего края: невидимая, в пару точек, ловит курсор
    let полоска = gtk::Window::new(gtk::WindowType::Toplevel);
    прозрачное(&полоска);
    полоска.init_layer_shell();
    полоска.set_layer(Layer::Top);
    полоска.set_namespace("glower-dock-edge");
    for край in [Edge::Bottom, Edge::Left, Edge::Right] {
        полоска.set_anchor(край, true);
    }
    полоска.set_exclusive_zone(0);
    полоска.set_keyboard_mode(KeyboardMode::None);
    полоска.set_size_request(-1, 2);
    полоска.connect_draw(|_, к| {
        к.set_operator(gtk::cairo::Operator::Source);
        к.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        let _ = к.paint();
        glib::Propagation::Stop
    });
    полоска.add_events(gtk::gdk::EventMask::ENTER_NOTIFY_MASK);
    полоска.connect_enter_notify_event(|_, _| {
        с_доком(|д| д.у_курсора = true);
        обнови_видимость();
        glib::Propagation::Proceed
    });

    ДОК.with(|д| {
        *д.borrow_mut() = Some(Док {
            окно,
            ряд,
            полоска,
            стол: Value::Null,
            чужие: Vec::new(),
            вовесь: false,
            занят: false,
            у_курсора: false,
            меню_открыто: false,
            уход: None,
            слепок: String::new(),
        })
    });

    // Окна программ машины спрашиваем у агента сами: стол о них знает
    // только с опросом раз в три секунды, а нам хватает и двух
    опроси_чужие();
    glib::timeout_add_local(Duration::from_secs(2), || {
        опроси_чужие();
        glib::ControlFlow::Continue
    });
    перерисуй();
}

fn опроси_чужие() {
    позови("sys.windows", json!({}), |о| {
        let Ok(о) = о else { return };
        let чужие: Vec<Value> = о["list"]
            .as_array()
            .map(|с| с.iter().filter(|w| w["оболочка"].as_bool() != Some(true)).cloned().collect())
            .unwrap_or_default();
        let вовесь = о["вовесьЭкран"].as_bool() == Some(true);
        let менялось = с_доком(|д| {
            let было = (serde_json::to_string(&д.чужие).unwrap_or_default(), д.вовесь);
            д.чужие = чужие;
            д.вовесь = вовесь;
            было != (serde_json::to_string(&д.чужие).unwrap_or_default(), д.вовесь)
        });
        if менялось == Some(true) {
            перерисуй();
        }
    });
}

/// Стол рассказал о своих программах и окнах.
pub fn данные(что: &Value) {
    с_доком(|д| д.стол = что.clone());
    перерисуй();
}

/// Стол занят целиком — мастером, приветствием: дока не видно вовсе.
pub fn занят(да: bool) {
    с_доком(|д| д.занят = да);
    обнови_видимость();
}

fn обнови_видимость() {
    let Some((окно, полоска, видно, ловить)) = с_доком(|д| {
        let своих = д.стол["окон"].as_u64().unwrap_or(0) > 0;
        let чужих = д.чужие.iter().any(|w| w["состояние"]["свёрнуто"].as_bool() != Some(true));
        let можно = !д.занят && !д.вовесь;
        let видно = можно && (!(своих || чужих) || д.у_курсора || д.меню_открыто);
        (д.окно.clone(), д.полоска.clone(), видно, можно && !видно)
    }) else {
        return;
    };
    if видно {
        окно.show_all();
    } else {
        окно.hide();
    }
    if ловить {
        полоска.show_all();
    } else {
        полоска.hide();
    }
}

// ------------------------------------------------------------- значки

/// Значок окна программы машины: ярлык с тем же именем, затем значок из
/// темы по имени окна (у многих программ они совпадают), затем наш знак.
/// Программы системы на Rust: имя окна → программа страницы и её знак.
/// Их окна — настоящие, но для человека это тот же Блокнот, что закреплён
/// в доке: точка под его значком, а не ещё одно безликое окно рядом.
const РОДНЫЕ: [(&str, &str, &str, &str); 5] = [
    ("glower-notepad", "notepad", "📝", "linear-gradient(140deg,#7dd3fc,#3b82f6)"),
    ("glower-calc", "calc", "🧮", "linear-gradient(140deg,#a5b4fc,#6366f1)"),
    ("glower-files", "files", "📁", "linear-gradient(140deg,#fcd34d,#f59e0b)"),
    ("glower-settings", "settings", "⚙️", "linear-gradient(140deg,#94a3b8,#334155)"),
    ("glower-store", "store", "🛍️", "linear-gradient(140deg,#c4b5fd,#7c3aed)"),
];

fn родное(app_id: &str) -> Option<(&'static str, &'static str, &'static str)> {
    РОДНЫЕ.iter().find(|(окно, ..)| *окно == app_id).map(|(_, id, знак, фон)| (*id, *знак, *фон))
}

/// Переключить настоящее окно, как в любой панели задач: в работе —
/// свернуть, свёрнутое — достать, иначе — перейти к нему.
fn переключи_окно(w: &Value) {
    let с = &w["состояние"];
    let действие = if с["свёрнуто"].as_bool() == Some(true) {
        "restore"
    } else if с["активно"].as_bool() == Some(true) {
        "minimize"
    } else {
        "focus"
    };
    позови("sys.window", json!({ "action": действие, "appId": w["appId"], "title": w["title"] }), |_| {
        glib::timeout_add_local_once(Duration::from_millis(250), опроси_чужие);
    });
}

fn значок_чужого(app_id: &str) -> gtk::Widget {
    if let Some((_, знак, фон)) = родное(app_id) {
        return overview::наш_значок(знак, фон);
    }
    let нижнее = app_id.to_lowercase();
    for имя in [format!("{app_id}.desktop"), format!("{нижнее}.desktop")] {
        if let Some(ярлык) = gio::DesktopAppInfo::new(&имя) {
            if let Some(з) = ярлык.icon() {
                let к = gtk::Image::from_gicon(&з, gtk::IconSize::Dnd);
                к.set_pixel_size(36);
                return к.upcast();
            }
        }
    }
    if let Some(тема) = gtk::IconTheme::default() {
        for имя in [app_id, нижнее.as_str()] {
            if !имя.is_empty() && тема.has_icon(имя) {
                let к = gtk::Image::from_icon_name(Some(имя), gtk::IconSize::Dnd);
                к.set_pixel_size(36);
                return к.upcast();
            }
        }
    }
    overview::наш_значок("🪟", "rgba(255,255,255,0.12)")
}

/// Кнопка дока: значок и точка под ним, если программа запущена.
fn кнопка(значок: gtk::Widget, подсказка: &str, запущена: bool, в_работе: bool) -> gtk::Button {
    let б = gtk::Button::new();
    б.style_context().add_class("dock-item");
    if в_работе {
        б.style_context().add_class("active");
    }
    let столбик = gtk::Box::new(gtk::Orientation::Vertical, 3);
    значок.set_halign(gtk::Align::Center);
    значок.style_context().add_class("dock-ico");
    столбик.add(&значок);
    let точка = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    точка.set_halign(gtk::Align::Center);
    точка.set_size_request(5, 5);
    точка.style_context().add_class(if запущена { "dot" } else { "no-dot" });
    столбик.add(&точка);
    б.add(&столбик);
    б.set_tooltip_text(Some(подсказка));
    б
}

fn меню(пункты: Vec<(String, Box<dyn Fn()>)>, кнопка: &gtk::Button, событие: &gtk::gdk::EventButton) {
    let меню = gtk::Menu::new();
    for (текст, дело) in пункты {
        let п = gtk::MenuItem::with_label(&текст);
        п.connect_activate(move |_| дело());
        меню.append(&п);
    }
    меню.show_all();
    // Пока открыто меню, док не прячем: иначе оно повисло бы в пустоте
    с_доком(|д| д.меню_открыто = true);
    меню.connect_deactivate(|_| {
        // Пока меню держало курсор, ухода с дока мы не слышали — сбрасываем
        // сами. Если курсор и правда над доком, GTK, отпуская его, пришлёт
        // «вход» заново, и док останется
        с_доком(|д| {
            д.меню_открыто = false;
            д.у_курсора = false;
        });
        glib::timeout_add_local_once(Duration::from_millis(300), обнови_видимость);
    });
    let событие: &gtk::gdk::Event = событие;
    меню.popup_at_widget(кнопка, gtk::gdk::Gravity::North, gtk::gdk::Gravity::South, Some(событие));
}

fn по_правой<F>(б: &gtk::Button, дело: F)
where
    F: Fn(&gtk::Button, &gtk::gdk::EventButton) + 'static,
{
    б.connect_button_press_event(move |к, с| {
        if с.button() == 3 {
            дело(к, с);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
}

fn черта(ряд: &gtk::Box) {
    let ч = gtk::Separator::new(gtk::Orientation::Vertical);
    ч.style_context().add_class("dock-sep");
    ряд.add(&ч);
}

// ------------------------------------------------------------- рисование

fn перерисуй() {
    let Some((ряд, стол, чужие, слепок_был)) =
        с_доком(|д| (д.ряд.clone(), д.стол.clone(), д.чужие.clone(), д.слепок.clone()))
    else {
        return;
    };
    // Перерисовываем, только если что-то изменилось: опрос раз в две
    // секунды не должен заново собирать док каждый раз
    let слепок = format!("{}|{}", стол, serde_json::to_string(&чужие).unwrap_or_default());
    if слепок != слепок_был {
        с_доком(|д| д.слепок = слепок);
        for р in ряд.children() {
            ряд.remove(&р);
        }
        let запущены: Vec<String> = стол["запущены"]
            .as_array()
            .map(|с| с.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let активное = стол["активное"].as_str().unwrap_or("").to_string();
        let закреплены = стол["закреплены"].as_array().cloned().unwrap_or_default();
        let ids: Vec<String> = закреплены.iter().filter_map(|п| п["id"].as_str().map(String::from)).collect();

        // Окна программ системы на Rust — по программе страницы
        let родные_для = |id: &str| -> Vec<Value> {
            чужие.iter().filter(|w| w["appId"].as_str().and_then(родное).is_some_and(|(р, ..)| р == id)).cloned().collect()
        };
        // Закреплённые программы системы
        for п in &закреплены {
            let id = п["id"].as_str().unwrap_or("");
            добавь_своё(&ряд, п, true, &запущены, &активное, родные_для(id));
        }
        // Запущенные, но не закреплённые
        let лишние: Vec<Value> = стол["сведения"]
            .as_array()
            .map(|с| {
                с.iter()
                    .filter(|п| п["id"].as_str().is_some_and(|id| !ids.iter().any(|x| x == id)))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        // Окна программ машины. Родные, чей значок уже стоит выше, второй
        // раз не показываем
        let показаны: Vec<String> = ids.iter().cloned().chain(лишние.iter().filter_map(|п| п["id"].as_str().map(String::from))).collect();
        let свои_окна: Vec<&Value> = чужие
            .iter()
            .filter(|w| !w["appId"].as_str().and_then(родное).is_some_and(|(р, ..)| показаны.iter().any(|x| x == р)))
            .collect();
        if !лишние.is_empty() || !свои_окна.is_empty() {
            черта(&ряд);
        }
        for п in &лишние {
            let id = п["id"].as_str().unwrap_or("");
            добавь_своё(&ряд, п, false, &запущены, &активное, родные_для(id));
        }
        for w in свои_окна {
            добавь_чужое(&ряд, w);
        }
        черта(&ряд);
        let корзина = кнопка(overview::наш_значок("🗑️", "rgba(255,255,255,0.10)"), "Корзина", false, false);
        корзина.connect_clicked(|_| открой_корзину());
        по_правой(&корзина, |к, с| {
            меню(
                vec![
                    ("Открыть корзину".into(), Box::new(открой_корзину)),
                    ("Очистить корзину".into(), Box::new(|| дело_стола("очисти-корзину", json!({})))),
                ],
                к,
                с,
            )
        });
        ряд.add(&корзина);
        ряд.show_all();
    }
    обнови_видимость();
}

fn добавь_своё(ряд: &gtk::Box, п: &Value, закреплена: bool, запущены: &[String], активное: &str, родные: Vec<Value>) {
    let id = п["id"].as_str().unwrap_or("").to_string();
    if id.is_empty() {
        return;
    }
    let имя = п["имя"].as_str().unwrap_or(&id).to_string();
    let на_странице = запущены.iter().any(|x| x == &id);
    let идёт = на_странице || !родные.is_empty();
    let в_работе = (на_странице && активное == id)
        || родные.iter().any(|w| w["состояние"]["активно"].as_bool() == Some(true));
    let б = кнопка(
        overview::наш_значок(п["знак"].as_str().unwrap_or(""), п["фон"].as_str().unwrap_or("")),
        &имя,
        идёт,
        в_работе,
    );
    {
        let id = id.clone();
        let первое = (!на_странице).then(|| родные.first().cloned()).flatten();
        // Как и прежде: не запущена — открыть, единственное окно в работе —
        // свернуть, иначе — показать. Окно на странице решает стол
        // (Shell.launch), настоящее окно — мы, как любое окно машины
        б.connect_clicked(move |_| match &первое {
            Some(w) => переключи_окно(w),
            None => дело_стола("запусти", json!({ "id": id, "вид": "приложение" })),
        });
    }
    по_правой(&б, move |к, с| {
        let mut пункты: Vec<(String, Box<dyn Fn()>)> = Vec::new();
        let i1 = id.clone();
        пункты.push(("Открыть".into(), Box::new(move || дело_стола("запусти", json!({ "id": i1, "вид": "приложение" })))));
        if идёт {
            let i2 = id.clone();
            let родные = родные.clone();
            пункты.push((
                "Закрыть все окна".into(),
                Box::new(move || {
                    дело_стола("закрой-все", json!({ "id": i2 }));
                    for w in &родные {
                        позови("sys.window", json!({ "action": "close", "appId": w["appId"], "title": w["title"] }), |_| {});
                    }
                }),
            ));
        }
        let i3 = id.clone();
        пункты.push((
            if закреплена { "Убрать из дока" } else { "Закрепить в доке" }.into(),
            Box::new(move || дело_стола("док", json!({ "id": i3, "вид": "приложение" }))),
        ));
        меню(пункты, к, с);
    });
    ряд.add(&б);
}

fn добавь_чужое(ряд: &gtk::Box, w: &Value) {
    let app_id = w["appId"].as_str().unwrap_or("").to_string();
    let заголовок = w["title"].as_str().unwrap_or("").to_string();
    let с = &w["состояние"];
    let свёрнуто = с["свёрнуто"].as_bool() == Some(true);
    let активно = с["активно"].as_bool() == Some(true);
    let подпись = if заголовок.is_empty() { app_id.clone() } else { заголовок.clone() };
    let б = кнопка(значок_чужого(&app_id), &подпись, true, активно);
    if свёрнуто {
        б.style_context().add_class("minimized");
    }
    {
        let (app_id, заголовок) = (app_id.clone(), заголовок.clone());
        // Как в любой панели задач: в работе — свернуть, свёрнутое —
        // достать, иначе — переключиться
        б.connect_clicked(move |_| {
            let действие = if свёрнуто { "restore" } else if активно { "minimize" } else { "focus" };
            позови("sys.window", json!({ "action": действие, "appId": app_id, "title": заголовок }), |_| {
                glib::timeout_add_local_once(Duration::from_millis(250), опроси_чужие);
            });
        });
    }
    по_правой(&б, move |к, с| {
        let (окно_id, окно_имя) = (app_id.clone(), заголовок.clone());
        меню(
            vec![(
                "Закрыть окно".into(),
                Box::new(move || {
                    позови("sys.window", json!({ "action": "close", "appId": окно_id, "title": окно_имя }), |_| {
                        glib::timeout_add_local_once(Duration::from_millis(300), опроси_чужие);
                    })
                }),
            )],
            к,
            с,
        )
    });
    ряд.add(&б);
}
