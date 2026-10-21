//! Рабочий стол: обои и значки — нативные, в той же программе, что панель.
//!
//! Лежит в самом низу, под всеми окнами. Страница стола (окна системы,
//! меню, диалоги) лежит над нами прозрачной и пропускает к нам щелчки там,
//! где у неё пусто.
//!
//! Что лежит на столе, где стоит и как открывается — знает страница:
//! она рассказывает («значки», «обои»). Мы рисуем, выделяем, двигаем и
//! ведём рамку выделения, а за действием — открыть, показать меню, запомнить
//! новое место — идём к ней («значки-дело»). Так все прежние меню стола —
//! создать файл и папку, виджеты, обои — остаются на месте как были.
//!
//! Устроено нарочно просто: окно, в нём неподвижное поле со значками, и
//! все нажатия ловит само окно. Какой значок под курсором, мы считаем сами
//! по местам значков. Вложенные «ящики событий» GTK здесь не годились:
//! у каждого своё окошко ввода, и щелчки доставались не тому, кто их ждал, —
//! стол просто не слышал нажатий.

use crate::agent::{позови, скажи};
use base64::Engine;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use serde_json::{json, Map, Value};
use std::cell::RefCell;
use std::rc::Rc;

/// Значок на столе.
struct Значок {
    имя: String,
    вид: gtk::Box,
    x: f64,
    y: f64,
}

/// Что сейчас делает мышь на столе.
enum Жест {
    Нет,
    /// Тянем выделенные значки: откуда начали и где они стояли.
    Тянем { x0: f64, y0: f64, где: Vec<(String, f64, f64)>, сдвинули: bool },
    /// Ведём рамку выделения от угла.
    Рамка { x0: f64, y0: f64, x1: f64, y1: f64 },
}

struct Стол {
    обои: gtk::Window,
    поле: gtk::Fixed,
    значки: Vec<Значок>,
    выбор: Vec<String>,
    слепок: String,
    обои_css: String,
    стиль_обоев: Option<gtk::CssProvider>,
    жест: Жест,
}

thread_local! {
    static СТОЛ: RefCell<Option<Rc<RefCell<Стол>>>> = const { RefCell::new(None) };
}

fn стол() -> Option<Rc<RefCell<Стол>>> {
    СТОЛ.with(|с| с.borrow().clone())
}

fn дело(что: &str, доводы: Value) {
    let mut тело = доводы;
    тело["что"] = json!(что);
    скажи("значки-дело", тело);
}

/// Клавиатура после щелчка по столу — странице стола: Enter, F2 и Delete
/// для выделенного обрабатывает она, и её диалоги ждут ввода.
fn клавиатуру_столу() {
    позови("sys.window", json!({ "action": "focus", "appId": "glowershell" }), |_| {});
}

fn отладка() -> bool {
    std::env::var_os("GLOWER_DEBUG").is_some()
}

// ------------------------------------------------------------- обои

/// GTK3 не знает восьмизначных цветов (#rrggbbaa) — переводим в rgba().
fn цвета_для_gtk(css: &str) -> String {
    let mut итог = String::with_capacity(css.len());
    let знаки: Vec<char> = css.chars().collect();
    let mut i = 0;
    while i < знаки.len() {
        if знаки[i] == '#' {
            let mut j = i + 1;
            while j < знаки.len() && знаки[j].is_ascii_hexdigit() {
                j += 1;
            }
            let hex: String = знаки[i + 1..j].iter().collect();
            let два = |s: &str| u8::from_str_radix(s, 16).unwrap_or(0);
            let раз = |c: char| {
                let v = c.to_digit(16).unwrap_or(0) as u8;
                v * 16 + v
            };
            let rgba = match hex.len() {
                8 => Some((два(&hex[0..2]), два(&hex[2..4]), два(&hex[4..6]), два(&hex[6..8]))),
                4 => {
                    let c: Vec<char> = hex.chars().collect();
                    Some((раз(c[0]), раз(c[1]), раз(c[2]), раз(c[3])))
                }
                _ => None,
            };
            match rgba {
                Some((r, g, b, a)) => итог.push_str(&format!("rgba({r},{g},{b},{:.3})", a as f64 / 255.0)),
                None => {
                    итог.push('#');
                    итог.push_str(&hex);
                }
            }
            i = j;
        } else {
            итог.push(знаки[i]);
            i += 1;
        }
    }
    итог
}

