//! Заставка между загрузкой и рабочим столом — как в Windows.
//!
//! Тот же вид, что у заставки загрузки (Plymouth, linux/plymouth): те же
//! цвета фона, логотип и надпись на тех же местах и та же комета на
//! орбите. Кадры Plymouth нарисованы этой же функцией (`glower-splash
//! --кадры`), поэтому от включения до стола экран не меняется — только
//! бежит комета.
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

/// Один оборот кометы, секунд
pub const ПЕРИОД: f64 = 2.0;
/// Кадров на оборот у Plymouth: он перерисовывает экран 50 раз в секунду,
/// и каждый его кадр — свой рисунок, без сдвига картинки по целым пикселям
pub const КАДРОВ: usize = 100;
/// Холст кадра Plymouth и радиус орбиты на нём
pub const ХОЛСТ: f64 = 208.0;
pub const ХОЛСТ_R: f64 = 72.0;
/// Толщина хвоста — доля радиуса орбиты
pub const ТОЛЩИНА: f64 = 0.11;

/// Где голова кометы и как быстро она идёт. Скорость плавно меняется за
/// оборот (от 0,62 до 1,38 средней): сверху комета разгоняется, снизу
/// притормаживает, и хвост вслед за скоростью то вытягивается, то
/// подбирается. Ровно за период — ровно оборот: кадры зациклены без шва
pub fn голова(t: f64) -> (f64, f64) {
    let p = (t / ПЕРИОД).rem_euclid(1.0);
    let к = 0.06;
    let угол = -PI / 2.0 + 2.0 * PI * (p + к * (2.0 * PI * p).sin());
    let скорость = 1.0 + к * 2.0 * PI * (2.0 * PI * p).cos();
    (угол, скорость)
}

fn смешай(а: (f64, f64, f64), б: (f64, f64, f64), u: f64) -> (f64, f64, f64) {
    (а.0 + (б.0 - а.0) * u, а.1 + (б.1 - а.1) * u, а.2 + (б.2 - а.2) * u)
}

/// Комета на орбите: едва видное кольцо, хвост из голубого в тёплый белый,
/// светлая голова с тёплым ореолом. r — радиус орбиты, w — толщина хвоста
pub fn комета(к: &gtk::cairo::Context, cx: f64, cy: f64, r: f64, w: f64, t: f64, видно: f64) {
    if видно <= 0.0 {
        return;
    }
    к.save().ok();
    к.set_antialias(gtk::cairo::Antialias::Best);
    к.set_line_cap(gtk::cairo::LineCap::Butt);
    // Орбита
    к.set_source_rgba(1.0, 1.0, 1.0, 0.09 * видно);
    к.set_line_width(w * 0.45);
    к.new_sub_path();
    к.arc(cx, cy, r, 0.0, 2.0 * PI);
    let _ = к.stroke();

    let (а, v) = голова(t);
    let длина = 0.5 + 1.7 * ((v - 0.62) / 0.76).clamp(0.0, 1.0);
    let хвост = (0.36, 0.76, 1.0);
    let тёплый = (1.0, 0.95, 0.86);
    let n = 64;
    for i in 0..n {
        let s0 = i as f64 / n as f64;
        let s1 = (i + 1) as f64 / n as f64;
        let u = s1;
        let (cr, cg, cb) = смешай(хвост, тёплый, u.powf(1.3));
        к.set_source_rgba(cr, cg, cb, u.powf(1.4) * видно);
        к.set_line_width(w * (0.25 + 0.75 * u));
        к.new_sub_path();
        к.arc(cx, cy, r, а - длина * (1.0 - s0), а - длина * (1.0 - s1) + 0.004);
        let _ = к.stroke();
    }

    // Ореол и голова
    let (hx, hy) = (cx + r * а.cos(), cy + r * а.sin());
    let ореол = gtk::cairo::RadialGradient::new(hx, hy, 0.0, hx, hy, w * 3.4);
    ореол.add_color_stop_rgba(0.0, 1.0, 0.82, 0.5, 0.5 * видно);
    ореол.add_color_stop_rgba(0.45, 1.0, 0.74, 0.4, 0.16 * видно);
    ореол.add_color_stop_rgba(1.0, 1.0, 0.7, 0.35, 0.0);
    let _ = к.set_source(&ореол);
    к.new_sub_path();
    к.arc(hx, hy, w * 3.4, 0.0, 2.0 * PI);
    let _ = к.fill();
    к.set_source_rgba(1.0, 0.98, 0.94, видно);
    к.new_sub_path();
    к.arc(hx, hy, w * 0.62, 0.0, 2.0 * PI);
    let _ = к.fill();
    к.restore().ok();
}

