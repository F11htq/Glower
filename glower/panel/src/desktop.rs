//! Рабочий стол: обои, значки и виджеты — нативные, в той же программе, что
//! панель. Лежит в самом низу, под всеми окнами.
//!
//! Что лежит на столе и где, рассказывает ядро («значки», «обои»): папка
//! «Рабочий стол», вынесенные программы и запомненные места. Стол рисует,
//! выделяет, двигает, ведёт рамку выделения — и сам открывает, показывает
//! меню, переименовывает, удаляет в корзину, создаёт папки и файлы.
//!
//! Устроено нарочно просто: окно, в нём неподвижное поле со значками и
//! виджетами, и все нажатия ловит само окно. Какой значок под курсором, мы
//! считаем сами по местам значков. Вложенные «ящики событий» GTK здесь не
//! годились: у каждого своё окошко ввода, и щелчки доставались не тому,
//! кто их ждал, — стол просто не слышал нажатий.

use crate::core;
use crate::notify;
use base64::Engine;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

/// Значок на столе.
struct Значок {
    имя: String,
    вид: gtk::Box,
    x: f64,
    y: f64,
    данные: Value,
}

/// Что сейчас делает мышь на столе.
enum Жест {
    Нет,
    /// Тянем выделенные значки: откуда начали и где они стояли.
    Тянем { x0: f64, y0: f64, где: Vec<(String, f64, f64)>, сдвинули: bool },
    /// Ведём рамку выделения от угла.
    Рамка { x0: f64, y0: f64, x1: f64, y1: f64 },
    /// Тянем виджет: какой, где за него взялись внутри и откуда начали
    Виджет { номер: usize, dx: f64, dy: f64, x0: f64, y0: f64, сдвинули: bool },
}

struct Стол {
    обои: gtk::Window,
    поле: gtk::Fixed,
    значки: Vec<Значок>,
    выбор: Vec<String>,
    слепок: String,
    обои_css: String,
    стиль_обоев: Option<gtk::CssProvider>,
    /// Обои, уже нарисованные в картинку размером с окно
    кэш_обоев: Option<(i32, i32, gtk::cairo::ImageSurface)>,
    жест: Жест,
    виджеты: gtk::Box,
    /// Все показанные виджеты по порядку: и в колонке у края, и
    /// перенесённые человеком в своё место на столе
    все_виджеты: Vec<gtk::Box>,
    слепок_виджетов: String,
}

thread_local! {
    static СТОЛ: RefCell<Option<Rc<RefCell<Стол>>>> = const { RefCell::new(None) };
}

fn стол() -> Option<Rc<RefCell<Стол>>> {
    СТОЛ.with(|с| с.borrow().clone())
}

fn отладка() -> bool {
    std::env::var_os("GLOWER_DEBUG").is_some()
}


// ------------------------------------------------------------- обои