/// Картинку обоев GTK по адресу http не возьмёт — кладём её в файл рядом.
fn картинка_в_файл(адрес: &str) -> Option<String> {
    let папка = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let путь = format!("{папка}/glower-oboi");
    let байты: Vec<u8> = if let Some((_, данные)) = адрес.split_once(";base64,") {
        base64::engine::general_purpose::STANDARD.decode(данные.trim()).ok()?
    } else if адрес.starts_with("http://") || адрес.starts_with("https://") {
        let mut тело = Vec::new();
        let ответ = ureq::get(адрес).timeout(std::time::Duration::from_secs(20)).call().ok()?;
        std::io::Read::read_to_end(&mut ответ.into_reader(), &mut тело).ok()?;
        тело
    } else if let Some(п) = адрес.strip_prefix("file://") {
        return Some(п.to_string());
    } else {
        return None;
    };
    std::fs::write(&путь, байты).ok()?;
    Some(путь)
}

fn поставь_обои(css: &str) {
    let Some(с) = стол() else { return };
    if с.borrow().обои_css == css {
        return;
    }
    с.borrow_mut().обои_css = css.to_string();

    // Картинка: url(...) — из сети агента или прямо в строке
    let адрес = css.trim().strip_prefix("url(").and_then(|х| х.strip_suffix(')')).map(|х| х.trim_matches(|c| c == '"' || c == '\'').to_string());
    if let Some(адрес) = адрес {
        let (куда, откуда) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let _ = куда.send_blocking(картинка_в_файл(&адрес));
        });
        glib::MainContext::default().spawn_local(async move {
            if let Ok(Some(путь)) = откуда.recv().await {
                let путь = путь.replace('"', "%22");
                примени_стиль(&format!(
                    "window.desktop {{ background-image: url(\"file://{путь}\"); background-size: cover; background-position: center; }}"
                ));
            }
        });
        return;
    }
    примени_стиль(&format!("window.desktop {{ background-image: {}; }}", цвета_для_gtk(css)));
}

fn примени_стиль(css: &str) {
    let Some(с) = стол() else { return };
    let новый = gtk::CssProvider::new();
    if новый.load_from_data(css.as_bytes()).is_err() {
        // Не поняли — ставим тёмный фон того же тона, что у загрузки, а не
        // оставляем белое пятно
        let _ = новый.load_from_data(b"window.desktop { background-image: linear-gradient(#0b0d1a, #05060c); }");
    }
    let mut с = с.borrow_mut();
    let контекст = с.обои.style_context();
    if let Some(старый) = с.стиль_обоев.take() {
        контекст.remove_provider(&старый);
    }
    контекст.add_provider(&новый, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2);
    с.стиль_обоев = Some(новый);
}


// ------------------------------------------------------------- поверхность

pub fn создай() {
    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_decorated(false);
    окно.style_context().add_class("desktop");
    окно.init_layer_shell();
    окно.set_layer(Layer::Background);
    окно.set_namespace("glower-desktop");
    for край in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        окно.set_anchor(край, true);
    }
    // Обои — во весь экран, под панелью тоже: она прозрачна
    окно.set_exclusive_zone(-1);
    окно.set_keyboard_mode(KeyboardMode::None);
    окно.add_events(
        gtk::gdk::EventMask::BUTTON_PRESS_MASK
            | gtk::gdk::EventMask::BUTTON_RELEASE_MASK
            | gtk::gdk::EventMask::BUTTON_MOTION_MASK,
    );

    let поле = gtk::Fixed::new();
    // Рамку выделения рисуем под значками, прямо на поле
    поле.connect_draw(|_, к| {
        if let Some(с) = стол() {
            if let Жест::Рамка { x0, y0, x1, y1 } = с.borrow().жест {
                let (x, y, ш, в) = (x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs());
                к.rectangle(x + 0.5, y + 0.5, ш, в);
                к.set_source_rgba(0.36, 0.58, 0.96, 0.22);
                let _ = к.fill_preserve();
                к.set_source_rgba(0.55, 0.72, 1.0, 0.85);
                к.set_line_width(1.0);
                let _ = к.stroke();
            }
        }
        glib::Propagation::Proceed
    });
    окно.add(&поле);

    окно.connect_button_press_event(|_, е| {
        нажатие(е);
        glib::Propagation::Stop
    });
    окно.connect_motion_notify_event(|_, е| {
        движение(е.position());
        glib::Propagation::Stop
    });
    окно.connect_button_release_event(|_, _| {
        отпускание();
        glib::Propagation::Stop
    });

    let с = Rc::new(RefCell::new(Стол {
        обои: окно.clone(),
        поле,
        значки: Vec::new(),
        выбор: Vec::new(),
        слепок: String::new(),
        обои_css: String::new(),
        стиль_обоев: None,
        жест: Жест::Нет,
    }));
    СТОЛ.with(|я| *я.borrow_mut() = Some(с));
    примени_стиль("window.desktop { background-image: linear-gradient(#0b0d1a, #05060c); }");
    окно.show_all();
}

