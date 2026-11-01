//! Заставка между загрузкой и рабочим столом — как в Windows.
//!
//! Тот же вид, что у заставки загрузки (Plymouth, linux/plymouth): те же
//! цвета фона, логотип и надпись на тех же местах, и те же пять точек,
//! бегущих по кругу по той же формуле, — поэтому от включения до стола
//! экран не меняется, только движутся точки.
//!
//!   glower-splash          — после входа: «Добро пожаловать», поверх всего;
//!                            гаснет, когда рабочий стол нарисован
//!                            ($XDG_RUNTIME_DIR/glower-desktop-ready);
//!   glower-splash --вход   — под экраном входа, пока тот загружается.
//! Сама уходит через 40 секунд, что бы ни случилось.

use gtk::glib;
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::f64::consts::PI;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub fn файл_готовности() -> std::path::PathBuf {
    glib::user_runtime_dir().join("glower-desktop-ready")
}

/// Рабочий стол нарисован — заставке пора
pub fn стол_готов() {
    let _ = std::fs::write(файл_готовности(), b"1");
}

/// Положение точки на круге — та же формула, что в заставке загрузки
/// (linux/plymouth/glower.script): точки бегут друг за другом, сверху
/// разгоняются, снизу притормаживают; в конце круга — короткая пауза
pub fn угол_точки(t: f64, i: usize) -> Option<f64> {
    let p = ((t - i as f64 * 0.11) / 2.4).rem_euclid(1.0);
    if p > 0.88 {
        return None;
    }
    let q = p / 0.88;
    let путь = q - (2.0 * PI * q).sin() / (2.0 * PI) * 0.75;
    Some(PI / 2.0 + путь * 2.0 * PI * 1.25)
}

/// Размеры — как в заставке загрузки: от высоты экрана
pub struct Разметка {
    pub cx: f64,
    pub cy: f64,
    pub знак: f64,
    pub слово_ш: f64,
    pub слово_в: f64,
    pub слово_y: f64,
    pub круг_y: f64,
    pub круг_r: f64,
    pub точка: f64,
}

pub fn разметка(w: f64, h: f64, слово_пропорция: f64) -> Разметка {
    let cx = w / 2.0;
    let cy = h / 2.0 - h / 22.0;
    let знак = (h / 5.2).floor();
    let слово_ш = (знак * 1.35).floor();
    let слово_в = (слово_ш * слово_пропорция).floor();
    let слово_y = cy + знак / 2.0 + знак * 0.22;
    let круг_r = (знак * 0.15).floor().max(14.0);
    let круг_y = слово_y + слово_в + знак * 0.42 + круг_r;
    let точка = (круг_r * 0.12).max(2.5);
    Разметка { cx, cy, знак, слово_ш, слово_в, слово_y, круг_y, круг_r, точка }
}