/// GTK3 не знает восьмизначных цветов (#rrggbbaa) — переводим в rgba().
pub(crate) fn цвета_для_gtk(css: &str) -> String {
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

/// Размер экрана в настоящих точках — под него готовим картинку обоев
fn размер_экрана() -> (i32, i32) {
    let Some(дисплей) = gtk::gdk::Display::default() else { return (1920, 1080) };
    let Some(м) = дисплей.monitor(0) else { return (1920, 1080) };
    let г = м.geometry();
    let к = м.scale_factor().max(1);
    ((г.width() * к).max(1), (г.height() * к).max(1))
}

/// Обои заранее ужимаем ровно под экран. GTK рисует «cover» сам, но делает
/// это на каждой перерисовке: фото 4000×3000 пересчитывается целиком на
/// каждый шаг мыши, и рамка выделения на столе тормозила. Картинка размером
/// с экран рисуется один к одному — это просто копирование.
fn подгони_под_экран(путь: &str, (ш, в): (i32, i32)) -> Option<String> {
    use std::hash::{Hash, Hasher};
    let мета = std::fs::metadata(путь).ok()?;
    let mut х = std::collections::hash_map::DefaultHasher::new();
    (путь, мета.len(), мета.modified().ok(), ш, в).hash(&mut х);
    let папка = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let куда = format!("{папка}/glower-oboi-{:016x}.jpg", х.finish());
    if std::path::Path::new(&куда).exists() {
        return Some(куда);
    }
    let к = gtk::gdk_pixbuf::Pixbuf::from_file(путь).ok()?;
    let (иш, ив) = (к.width() as f64, к.height() as f64);
    if иш < 1.0 || ив < 1.0 {
        return None;
    }
    let м = (ш as f64 / иш).max(в as f64 / ив);
    let (нш, нв) = (((иш * м).ceil() as i32).max(ш), ((ив * м).ceil() as i32).max(в));
    let большая = к.scale_simple(нш, нв, gtk::gdk_pixbuf::InterpType::Bilinear)?;
    let кусок = большая.new_subpixbuf((нш - ш) / 2, (нв - в) / 2, ш, в);
    кусок.savev(&куда, "jpeg", &[("quality", "92")]).ok()?;
    // Прежние подогнанные копии больше не нужны
    if let Ok(список) = std::fs::read_dir(&папка) {
        for ф in список.flatten() {
            let имя = ф.file_name().to_string_lossy().to_string();
            if имя.starts_with("glower-oboi-") && ф.path().to_string_lossy() != куда {
                let _ = std::fs::remove_file(ф.path());
            }
        }
    }
    Some(куда)
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
        let экран = размер_экрана();
        let (куда, откуда) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let путь = картинка_в_файл(&адрес);
            let _ = куда.send_blocking(путь.map(|п| подгони_под_экран(&п, экран).unwrap_or(п)));
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
    с.кэш_обоев = None;
    с.обои.queue_draw();
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
    // Клавиатура — по щелчку: Enter, F2, Delete для выделенного и заметка
    окно.set_keyboard_mode(KeyboardMode::OnDemand);
    окно.add_events(
        gtk::gdk::EventMask::BUTTON_PRESS_MASK
            | gtk::gdk::EventMask::BUTTON_RELEASE_MASK
            | gtk::gdk::EventMask::BUTTON_MOTION_MASK
            | gtk::gdk::EventMask::KEY_PRESS_MASK,
    );

    // Обои рисуем сами, из готовой картинки. Фон стола — градиент в
    // несколько слоёв или фото — GTK пересчитывал на каждую перерисовку, и
    // рамка выделения, перерисовывая экран на каждый шаг мыши, тормозила.
    // Теперь фон считается один раз (и заново — при смене обоев или
    // размера), а дальше только копируется
    окно.set_app_paintable(true);
    окно.connect_draw(|о, к| {
        let (ш, в) = (о.allocated_width(), о.allocated_height());
        let Some(с) = стол() else { return glib::Propagation::Proceed };
        let Ok(mut с) = с.try_borrow_mut() else { return glib::Propagation::Proceed };
        let годен = с.кэш_обоев.as_ref().is_some_and(|(кш, кв, _)| *кш == ш && *кв == в);
        if !годен {
            с.кэш_обоев = None;
            // На экране с увеличением — в настоящих точках, иначе обои мылятся
            let м = о.scale_factor().max(1);
            if let Ok(картинка) = gtk::cairo::ImageSurface::create(gtk::cairo::Format::Rgb24, ш.max(1) * м, в.max(1) * м) {
                картинка.set_device_scale(м as f64, м as f64);
                if let Ok(ик) = gtk::cairo::Context::new(&картинка) {
                    gtk::render_background(&о.style_context(), &ик, 0.0, 0.0, ш as f64, в as f64);
                }
                картинка.flush();
                с.кэш_обоев = Some((ш, в, картинка));
            }
        }
        if let Some((_, _, картинка)) = с.кэш_обоев.as_ref() {
            if к.set_source_surface(картинка, 0.0, 0.0).is_ok() {
                let _ = к.paint();
            }
        }
        glib::Propagation::Proceed
    });

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
    let виджеты = gtk::Box::new(gtk::Orientation::Vertical, 14);
    виджеты.style_context().add_class("widgets");
    поле.put(&виджеты, 0, 0);
    // Виджеты — у правого края: место узнаём, когда окно получит размер.
    // Двигаем после раскладки, а не внутри неё: изнутри GTK сдвиг
    // откладывает до следующей, а её может и не быть — если оконный сервер
    // сразу дал окну полный размер, виджеты оставались слева, на значках
    окно.connect_size_allocate(|о, _| {
        let ш = о.allocated_width();
        glib::idle_add_local_once(move || {
            if let Some(с) = стол() {
                let с = с.borrow();
                let x = (ш - 270).max(0);
                if с.поле.child_x(&с.виджеты) != x {
                    с.поле.move_(&с.виджеты, x, 44);
                }
            }
        });
    });

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
    окно.connect_key_press_event(|_, е| клавиша(е));

    let с = Rc::new(RefCell::new(Стол {
        обои: окно.clone(),
        поле,
        значки: Vec::new(),
        выбор: Vec::new(),
        слепок: String::new(),
        обои_css: String::new(),
        стиль_обоев: None,
        кэш_обоев: None,
        жест: Жест::Нет,
        виджеты,
        все_виджеты: Vec::new(),
        слепок_виджетов: String::new(),
    }));
    СТОЛ.with(|я| *я.borrow_mut() = Some(с));
    примени_стиль("window.desktop { background-image: linear-gradient(#0b0d1a, #05060c); }");
    окно.show_all();
    // Часы и система на виджетах — живые
    glib::timeout_add_local(Duration::from_secs(1), || {
        тик_виджетов();
        glib::ControlFlow::Continue
    });
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

/// Над виджетами стол своими жестами не распоряжается
/// Который по счёту виджет под курсором
fn виджет_под(x: f64, y: f64) -> Option<usize> {
    let с = стол()?;
    let с = с.borrow();
    с.все_виджеты.iter().position(|в| {
        let а = в.allocation();
        в.is_visible() && x >= а.x() as f64 && x <= (а.x() + а.width()) as f64 && y >= а.y() as f64 && y <= (а.y() + а.height()) as f64
    })
}

/// Куда ведёт виджет: погода — на страницу с прогнозом для своего города,
/// остальные — в свою программу. Заметку пишут прямо на столе, ей некуда
fn открой_виджет(t: &str) {
    match t {
        "weather" => {
            let город = core::погода()["город"].as_str().map(String::from)
                .or_else(|| core::настройка("city").as_str().map(String::from))
                .unwrap_or_else(|| "Москва".into());
            let адрес = format!("https://www.google.com/search?q={}", glib::Uri::escape_string(&format!("погода {город}"), None, false));
            if gio::AppInfo::launch_default_for_uri(&адрес, None::<&gio::AppLaunchContext>).is_err() {
                core::запусти("browser", "приложение");
            }
        }
        "clock" => core::запусти("clock", "приложение"),
        "calendar" => core::запусти("calendar", "приложение"),
        "todo" => core::запусти("todo", "приложение"),
        "system" => core::запусти("taskmgr", "приложение"),
        _ => {}
    }
}

/// Записи показанных виджетов: только с полем «t», как и при показе
fn записи_виджетов() -> (Vec<Value>, Vec<usize>) {
    let список: Vec<Value> = core::настройка("deskWidgets").as_array().cloned().unwrap_or_default();
    let где = список.iter().enumerate().filter(|(_, в)| в["t"].is_string()).map(|(i, _)| i).collect();
    (список, где)
}

/// Запомнить место виджета (или забыть — тогда он вернётся в колонку)
fn место_виджета(н: usize, место: Option<(f64, f64)>) {
    let (mut список, где) = записи_виджетов();
    let Some(&i) = где.get(н) else { return };
    let Some(запись) = список[i].as_object_mut() else { return };
    match место {
        Some((x, y)) => {
            запись.insert("x".into(), json!(x.round()));
            запись.insert("y".into(), json!(y.round()));
        }
        None => {
            запись.remove("x");
            запись.remove("y");
        }
    }
    core::задай("deskWidgets", Value::Array(список));
}

/// Меню над виджетом. Раньше его не было вовсе: виджет, однажды
/// добавленный, со стола было не убрать ничем, кроме правки настроек руками
fn меню_виджета(е: &gtk::gdk::EventButton, номер: Option<usize>) {
    let меню = gtk::Menu::new();
    if let Some(н) = номер {
        пункт(&меню, "Убрать виджет", move || убери_виджет(н));
        let (список, где) = записи_виджетов();
        if где.get(н).is_some_and(|&i| список[i]["x"].is_number()) {
            пункт(&меню, "Вернуть к краю экрана", move || место_виджета(н, None));
        }
    }
    пункт(&меню, "Скрыть все виджеты", || core::задай("showDeskWidgets", json!(false)));
    меню.show_all();
    let событие: &gtk::gdk::Event = е;
    меню.popup_at_pointer(Some(событие));
}

/// Убрать н-й из показанных виджетов. Показаны только записи с полем «t»,
/// поэтому и считаем только их — иначе ушёл бы соседний
fn убери_виджет(н: usize) {
    let mut список: Vec<Value> = core::настройка("deskWidgets").as_array().cloned().unwrap_or_default();
    let Some(где) = список.iter().enumerate().filter(|(_, в)| в["t"].is_string()).map(|(i, _)| i).nth(н) else { return };
    список.remove(где);
    core::задай("deskWidgets", Value::Array(список));
}

fn данные_значка(имя: &str) -> Option<Value> {
    let с = стол()?;
    let с = с.borrow();
    с.значки.iter().find(|з| з.имя == имя).map(|з| з.данные.clone())
}

fn нажатие(е: &gtk::gdk::EventButton) {
    let (x, y) = е.position();
    if отладка() {
        eprintln!("glower-panel: стол: нажатие {} в {x:.0},{y:.0}", е.button());
    }
    if let Some(н) = виджет_под(x, y) {
        if е.event_type() == gtk::gdk::EventType::ButtonPress {
            if е.button() == 3 {
                меню_виджета(е, Some(н));
            } else if е.button() == 1 {
                // Виджет берут за любое место, кроме кнопок и полей внутри:
                // те забирают нажатие себе раньше, чем оно дойдёт сюда
                if let Some(с) = стол() {
                    let а = с.borrow().все_виджеты[н].allocation();
                    с.borrow_mut().жест = Жест::Виджет { номер: н, dx: x - а.x() as f64, dy: y - а.y() as f64, x0: x, y0: y, сдвинули: false };
                }
            }
        }
        return;
    }
    let Some(с) = стол() else { return };
    // GTK шлёт и одиночное нажатие, и «двойное» вдогонку — двойное
    // означает «открыть», и только для значка
    if е.event_type() == gtk::gdk::EventType::DoubleButtonPress {
        if е.button() == 1 {
            if let Some(имя) = под_курсором(x, y) {
                с.borrow_mut().жест = Жест::Нет;
                открой(&имя);
            }
        }
        return;
    }
    if е.event_type() != gtk::gdk::EventType::ButtonPress {
        return;
    }
    let значок = под_курсором(x, y);

    if е.button() == 3 {
        match значок {
            Some(имя) => {
                if !с.borrow().выбор.contains(&имя) {
                    выдели(vec![имя.clone()]);
                }
                меню_значков(е);
            }
            None => {
                выдели(Vec::new());
                меню_стола(е);
            }
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
            // Перерисовываем только то, где рамка была и где стала, а не весь
            // экран с обоями на каждый шаг мыши
            let (бx, бy) = (*x1, *y1);
            *x1 = x;
            *y1 = y;
            let (лево, верх, право, низ) = (x0.min(x), y0.min(y), x0.max(x), y0.max(y));
            let (зл, зв) = (лево.min(x0.min(бx)), верх.min(y0.min(бy)));
            let (зп, зн) = (право.max(x0.max(бx)), низ.max(y0.max(бy)));
            let область = ((зл - 2.0) as i32, (зв - 2.0) as i32, (зп - зл + 5.0) as i32, (зн - зв + 5.0) as i32);
            let выбранные: Vec<String> = с_
                .значки
                .iter()
                .filter(|з| {
                    let (ш, в) = (з.вид.allocated_width() as f64, з.вид.allocated_height() as f64);
                    !(з.x + ш < лево || з.x > право || з.y + в < верх || з.y > низ)
                })
                .map(|з| з.имя.clone())
                .collect();
            let тот_же = выбранные == с_.выбор;
            drop(с_);
            поле.queue_draw_area(область.0, область.1, область.2, область.3);
            if !тот_же {
                выдели(выбранные);
            }
        }
        Жест::Виджет { номер, dx, dy, x0, y0, сдвинули } => {
            if !*сдвинули && (x - *x0).hypot(y - *y0) < 5.0 {
                return;
            }
            *сдвинули = true;
            let (н, dx, dy) = (*номер, *dx, *dy);
            let Some(в) = с_.все_виджеты.get(н).cloned() else { return };
            let колонка = с_.виджеты.clone();
            let окно = с_.обои.clone();
            drop(с_);
            // Из колонки у края — на свободное место стола
            if в.parent().as_ref() == Some(колонка.upcast_ref::<gtk::Widget>()) {
                let а = в.allocation();
                колонка.remove(&в);
                поле.put(&в, а.x(), а.y());
                в.show_all();
            }
            let (ш, вс) = (окно.allocated_width() as f64, окно.allocated_height() as f64);
            let нx = (x - dx).clamp(0.0, (ш - 60.0).max(0.0));
            let нy = (y - dy).clamp(30.0, (вс - 60.0).max(30.0));
            поле.move_(&в, нx as i32, нy as i32);
        }
        Жест::Нет => {}
    }
}

fn отпускание() {
    let Some(с) = стол() else { return };
    let жест = std::mem::replace(&mut с.borrow_mut().жест, Жест::Нет);
    match жест {
        // Новое место запоминает ядро — в настройках, iconPos
        Жест::Тянем { где, сдвинули: true, .. } => {
            let с_ = с.borrow();
            let mut места = core::настройка("iconPos").as_object().cloned().unwrap_or_default();
            for (n, _, _) in &где {
                if let Some(з) = с_.значки.iter().find(|з| &з.имя == n) {
                    места.insert(n.clone(), json!({ "x": з.x.round(), "y": з.y.round() }));
                }
            }
            drop(с_);
            core::задай("iconPos", Value::Object(места));
        }
        Жест::Рамка { .. } => с.borrow().поле.queue_draw(),
        // Щёлкнули, не сдвинув, — открыть то, что за виджетом стоит
        Жест::Виджет { номер, сдвинули: false, .. } => {
            let (список, где) = записи_виджетов();
            if let Some(t) = где.get(номер).and_then(|&i| список[i]["t"].as_str()) {
                открой_виджет(t);
            }
        }
        Жест::Виджет { номер, сдвинули: true, .. } => {
            let место = {
                let с_ = с.borrow();
                с_.все_виджеты.get(номер).map(|в| (с_.поле.child_x(в) as f64, с_.поле.child_y(в) as f64))
            };
            if let Some(м) = место {
                место_виджета(номер, Some(м));
            }
        }
        _ => {}
    }
}

// ------------------------------------------------------------- выделение

fn выдели(имена: Vec<String>) {
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

fn выбранные() -> Vec<Value> {
    let Some(с) = стол() else { return Vec::new() };
    let с = с.borrow();
    с.значки.iter().filter(|з| с.выбор.contains(&з.имя)).map(|з| з.данные.clone()).collect()
}

// ------------------------------------------------------------- дела

fn путь_файла(з: &Value) -> Option<PathBuf> {
    matches!(з["вид"].as_str(), Some("файл") | Some("папка")).then(|| core::папка_стола().join(з["n"].as_str().unwrap_or("")))
}

fn открой(имя: &str) {
    let Some(з) = данные_значка(имя) else { return };
    match з["вид"].as_str().unwrap_or("") {
        "приложение" => core::запусти(з["ярлык"].as_str().unwrap_or(""), "приложение"),
        "машина" => core::запусти(з["ярлык"].as_str().unwrap_or(""), "машина"),
        "система" => {
            core::запусти_команду("glower-files", &[]);
        }
        _ => {
            if let Some(п) = путь_файла(&з) {
                core::открой_путь(&п);
            }
        }
    }
}

/// Просьбы с шины — для тех, кто ещё говорит со столом по-старому
pub fn дело_ядра(д: &Value) {
    match д["что"].as_str().unwrap_or("") {
        "открой" => открой(д["n"].as_str().unwrap_or("")),
        "позиции" => {
            let mut места = core::настройка("iconPos").as_object().cloned().unwrap_or_default();
            for (n, п) in д["позиции"].as_object().cloned().unwrap_or_default() {
                места.insert(n, п);
            }
            core::задай("iconPos", Value::Object(места));
        }
        _ => {}
    }
}

fn спроси_имя(заголовок: &str, было: &str) -> Option<String> {
    let д = gtk::Dialog::with_buttons(Some(заголовок), None::<&gtk::Window>, gtk::DialogFlags::MODAL, &[("Отмена", gtk::ResponseType::Cancel), ("Готово", gtk::ResponseType::Accept)]);
    д.style_context().add_class("glower-app");
    д.set_default_response(gtk::ResponseType::Accept);
    let поле = gtk::Entry::new();
    поле.set_text(было);
    {
        let д = д.clone();
        поле.connect_activate(move |_| д.response(gtk::ResponseType::Accept));
    }
    поле.set_width_chars(34);
    поле.set_margin(14);
    д.content_area().add(&поле);
    д.show_all();
    let конец = match было.rfind('.') {
        Some(т) if т > 0 => т as i32,
        _ => -1,
    };
    let поле2 = поле.clone();
    glib::idle_add_local_once(move || поле2.select_region(0, конец));
    let ответ = д.run();
    let имя = поле.text().trim().to_string();
    unsafe { д.destroy() };
    (ответ == gtk::ResponseType::Accept && !имя.is_empty() && !имя.contains('/') && имя != "." && имя != "..").then_some(имя)
}

fn свободное(имя: &str) -> PathBuf {
    let папка = core::папка_стола();
    let п = папка.join(имя);
    if !п.exists() {
        return п;
    }
    let (основа, хвост) = match имя.rfind('.') {
        Some(т) if т > 0 => (&имя[..т], &имя[т..]),
        _ => (имя, ""),
    };
    (2..).map(|н| папка.join(format!("{основа} ({н}){хвост}"))).find(|п| !п.exists()).unwrap()
}

fn создай_на_столе(папку: bool) {
    let _ = std::fs::create_dir_all(core::папка_стола());
    let черновик = свободное(if папку { "Новая папка" } else { "Новый документ.txt" });
    let имя = черновик.file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_default();
    let Some(имя) = спроси_имя(if папку { "Новая папка" } else { "Новый файл" }, &имя) else { return };
    let путь = core::папка_стола().join(&имя);
    if путь.exists() {
        return notify::покажи("Имя уже занято", &format!("На столе уже есть «{имя}»"), "⚠️");
    }
    let итог = if папку { std::fs::create_dir(&путь) } else { std::fs::write(&путь, b"") };
    if let Err(е) = итог {
        notify::покажи("Не удалось создать", &е.to_string(), "⚠️");
    }
}

fn переименуй() {
    let выбор = выбранные();
    let [з] = выбор.as_slice() else { return };
    let Some(путь) = путь_файла(з) else { return };
    let было = з["n"].as_str().unwrap_or("").to_string();
    let Some(имя) = спроси_имя("Переименовать", &было) else { return };
    if имя == было {
        return;
    }
    let куда = core::папка_стола().join(&имя);
    if куда.exists() {
        return notify::покажи("Имя уже занято", &format!("На столе уже есть «{имя}»"), "⚠️");
    }
    match std::fs::rename(&путь, &куда) {
        Ok(()) => {
            // Место значка переезжает вместе с именем
            let mut места = core::настройка("iconPos").as_object().cloned().unwrap_or_default();
            if let Some(м) = места.remove(&было) {
                места.insert(имя, м);
                core::задай("iconPos", Value::Object(места));
            }
        }
        Err(е) => notify::покажи("Не удалось переименовать", &е.to_string(), "⚠️"),
    }
}

/// Удалить выделенное: файлы — в корзину, программы — со стола
fn удали() {
    for з in выбранные() {
        match з["вид"].as_str().unwrap_or("") {
            "файл" | "папка" => {
                if let Some(п) = путь_файла(&з) {
                    if let Err(е) = gio::File::for_path(&п).trash(gio::Cancellable::NONE) {
                        notify::покажи("Не удалось удалить", &format!("«{}»: {е}", з["n"].as_str().unwrap_or("")), "⚠️");
                    }
                }
            }
            "приложение" => crate::core::убери_со_стола(з["ярлык"].as_str().unwrap_or(""), "приложение"),
            "машина" => crate::core::убери_со_стола(з["ярлык"].as_str().unwrap_or(""), "машина"),
            "система" => core::задай("deskThisPC", json!(false)),
            _ => {}
        }
    }
    выдели(Vec::new());
}

fn пункт(меню: &gtk::Menu, текст: &str, дело: impl Fn() + 'static) {
    let п = gtk::MenuItem::with_label(текст);
    п.connect_activate(move |_| дело());
    меню.append(&п);
}

fn черта(меню: &gtk::Menu) {
    меню.append(&gtk::SeparatorMenuItem::new());
}

fn меню_значков(е: &gtk::gdk::EventButton) {
    let выбор = выбранные();
    let Some(первый) = выбор.first().cloned() else { return };
    let один = выбор.len() == 1;
    let вид = первый["вид"].as_str().unwrap_or("").to_string();
    let меню = gtk::Menu::new();
    {
        let имя = первый["n"].as_str().unwrap_or("").to_string();
        пункт(&меню, "Открыть", move || открой(&имя));
    }
    if один && вид == "папка" {
        if let Some(п) = путь_файла(&первый) {
            пункт(&меню, "Открыть в Проводнике", move || {
                core::запусти_команду("glower-files", &[&п.to_string_lossy()]);
            });
        }
    }
    let файлы: Vec<PathBuf> = выбор.iter().filter_map(путь_файла).collect();
    if !файлы.is_empty() {
        черта(&меню);
        let пути = файлы.iter().map(|п| п.to_string_lossy().to_string()).collect::<Vec<_>>().join("\n");
        пункт(&меню, "Копировать путь", move || gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD).set_text(&пути));
        if один {
            пункт(&меню, "Переименовать", переименуй);
        }
        пункт(&меню, "Удалить", удали);
        if один {
            let п = файлы[0].clone();
            пункт(&меню, "Показать в Проводнике", move || {
                core::запусти_команду("glower-files", &[&п.to_string_lossy()]);
            });
        }
        черта(&меню);
        let пути: Vec<String> = файлы.iter().map(|п| п.to_string_lossy().to_string()).collect();
        пункт(&меню, "Свойства", move || {
            let доводы: Vec<&str> = пути.iter().map(|п| п.as_str()).collect();
            core::запусти_команду("glower-props", &доводы);
        });
    } else if вид == "приложение" || вид == "машина" {
        черта(&меню);
        пункт(&меню, "Убрать с рабочего стола", удали);
    } else if вид == "система" {
        черта(&меню);
        пункт(&меню, "Убрать с рабочего стола", || core::задай("deskThisPC", json!(false)));
    }
    меню.show_all();
    let событие: &gtk::gdk::Event = е;
    меню.popup_at_pointer(Some(событие));
}

fn меню_стола(е: &gtk::gdk::EventButton) {
    let меню = gtk::Menu::new();
    пункт(&меню, "Создать папку", || создай_на_столе(true));
    пункт(&меню, "Создать текстовый файл", || создай_на_столе(false));
    черта(&меню);
    пункт(&меню, "Упорядочить значки", || core::задай("iconPos", json!({})));
    let виджеты = gtk::Menu::new();
    for (t, имя, знак) in core::ВИДЖЕТЫ {
        let t = t.to_string();
        пункт(&виджеты, &format!("{знак} {имя}"), move || {
            let mut с: Vec<Value> = core::настройка("deskWidgets").as_array().cloned().unwrap_or_default();
            с.push(json!({ "t": t }));
            core::задай("deskWidgets", Value::Array(с));
            core::задай("showDeskWidgets", json!(true));
        });
    }
    let добавить = gtk::MenuItem::with_label("Добавить виджет");
    добавить.set_submenu(Some(&виджеты));
    меню.append(&добавить);
    // «Вид» — как в Windows: галочками, что показывать на столе
    let вид = gtk::Menu::new();
    for (ключ, текст) in [("showDesktopIcons", "Показывать значки рабочего стола"), ("showDeskWidgets", "Показывать виджеты")] {
        let п = gtk::CheckMenuItem::with_label(текст);
        п.set_active(core::настройка(ключ).as_bool() != Some(false));
        п.connect_toggled(move |п| core::задай(ключ, json!(п.is_active())));
        вид.append(&п);
    }
    let пункт_вид = gtk::MenuItem::with_label("Вид");
    пункт_вид.set_submenu(Some(&вид));
    меню.append(&пункт_вид);
    if core::настройка("deskThisPC").as_bool() == Some(false) {
        пункт(&меню, "Показать «Этот компьютер»", || core::задай("deskThisPC", json!(true)));
    }
    черта(&меню);
    пункт(&меню, "Сменить обои…", || {
        core::открой_параметры("person");
    });
    пункт(&меню, "Параметры экрана", || {
        core::открой_параметры("display");
    });
    пункт(&меню, "Открыть папку стола", || {
        core::запусти_команду("glower-files", &[&core::папка_стола().to_string_lossy()]);
    });
    меню.show_all();
    let событие: &gtk::gdk::Event = е;
    меню.popup_at_pointer(Some(событие));
}

fn клавиша(е: &gtk::gdk::EventKey) -> glib::Propagation {
    use gtk::gdk::keys::constants as к;
    let ключ = е.keyval();
    let ctrl = е.state().contains(gtk::gdk::ModifierType::CONTROL_MASK);
    if ключ == к::Delete || ключ == к::KP_Delete {
        удали();
    } else if ключ == к::F2 {
        переименуй();
    } else if ключ == к::Return || ключ == к::KP_Enter {
        for з in выбранные() {
            открой(з["n"].as_str().unwrap_or(""));
        }
    } else if ctrl && crate::notepad::латинская(е) == "a" {
        let все: Vec<String> = стол().map(|с| с.borrow().значки.iter().map(|з| з.имя.clone()).collect()).unwrap_or_default();
        выдели(все);
    } else if ключ == к::Escape {
        выдели(Vec::new());
    } else {
        return glib::Propagation::Proceed;
    }
    glib::Propagation::Stop
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
    // У файлов — значок темы по их типу, если тема его знает
    let тема = з["значок"].as_str().unwrap_or("");
    if (вид == "файл" || вид == "папка") && !тема.is_empty() && gtk::IconTheme::default().is_some_and(|т| т.has_icon(тема)) {
        let к = gtk::Image::from_icon_name(Some(if вид == "папка" { "folder" } else { тема }), gtk::IconSize::Dialog);
        к.set_pixel_size(48);
        return к.upcast();
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

/// Ядро рассказало, что лежит на столе и где.
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
    let прежние: Vec<gtk::Box> = с.borrow().значки.iter().map(|з| з.вид.clone()).collect();
    for р in прежние {
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
        // Имена файлов и чужих программ не переводим: это не надписи системы
        if matches!(з["вид"].as_str(), Some("файл" | "папка" | "машина")) {
            подпись.style_context().add_class("no-i18n");
        }
        подпись.set_line_wrap(true);
        подпись.set_line_wrap_mode(gtk::pango::WrapMode::WordChar);
        подпись.set_lines(2);
        подпись.set_ellipsize(gtk::pango::EllipsizeMode::End);
        подпись.set_max_width_chars(12);
        подпись.set_justify(gtk::Justification::Center);
        вид.add(&подпись);
        поле.put(&вид, x as i32, y as i32);
        значки.push(Значок { имя, вид, x, y, данные: з });
    }
    {
        let mut с_ = с.borrow_mut();
        с_.выбор.retain(|и| значки.iter().any(|з| &з.имя == и));
        с_.значки = значки;
        с_.слепок = слепок;
    }
    поле.show_all();
    обнови_виджеты();
}

/// Ядро рассказало, какие у стола обои.
pub fn обои(что: &Value) {
    if let Some(css) = что["css"].as_str() {
        поставь_обои(css);
    }
}

// ------------------------------------------------------------- виджеты

/// Перестроить виджеты: какие показаны, решают настройки
fn сегодня_ключ() -> String {
    let т = сейчас();
    crate::calendar::ключ(т.year(), т.month() as u32, т.day_of_month() as u32)
}

pub fn обнови_виджеты() {
    let Some(с) = стол() else { return };
    let показывать = core::настройка("showDeskWidgets").as_bool() != Some(false);
    let записи: Vec<Value> = if показывать {
        core::настройка("deskWidgets").as_array().cloned().unwrap_or_default().into_iter().filter(|в| в["t"].is_string()).collect()
    } else {
        Vec::new()
    };
    let список: Vec<String> = записи.iter().map(|в| format!("{}@{}:{}", в["t"].as_str().unwrap_or(""), в["x"], в["y"])).collect();
    // Дела и события дня тоже видны на столе: поменялись — перерисовать
    let слепок = format!(
        "{}|{}|{}|{}|{}",
        список.join(","),
        core::погода(),
        core::настройка("tz"),
        crate::apps::читай_данные(crate::todo::ФАЙЛ),
        crate::apps::читай_данные(crate::calendar::ФАЙЛ)[&сегодня_ключ()]
    );
    if с.borrow().слепок_виджетов == слепок {
        return;
    }
    с.borrow_mut().слепок_виджетов = слепок;
    let коробка = с.borrow().виджеты.clone();
    let поле = с.borrow().поле.clone();
    for р in коробка.children() {
        коробка.remove(&р);
    }
    let старые = std::mem::take(&mut с.borrow_mut().все_виджеты);
    for в in старые {
        if в.parent().as_ref() == Some(поле.upcast_ref::<gtk::Widget>()) {
            поле.remove(&в);
        }
    }
    let mut все = Vec::new();
    for запись in записи {
        let t = запись["t"].as_str().unwrap_or("").to_string();
        let к = gtk::Box::new(gtk::Orientation::Vertical, 4);
        к.style_context().add_class("widget");
        к.set_size_request(240, -1);
        к.set_widget_name(&format!("w-{t}"));
        наполни_виджет(&к, &t);
        // Перенесённый человеком — на своём месте, остальные — колонкой у края
        match (запись["x"].as_f64(), запись["y"].as_f64()) {
            (Some(x), Some(y)) => {
                поле.put(&к, x.max(0.0) as i32, y.max(30.0) as i32);
                к.show_all();
            }
            _ => коробка.add(&к),
        }
        все.push(к);
    }
    коробка.show_all();
    с.borrow_mut().все_виджеты = все;
}

fn строка_виджета(к: &gtk::Box, текст: &str, класс: &str) -> gtk::Label {
    let л = gtk::Label::new(Some(текст));
    л.set_xalign(0.0);
    л.set_line_wrap(true);
    л.style_context().add_class(класс);
    к.add(&л);
    л
}

fn сейчас() -> glib::DateTime {
    crate::menus::сейчас()
}

fn наполни_виджет(к: &gtk::Box, t: &str) {
    match t {
        "clock" => {
            строка_виджета(к, "Часы", "w-t");
            let (в, _) = crate::menus::время_и_дата();
            строка_виджета(к, &в, "w-big").set_widget_name("время");
            let т = сейчас();
            строка_виджета(к, &format!("{}, {} {}", crate::menus::ДНИ[(т.day_of_week() as usize).clamp(1, 7) - 1], т.day_of_month(), crate::menus::МЕСЯЦЫ[(т.month() as usize).clamp(1, 12) - 1]), "w-sub");
        }
        "weather" => {
            let п = core::погода();
            let город = п["город"].as_str().map(String::from).unwrap_or_else(|| core::настройка("city").as_str().unwrap_or("").to_string());
            строка_виджета(к, &format!("Погода · {город}"), "w-t");
            if п.is_null() {
                строка_виджета(к, "Нет данных — нужна сеть", "w-sub");
                return;
            }
            let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            let з = gtk::Label::new(п["знак"].as_str());
            з.style_context().add_class("w-emoji");
            ряд.add(&з);
            let т = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let г = gtk::Label::new(Some(&format!("{:+}°", п["t"].as_i64().unwrap_or(0))));
            г.style_context().add_class("w-big");
            г.set_xalign(0.0);
            т.add(&г);
            let с = gtk::Label::new(п["слово"].as_str());
            с.style_context().add_class("w-sub");
            с.set_xalign(0.0);
            т.add(&с);
            ряд.add(&т);
            к.add(&ряд);
            let дни = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            дни.set_homogeneous(true);
            for д in п["дни"].as_array().cloned().unwrap_or_default() {
                let столб = gtk::Box::new(gtk::Orientation::Vertical, 0);
                for (текст, класс) in [(д["день"].as_str().unwrap_or("").to_string(), "w-sub"), (д["знак"].as_str().unwrap_or("").to_string(), ""), (format!("{}°", д["max"]), "w-sub")] {
                    let л = gtk::Label::new(Some(&текст));
                    if !класс.is_empty() {
                        л.style_context().add_class(класс);
                    }
                    столб.add(&л);
                }
                дни.add(&столб);
            }
            к.add(&дни);
        }
        "calendar" => {
            let т = сейчас();
            строка_виджета(к, &format!("{} {}", crate::menus::МЕСЯЦЫ_ИМ[(т.month() as usize).clamp(1, 12) - 1], т.year()), "w-t");
            let сетка = gtk::Grid::new();
            сетка.set_column_homogeneous(true);
            сетка.set_row_spacing(2);
            for (i, д) in ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"].iter().enumerate() {
                let л = gtk::Label::new(Some(д));
                л.style_context().add_class("w-sub");
                сетка.attach(&л, i as i32, 0, 1, 1);
            }
            let первое = glib::DateTime::from_local(т.year(), т.month(), 1, 0, 0, 0.0).ok();
            let сдвиг = первое.as_ref().map(|п| п.day_of_week() - 1).unwrap_or(0);
            let дней = первое.and_then(|п| п.add_months(1).ok()).and_then(|с| с.add_days(-1).ok()).map(|п| п.day_of_month()).unwrap_or(30);
            for д in 1..=дней {
                let место = сдвиг + д - 1;
                let л = gtk::Label::new(Some(&д.to_string()));
                if д == т.day_of_month() {
                    л.style_context().add_class("w-today");
                }
                сетка.attach(&л, место % 7, 1 + место / 7, 1, 1);
            }
            к.add(&сетка);
            for е in crate::calendar::события(&crate::apps::читай_данные(crate::calendar::ФАЙЛ), &сегодня_ключ()).iter().take(3) {
                let т = format!("{}{}", е["time"].as_str().filter(|т| !т.is_empty()).map(|т| format!("{т}  ")).unwrap_or_default(), е["t"].as_str().unwrap_or(""));
                let л = строка_виджета(к, &т, "w-sub");
                л.set_ellipsize(gtk::pango::EllipsizeMode::End);
            }
        }
        "todo" => {
            строка_виджета(к, "Дела", "w-t");
            let все = crate::todo::дела();
            let открытые: Vec<(usize, serde_json::Value)> = все.iter().cloned().enumerate().filter(|(_, д)| д["d"].as_bool() != Some(true)).collect();
            if открытые.is_empty() {
                строка_виджета(к, "Всё сделано 🎉", "w-sub");
            }
            for (i, д) in открытые.into_iter().take(5) {
                let г = gtk::CheckButton::with_label(д["t"].as_str().unwrap_or(""));
                г.connect_toggled(move |г| {
                    if !г.is_active() {
                        return;
                    }
                    let mut все = crate::todo::дела();
                    if let Some(д) = все.get_mut(i) {
                        д["d"] = json!(true);
                    }
                    crate::apps::пиши_данные(crate::todo::ФАЙЛ, &serde_json::Value::Array(все));
                    glib::idle_add_local_once(обнови_виджеты);
                });
                к.add(&г);
            }
            let открыть = gtk::Button::with_label("Открыть Дела");
            открыть.style_context().add_class("w-link");
            открыть.set_halign(gtk::Align::Start);
            открыть.connect_clicked(|_| core::запусти("todo", "приложение"));
            к.add(&открыть);
        }
        "system" => {
            строка_виджета(к, "Система", "w-t");
            строка_виджета(к, "…", "w-sub").set_widget_name("система");
        }
        "note" => {
            строка_виджета(к, "Заметка", "w-t");
            let поле = gtk::TextView::new();
            поле.set_wrap_mode(gtk::WrapMode::WordChar);
            поле.style_context().add_class("w-note");
            поле.set_size_request(-1, 90);
            поле.buffer().map(|б| б.set_text(core::настройка("note").as_str().unwrap_or("")));
            // Пишем по паузе: каждую букву в файл класть незачем
            let ждём: Rc<std::cell::Cell<u32>> = Rc::new(std::cell::Cell::new(0));
            if let Some(б) = поле.buffer() {
                б.connect_changed(move |б| {
                    let номер = ждём.get() + 1;
                    ждём.set(номер);
                    let (ждём2, б2) = (ждём.clone(), б.clone());
                    glib::timeout_add_local_once(Duration::from_millis(800), move || {
                        if ждём2.get() == номер {
                            let (н, к) = б2.bounds();
                            core::задай_тихо("note", json!(б2.text(&н, &к, false).map(|т| т.to_string()).unwrap_or_default()));
                        }
                    });
                });
            }
            к.add(&поле);
        }
        _ => {}
    }
}

/// Раз в секунду — часы, раз в две — система
fn тик_виджетов() {
    let Some(с) = стол() else { return };
    let все = с.borrow().все_виджеты.clone();
    for к in все {
        for р in к.children() {
            match р.widget_name().as_str() {
                "время" => {
                    if let Some(л) = р.downcast_ref::<gtk::Label>() {
                        л.set_text(&crate::menus::время_и_дата().0);
                    }
                }
                "система" => {
                    if let Some(л) = р.downcast_ref::<gtk::Label>() {
                        л.set_text(&система_словами());
                    }
                }
                _ => {}
            }
        }
    }
}

thread_local! {
    static ТАКТЫ: std::cell::Cell<(u64, u64)> = const { std::cell::Cell::new((0, 0)) };
}

fn система_словами() -> String {
    let т = std::fs::read_to_string("/proc/stat").unwrap_or_default();
    let ч: Vec<u64> = т.lines().next().unwrap_or("").split_whitespace().skip(1).filter_map(|с| с.parse().ok()).collect();
    let всего: u64 = ч.iter().sum();
    let простой = ч.get(3).copied().unwrap_or(0) + ч.get(4).copied().unwrap_or(0);
    let (б_в, б_п) = ТАКТЫ.with(|т| т.replace((всего, простой)));
    let цп = if б_в > 0 && всего > б_в { (1.0 - (простой - б_п) as f64 / (всего - б_в) as f64) * 100.0 } else { 0.0 };
    let м = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let поле = |имя: &str| м.lines().find(|с| с.starts_with(имя)).and_then(|с| с.split_whitespace().nth(1)).and_then(|ч| ч.parse::<f64>().ok()).unwrap_or(0.0) / 1048576.0;
    let (всего_п, свободно) = (поле("MemTotal:"), поле("MemAvailable:"));
    format!("Процессор {цп:.0}%   ·   Память {:.1} из {:.1} ГБ", всего_п - свободно, всего_п).replace('.', ",")
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