/// Какой значок под точкой — по его месту и размеру.
fn под_курсором(x: f64, y: f64) -> Option<String> {
    let с = стол()?;
    let с = с.borrow();
    с.значки
        .iter()
        .rev()
        .find(|з| {
            let (ш, в) = (з.вид.allocated_width() as f64, з.вид.allocated_height() as f64);
            x >= з.x && x <= з.x + ш && y >= з.y && y <= з.y + в
        })
        .map(|з| з.имя.clone())
}

fn нажатие(е: &gtk::gdk::EventButton) {
    let (x, y) = е.position();
    if отладка() {
        eprintln!("glower-panel: стол: нажатие {} в {x:.0},{y:.0}", е.button());
    }
    let Some(с) = стол() else { return };
    // GTK шлёт и одиночное нажатие, и «двойное» вдогонку — двойное
    // означает «открыть», и только для значка
    if е.event_type() == gtk::gdk::EventType::DoubleButtonPress {
        if е.button() == 1 {
            if let Some(имя) = под_курсором(x, y) {
                с.borrow_mut().жест = Жест::Нет;
                дело("открой", json!({ "n": имя }));
            }
        }
        return;
    }
    if е.event_type() != gtk::gdk::EventType::ButtonPress {
        return;
    }
    клавиатуру_столу();
    let значок = под_курсором(x, y);

    if е.button() == 3 {
        match значок {
            Some(имя) => {
                if !с.borrow().выбор.contains(&имя) {
                    выдели(vec![имя.clone()]);
                }
                дело("меню", json!({ "n": имя, "x": x.round(), "y": y.round() }));
            }
            // По пустому месту — меню стола, то же, что было всегда
            None => дело("меню-стола", json!({ "x": x.round(), "y": y.round() })),
        }
        return;
    }
    if е.button() != 1 {
        return;
    }
    match значок {
        Some(имя) => {
            let выбран = с.borrow().выбор.contains(&имя);
            if е.state().contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                let mut выбор = с.borrow().выбор.clone();
                if выбран {
                    выбор.retain(|x| x != &имя);
                } else {
                    выбор.push(имя.clone());
                }
                выдели(выбор);
            } else if !выбран {
                выдели(vec![имя.clone()]);
            }
            let где: Vec<(String, f64, f64)> = {
                let с_ = с.borrow();
                с_.значки.iter().filter(|з| с_.выбор.contains(&з.имя)).map(|з| (з.имя.clone(), з.x, з.y)).collect()
            };
            с.borrow_mut().жест = Жест::Тянем { x0: x, y0: y, где, сдвинули: false };
        }
        None => {
            выдели(Vec::new());
            с.borrow_mut().жест = Жест::Рамка { x0: x, y0: y, x1: x, y1: y };
        }
    }
}

fn движение((x, y): (f64, f64)) {
    let Some(с) = стол() else { return };
    let mut с_ = с.borrow_mut();
    let поле = с_.поле.clone();
    match &mut с_.жест {
        Жест::Тянем { x0, y0, где, сдвинули } => {
            let (dx, dy) = (x - *x0, y - *y0);
            if !*сдвинули && dx.hypot(dy) < 5.0 {
                return;
            }
            *сдвинули = true;
            let новые: Vec<(String, f64, f64)> =
                где.iter().map(|(n, ix, iy)| (n.clone(), (ix + dx).max(0.0), (iy + dy).max(30.0))).collect();
            for з in с_.значки.iter_mut() {
                if let Some((_, нx, нy)) = новые.iter().find(|(n, _, _)| n == &з.имя) {
                    поле.move_(&з.вид, *нx as i32, *нy as i32);
                    з.x = *нx;
                    з.y = *нy;
                }
            }
        }
        Жест::Рамка { x0, y0, x1, y1 } => {
            *x1 = x;
            *y1 = y;
            let (лево, верх, право, низ) = (x0.min(x), y0.min(y), x0.max(x), y0.max(y));
            let выбранные: Vec<String> = с_
                .значки
                .iter()
                .filter(|з| {
                    let (ш, в) = (з.вид.allocated_width() as f64, з.вид.allocated_height() as f64);
                    !(з.x + ш < лево || з.x > право || з.y + в < верх || з.y > низ)
                })
                .map(|з| з.имя.clone())
                .collect();
            drop(с_);
            поле.queue_draw();
            выдели_молча(выбранные);
        }
        Жест::Нет => {}
    }
}

fn отпускание() {
    let Some(с) = стол() else { return };
    let жест = std::mem::replace(&mut с.borrow_mut().жест, Жест::Нет);
    match жест {
        // Новое место запоминает страница — как и прежде, в iconPos
        Жест::Тянем { где, сдвинули: true, .. } => {
            let с_ = с.borrow();
            let mut места = Map::new();
            for (n, _, _) in &где {
                if let Some(з) = с_.значки.iter().find(|з| &з.имя == n) {
                    места.insert(n.clone(), json!({ "x": з.x.round(), "y": з.y.round() }));
                }
            }
            drop(с_);
            дело("позиции", json!({ "позиции": Value::Object(места) }));
        }
        Жест::Рамка { .. } => {
            с.borrow().поле.queue_draw();
            let выбор = с.borrow().выбор.clone();
            дело("выбор", json!({ "имена": выбор }));
        }
        _ => {}
    }
}