fn плавно(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Картинка, уменьшенная один раз и хорошо (а не растянутая на каждом кадре)
fn готовая(имя: &str, ш: f64, в: f64) -> Option<gtk::cairo::ImageSurface> {
    let путь = crate::apps::файл_системы(имя)?;
    let исходная = gtk::gdk_pixbuf::Pixbuf::from_file(путь).ok()?;
    let м = исходная.scale_simple(ш.max(1.0) as i32, в.max(1.0) as i32, gtk::gdk_pixbuf::InterpType::Hyper)?;
    let поверхность = gtk::cairo::ImageSurface::create(gtk::cairo::Format::ARgb32, м.width(), м.height()).ok()?;
    {
        let к = gtk::cairo::Context::new(&поверхность).ok()?;
        к.set_source_pixbuf(&м, 0.0, 0.0);
        к.paint().ok()?;
    }
    Some(поверхность)
}

struct Картинки {
    размер: (i32, i32),
    знак: Option<gtk::cairo::ImageSurface>,
    слово: Option<gtk::cairo::ImageSurface>,
}

pub fn запусти(доводы: Vec<String>) -> ! {
    if gtk::init().is_err() {
        std::process::exit(1);
    }
    crate::i18n::подключи();
    let вход = доводы.iter().any(|д| д == "--вход");
    if !вход {
        let _ = std::fs::remove_file(файл_готовности());
    }
    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.init_layer_shell();
    окно.set_layer(if вход { Layer::Background } else { Layer::Overlay });
    окно.set_namespace("glower-splash");
    for край in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        окно.set_anchor(край, true);
    }
    окно.set_exclusive_zone(-1);
    окно.set_keyboard_mode(KeyboardMode::None);
    окно.set_app_paintable(true);
    if let Some(в) = gtk::prelude::WidgetExt::screen(&окно).and_then(|э| э.rgba_visual()) {
        окно.set_visual(Some(&в));
    }

    let пропорция = crate::apps::файл_системы("logo-word.png")
        .and_then(|п| gtk::gdk_pixbuf::Pixbuf::file_info(п).map(|(_, ш, в)| в as f64 / ш.max(1) as f64))
        .unwrap_or(175.0 / 994.0);
    let подпись = if вход { String::new() } else { crate::i18n::tr("Добро пожаловать") };
    let начало = Instant::now();
    let гаснет: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));
    let картинки: Rc<RefCell<Option<Картинки>>> = Rc::new(RefCell::new(None));

    {
        let (гаснет, картинки) = (гаснет.clone(), картинки.clone());
        окно.connect_draw(move |о, к| {
            let (ш, в) = (о.allocated_width(), о.allocated_height());
            let (w, h) = (ш as f64, в as f64);
            let р = разметка(w, h, пропорция);
            // Картинки — под этот размер экрана, один раз
            if картинки.borrow().as_ref().map(|к| к.размер) != Some((ш, в)) {
                *картинки.borrow_mut() = Some(Картинки {
                    размер: (ш, в),
                    знак: готовая("logo-dark.png", р.знак, р.знак).or_else(|| готовая("logo.png", р.знак, р.знак)),
                    слово: готовая("logo-word.png", р.слово_ш, р.слово_в),
                });
            }
            let t = начало.elapsed().as_secs_f64();
            let видно = match гаснет.get() {
                Some(с) => 1.0 - плавно(с.elapsed().as_secs_f64() / 0.45),
                None => 1.0,
            };

            к.set_operator(gtk::cairo::Operator::Source);
            к.set_source_rgba(0.0, 0.0, 0.0, 0.0);
            let _ = к.paint();
            к.set_operator(gtk::cairo::Operator::Over);
            let _ = к.push_group();

            // Фон — как у заставки загрузки
            let г = gtk::cairo::LinearGradient::new(0.0, 0.0, 0.0, h);
            г.add_color_stop_rgb(0.0, 0.035, 0.04, 0.085);
            г.add_color_stop_rgb(1.0, 0.01, 0.01, 0.03);
            let _ = к.set_source(&г);
            let _ = к.paint();

            if let Some(кар) = картинки.borrow().as_ref() {
                if let Some(з) = &кар.знак {
                    let _ = к.set_source_surface(з, (р.cx - р.знак / 2.0).round(), (р.cy - р.знак / 2.0).round());
                    let _ = к.paint();
                }
                if let Some(с) = &кар.слово {
                    let _ = к.set_source_surface(с, (р.cx - р.слово_ш / 2.0).round(), р.слово_y.round());
                    let _ = к.paint();
                }
            }

            // Пять точек; первые полсекунды проявляются
            let проявление = плавно(t / 0.5);
            к.set_antialias(gtk::cairo::Antialias::Best);
            for i in 0..5 {
                if let Some(а) = угол_точки(t, i) {
                    к.set_source_rgba(1.0, 1.0, 1.0, 0.95 * проявление);
                    к.arc(р.cx + р.круг_r * а.cos(), р.круг_y + р.круг_r * а.sin(), р.точка, 0.0, 2.0 * PI);
                    let _ = к.fill();
                }
            }

            // Подпись — системным шрифтом, проявляется вслед за точками
            if !подпись.is_empty() {
                let слой = pangocairo::functions::create_layout(к);
                let mut шрифт = gtk::pango::FontDescription::new();
                шрифт.set_family("Sans");
                шрифт.set_weight(gtk::pango::Weight::Normal);
                шрифт.set_absolute_size((р.знак / 9.0).clamp(15.0, 28.0) * gtk::pango::SCALE as f64);
                слой.set_font_description(Some(&шрифт));
                слой.set_text(&подпись);
                let (тш, _) = слой.pixel_size();
                к.set_source_rgba(1.0, 1.0, 1.0, 0.86 * плавно((t - 0.3) / 0.5));
                к.move_to((р.cx - тш as f64 / 2.0).round(), (р.круг_y + р.круг_r + р.знак * 0.2).round());
                pangocairo::functions::show_layout(к, &слой);
            }

            let _ = к.pop_group_to_source();
            let _ = к.paint_with_alpha(видно);
            glib::Propagation::Stop
        });
    }

    // Кадр — по кадрам монитора, без рывков таймера. Стол готов — плавно
    // гаснем; сорок секунд прошло — тоже
    {
        let гаснет = гаснет.clone();
        окно.add_tick_callback(move |о, _| {
            if гаснет.get().is_none() && ((!вход && файл_готовности().exists()) || начало.elapsed() > Duration::from_secs(40)) {
                гаснет.set(Some(Instant::now()));
            }
            if гаснет.get().is_some_and(|с| с.elapsed() > Duration::from_millis(480)) {
                std::process::exit(0);
            }
            о.queue_draw();
            glib::ControlFlow::Continue
        });
    }
    окно.show_all();
    gtk::main();
    std::process::exit(0);
}

#[cfg(test)]
mod проверки {
    #[test]
    fn точки_бегут_и_исчезают_между_кругами() {
        assert!((0..5).all(|i| super::угол_точки(0.6, i).is_some()));
        assert!((0..5).all(|i| super::угол_точки(2.35 + i as f64 * 0.11, i).is_none()));
        let (а, б) = (super::угол_точки(0.5, 0).unwrap(), super::угол_точки(0.5, 1).unwrap());
        assert!(а > б);
    }

    #[test]
    fn разметка_как_у_заставки_загрузки() {
        // 1920×1080: знак 207, надпись 279 в ширину — те же числа, что
        // считает linux/plymouth/glower.script
        let р = super::разметка(1920.0, 1080.0, 175.0 / 994.0);
        assert_eq!(р.знак, 207.0);
        assert_eq!(р.слово_ш, 279.0);
        assert_eq!(р.круг_r, 31.0);
    }
}