/// Кадры для Plymouth: `glower-splash --кадры ПАПКА` → spin-0.png … spin-99.png
fn нарисуй_кадры(папка: &str) -> Result<(), String> {
    std::fs::create_dir_all(папка).map_err(|е| е.to_string())?;
    let размер = ХОЛСТ as i32;
    for i in 0..КАДРОВ {
        let mut п = gtk::cairo::ImageSurface::create(gtk::cairo::Format::ARgb32, размер, размер).map_err(|е| е.to_string())?;
        {
            let к = gtk::cairo::Context::new(&п).map_err(|е| е.to_string())?;
            комета(&к, ХОЛСТ / 2.0, ХОЛСТ / 2.0, ХОЛСТ_R, ХОЛСТ_R * ТОЛЩИНА, i as f64 * ПЕРИОД / КАДРОВ as f64, 1.0);
        }
        п.flush();
        // Кадр cairo (BGRA с умноженной прозрачностью) → обычный RGBA для PNG;
        // без GDK — кадры рисуются при сборке, экрана там нет
        let шаг = п.stride() as usize;
        let данные = п.data().map_err(|е| е.to_string())?;
        let mut rgba = Vec::with_capacity((размер * размер * 4) as usize);
        for y in 0..размер as usize {
            for x in 0..размер as usize {
                let о = y * шаг + x * 4;
                let (b, g, r, a) = (данные[о] as u32, данные[о + 1] as u32, данные[о + 2] as u32, данные[о + 3] as u32);
                let раз = |c: u32| if a == 0 { 0 } else { ((c * 255 + a / 2) / a).min(255) as u8 };
                rgba.extend_from_slice(&[раз(r), раз(g), раз(b), a as u8]);
            }
        }
        let пб = gtk::gdk_pixbuf::Pixbuf::from_bytes(&glib::Bytes::from_owned(rgba), gtk::gdk_pixbuf::Colorspace::Rgb, true, 8, размер, размер, размер * 4);
        пб.savev(format!("{папка}/spin-{i}.png"), "png", &[]).map_err(|е| е.to_string())?;
    }
    Ok(())
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
    pub хвост: f64,
}

pub fn разметка(w: f64, h: f64, слово_пропорция: f64) -> Разметка {
    let cx = w / 2.0;
    let cy = h / 2.0 - h / 22.0;
    let знак = (h / 5.2).floor();
    let слово_ш = (знак * 1.35).floor();
    let слово_в = (слово_ш * слово_пропорция).floor();
    let слово_y = cy + знак / 2.0 + знак * 0.22;
    let круг_r = (знак * 0.17).floor().max(16.0);
    let круг_y = слово_y + слово_в + знак * 0.42 + круг_r;
    // Plymouth показывает кадр, уменьшенный до этого же радиуса
    let хвост = круг_r * ТОЛЩИНА;
    Разметка { cx, cy, знак, слово_ш, слово_в, слово_y, круг_y, круг_r, хвост }
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
    if let Some(i) = доводы.iter().position(|д| д == "--кадры") {
        let папка = доводы.get(i + 1).cloned().unwrap_or_else(|| ".".into());
        match нарисуй_кадры(&папка) {
            Ok(()) => std::process::exit(0),
            Err(е) => {
                eprintln!("кадры заставки: {е}");
                std::process::exit(1);
            }
        }
    }
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

            // Комета; первые полсекунды проявляется
            комета(к, р.cx, р.круг_y, р.круг_r, р.хвост, t, плавно(t / 0.5));

            // Подпись — системным шрифтом, проявляется вслед за кометой
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
    fn комета_делает_ровно_оборот_и_меняет_скорость() {
        use std::f64::consts::PI;
        let (а0, _) = super::голова(0.0);
        let (а1, _) = super::голова(super::ПЕРИОД - 1e-9);
        assert!((а1 - а0 - 2.0 * PI).abs() < 1e-6);
        let скорости: Vec<f64> = (0..20).map(|i| super::голова(i as f64 * 0.1).1).collect();
        let (мин, макс) = скорости.iter().fold((9.0f64, 0.0f64), |(а, б), &в| (а.min(в), б.max(в)));
        assert!(мин > 0.5 && макс < 1.5 && макс - мин > 0.5);
    }

    #[test]
    fn разметка_как_у_заставки_загрузки() {
        // 1920×1080: знак 207, надпись 279 в ширину — те же числа, что
        // считает linux/plymouth/glower.script
        let р = super::разметка(1920.0, 1080.0, 175.0 / 994.0);
        assert_eq!(р.знак, 207.0);
        assert_eq!(р.слово_ш, 279.0);
        assert_eq!(р.круг_r, 35.0);
    }
}