// ------------------------------------------------------------- выделение

fn выдели_молча(имена: Vec<String>) {
    let Some(с) = стол() else { return };
    let mut с = с.borrow_mut();
    for з in &с.значки {
        if имена.contains(&з.имя) {
            з.вид.style_context().add_class("sel");
        } else {
            з.вид.style_context().remove_class("sel");
        }
    }
    с.выбор = имена;
}

fn выдели(имена: Vec<String>) {
    выдели_молча(имена.clone());
    дело("выбор", json!({ "имена": имена }));
}

// ------------------------------------------------------------- значки

fn значок_картинкой(з: &Value) -> gtk::Widget {
    let вид = з["вид"].as_str().unwrap_or("");
    let ярлык = з["ярлык"].as_str().unwrap_or("");
    if вид == "машина" && !ярлык.is_empty() {
        if let Some(п) = gio::DesktopAppInfo::new(ярлык) {
            if let Some(и) = п.icon() {
                let к = gtk::Image::from_gicon(&и, gtk::IconSize::Dialog);
                к.set_pixel_size(48);
                return к.upcast();
            }
        }
    }
    let знак = з["g"].as_str().unwrap_or("");
    let л = gtk::Label::new(Some(if знак.is_empty() {
        match вид {
            "папка" => "📁",
            "файл" => "📄",
            _ => "▢",
        }
    } else {
        знак
    }));
    л.style_context().add_class("di-glyph");
    л.upcast()
}

/// Стол рассказал, что на нём лежит и где.
pub fn значки(список: &Value) {
    let Some(с) = стол() else { return };
    // Пока тянем значок, не перестраиваем: место ему ещё не записано
    if matches!(с.borrow().жест, Жест::Тянем { .. }) {
        return;
    }
    let слепок = список.to_string();
    if с.borrow().слепок == слепок {
        return;
    }
    let поле = с.borrow().поле.clone();
    for р in поле.children() {
        поле.remove(&р);
    }
    let выбор = с.borrow().выбор.clone();
    let mut значки = Vec::new();
    for з in список.as_array().cloned().unwrap_or_default() {
        let имя = з["n"].as_str().unwrap_or("").to_string();
        if имя.is_empty() {
            continue;
        }
        let (x, y) = (з["x"].as_f64().unwrap_or(0.0), з["y"].as_f64().unwrap_or(0.0));
        let вид = gtk::Box::new(gtk::Orientation::Vertical, 4);
        вид.style_context().add_class("di");
        if выбор.contains(&имя) {
            вид.style_context().add_class("sel");
        }
        вид.set_size_request(92, -1);
        let картинка = значок_картинкой(&з);
        картинка.set_halign(gtk::Align::Center);
        вид.add(&картинка);
        let подпись = gtk::Label::new(Some(&имя));
        подпись.style_context().add_class("di-label");
        подпись.set_line_wrap(true);
        подпись.set_line_wrap_mode(gtk::pango::WrapMode::WordChar);
        подпись.set_lines(2);
        подпись.set_ellipsize(gtk::pango::EllipsizeMode::End);
        подпись.set_max_width_chars(12);
        подпись.set_justify(gtk::Justification::Center);
        вид.add(&подпись);
        поле.put(&вид, x as i32, y as i32);
        значки.push(Значок { имя, вид, x, y });
    }
    {
        let mut с_ = с.borrow_mut();
        с_.значки = значки;
        с_.слепок = слепок;
    }
    поле.show_all();
}

/// Стол рассказал, какие у него обои.
pub fn обои(что: &Value) {
    if let Some(css) = что["css"].as_str() {
        поставь_обои(css);
    }
}

#[cfg(test)]
mod проверки {
    use super::цвета_для_gtk;

    #[test]
    fn восьмизначные_цвета_переводятся() {
        assert_eq!(
            цвета_для_gtk("radial-gradient(50% 50% at 80% 70%,#ff4d9d33,transparent 70%)"),
            "radial-gradient(50% 50% at 80% 70%,rgba(255,77,157,0.200),transparent 70%)"
        );
        assert_eq!(цвета_для_gtk("linear-gradient(#1b2554 0%,#fff 100%)"), "linear-gradient(#1b2554 0%,#fff 100%)");
        assert_eq!(цвета_для_gtk("#f008"), "rgba(255,0,0,0.533)");
    }
}
