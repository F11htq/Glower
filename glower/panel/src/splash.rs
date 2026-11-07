//! Заставка между загрузкой и рабочим столом — как в Windows.
//!
//! Заставка загрузки (Plymouth, linux/plymouth) собирает логотип из строк
//! кода белым по чёрному; строки для неё рисует `--кадры` (модуль
//! `строки`). После входа, на том же чёрном фоне, — блик: тёмная основа
//! логотипа проявляется из черноты, по ней наискосок проходит полоса
//! света, и за ней знак становится цветным; потом — «Добро пожаловать» и
//! три точки, пока стол грузится.
//!
//!   glower-splash          — после входа: «Добро пожаловать», поверх всего;
//!                            гаснет, когда рабочий стол нарисован
//!                            ($XDG_RUNTIME_DIR/glower-desktop-ready);
//!   glower-splash --вход   — под экраном входа, пока тот загружается;
//!   glower-splash --кадры ПАПКА ЛОГОТИП КОД… — строки кода для заставки
//!                            загрузки (Plymouth рисует логотип из них).
//! Сама уходит через 40 секунд, что бы ни случилось.

use gtk::glib;
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

pub fn файл_готовности() -> std::path::PathBuf {
    glib::user_runtime_dir().join("glower-desktop-ready")
}

/// Рабочий стол нарисован — заставке пора
pub fn стол_готов() {
    let _ = std::fs::write(файл_готовности(), b"1");
}

/// Блик после входа: тёмная основа проявляется, полоса света наискосок
/// открывает цвет, потом — «Добро пожаловать» и три точки. Секунды
pub const ОСНОВА: (f64, f64) = (0.1, 0.8);
pub const ПОЛОСА: (f64, f64) = (0.9, 1.1);
pub const ПОДПИСЬ: (f64, f64) = (2.0, 0.5);

/// Случайные, но всегда одинаковые числа (генератор Лемера). Такой же
/// считает linux/plymouth/glower.script для строк кода
pub struct Случай(u64);
impl Случай {
    pub fn new() -> Self {
        Случай(20251107)
    }
    pub fn next(&mut self) -> f64 {
        self.0 = self.0 * 16807 % 2147483647;
        self.0 as f64 / 2147483647.0
    }
}

/// Картинка cairo (BGRA с умноженной прозрачностью) → PNG; без GDK —
/// картинки рисуются при сборке, экрана там нет
fn сохрани(п: &mut gtk::cairo::ImageSurface, путь: &str) -> Result<(), String> {
    п.flush();
    let (ш, в) = (п.width(), п.height());
    let шаг = п.stride() as usize;
    let данные = п.data().map_err(|е| е.to_string())?;
    let mut rgba = Vec::with_capacity((ш * в * 4) as usize);
    for y in 0..в as usize {
        for x in 0..ш as usize {
            let о = y * шаг + x * 4;
            let (b, g, r, a) = (данные[о] as u32, данные[о + 1] as u32, данные[о + 2] as u32, данные[о + 3] as u32);
            let раз = |c: u32| if a == 0 { 0 } else { ((c * 255 + a / 2) / a).min(255) as u8 };
            rgba.extend_from_slice(&[раз(r), раз(g), раз(b), a as u8]);
        }
    }
    let пб = gtk::gdk_pixbuf::Pixbuf::from_bytes(&glib::Bytes::from_owned(rgba), gtk::gdk_pixbuf::Colorspace::Rgb, true, 8, ш, в, ш * 4);
    пб.savev(путь, "png", &[]).map_err(|е| е.to_string())
}

/// Заставка загрузки (Plymouth): логотип из строк кода, белым по чёрному.
/// Plymouth не умеет рисовать текст по знакам — строки рисуются здесь
/// заранее, под несколько высот экрана, а сценарий в glower.script их
/// только двигает. Разметка — как в glower.script
pub mod строки {
    /// Высоты экрана, под которые нарисованы строки; Plymouth берёт ближайшую
    pub const ВЫСОТЫ: [i32; 4] = [768, 1080, 1440, 2160];
    /// Рядов на высоту знака, ещё по стольку сверху и снизу — просто код
    pub const РЯДОВ: usize = 40;
    pub const ЛИШНИХ: usize = 6;
    /// Знак из кода крупнее обычного — иначе мелкие детали теряются
    pub const КРУПНЕЕ: f64 = 1.5;
    /// Ширина строки — в размерах знака: хватает и широкому экрану
    pub const ШИРИНА: f64 = 16.0;
    pub const ИМЯ: &str = "GlowerOS";

    pub fn знак(h: f64) -> f64 {
        (h / 5.2 * КРУПНЕЕ).floor()
    }
}

/// Картинки для Plymouth: `glower-splash --кадры ПАПКА ЛОГОТИП КОД…`.
/// Для каждой высоты экрана H: c{H}-{ряд}.png — тусклый код ряда,
/// l{H}-{ряд}.png — знаки логотипа в ряду (яркость — по светлоте
/// логотипа под знаком), n{H}-{0..8}.png — набранное «GlowerOS» по буквам,
/// cell{H}.png — ширина знакоместа. И dot.png — белая точка для курсора
fn нарисуй_кадры(папка: &str, логотип: &str, код: &[String]) -> Result<(), String> {
    use gtk::cairo::{Context, FontSlant, FontWeight, Format, ImageSurface};
    use строки::*;
    std::fs::create_dir_all(папка).map_err(|е| е.to_string())?;
    let е = |е: gtk::cairo::Error| е.to_string();

    // Строки кода подряд, через два пробела
    let mut текст_кода: Vec<String> = vec![];
    for путь in код {
        let т = std::fs::read_to_string(путь).map_err(|е| format!("{путь}: {е}"))?;
        for с in т.lines() {
            let с = с.trim();
            if с.chars().count() >= 12 && !["//", "/*", "*", "#"].iter().any(|н| с.starts_with(н)) {
                текст_кода.push(с.to_string());
            }
        }
    }
    if текст_кода.is_empty() {
        return Err("нет строк кода".into());
    }

    let исходный = gtk::gdk_pixbuf::Pixbuf::from_file(логотип).map_err(|е| е.to_string())?;
    let белая = |ш: i32, в: i32, путь: String| -> Result<(), String> {
        let mut п = ImageSurface::create(Format::ARgb32, ш, в).map_err(е)?;
        {
            let к = Context::new(&п).map_err(е)?;
            к.set_source_rgba(1.0, 1.0, 1.0, 1.0);
            let _ = к.paint();
        }
        сохрани(&mut п, &путь)
    };
    белая(4, 4, format!("{папка}/dot.png"))?;

    for h in ВЫСОТЫ {
        let знак = строки::знак(h as f64);
        let ряд = знак / РЯДОВ as f64;
        let шрифт = ряд * 0.98;
        let выбери = |к: &Context, жирный: bool, размер: f64| {
            к.select_font_face("DejaVu Sans Mono", FontSlant::Normal, if жирный { FontWeight::Bold } else { FontWeight::Normal });
            к.set_font_size(размер);
        };
        // Знакоместо — целое число точек: так строки встают ровно
        let пробная = ImageSurface::create(Format::ARgb32, 8, 8).map_err(е)?;
        let кп = Context::new(&пробная).map_err(е)?;
        выбери(&кп, true, шрифт);
        let клетка = кп.text_extents("M").map_err(е)?.x_advance().round().max(1.0) as usize;
        let fe = кп.font_extents().map_err(е)?;
        let столбцов = (знак / клетка as f64).ceil() as usize;
        let mut всего = (ШИРИНА * знак / клетка as f64).ceil() as usize;
        if (всего - столбцов) % 2 == 1 {
            всего += 1;
        }
        let лево = (всего - столбцов) / 2;
        let в = ряд.ceil() as i32;
        let базовая = в as f64 / 2.0 + (fe.ascent() - fe.descent()) / 2.0;
        белая(клетка as i32, 1, format!("{папка}/cell{h}.png"))?;

        // Светлота и прозрачность логотипа по клеткам: уменьшаем хорошо, потом
        // усредняем по четыре точки
        let м = исходный
            .scale_simple((столбцов * 4) as i32, (РЯДОВ * 4) as i32, gtk::gdk_pixbuf::InterpType::Hyper)
            .ok_or("логотип не уменьшился")?;
        let (пикс, шаг, n) = (м.read_pixel_bytes(), м.rowstride() as usize, м.n_channels() as usize);
        let клетка_лого = |r: usize, c: usize| -> (f64, f64) {
            let (mut а, mut свет) = (0.0, 0.0);
            for y in r * 4..r * 4 + 4 {
                for x in c * 4..c * 4 + 4 {
                    let о = y * шаг + x * n;
                    let ал = if n == 4 { пикс[о + 3] as f64 / 255.0 } else { 1.0 };
                    а += ал;
                    свет += ал * (0.3 * пикс[о] as f64 + 0.59 * пикс[о + 1] as f64 + 0.11 * пикс[о + 2] as f64) / 255.0;
                }
            }
            (а / 16.0, if а > 0.0 { свет / а } else { 0.0 })
        };

        let mut с = Случай::new();
        let mut номер = (с.next() * текст_кода.len() as f64) as usize;
        for i in 0..РЯДОВ + 2 * ЛИШНИХ {
            let mut знаки: Vec<char> = vec![];
            while знаки.len() < всего {
                знаки.extend(текст_кода[номер % текст_кода.len()].chars());
                знаки.extend("  ".chars());
                номер += 1;
            }
            знаки.truncate(всего);
            let ряд_лого = i.checked_sub(ЛИШНИХ).filter(|r| *r < РЯДОВ);
            let mut тусклый = ImageSurface::create(Format::ARgb32, (всего * клетка) as i32, в).map_err(е)?;
            let mut лого = ImageSurface::create(Format::ARgb32, (столбцов * клетка) as i32, в).map_err(е)?;
            {
                let т = Context::new(&тусклый).map_err(е)?;
                let л = Context::new(&лого).map_err(е)?;
                выбери(&т, false, шрифт);
                выбери(&л, true, шрифт);
                т.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                for (j, ch) in знаки.iter().enumerate() {
                    if *ch == ' ' {
                        continue;
                    }
                    let c = j as isize - лево as isize;
                    let (а, свет) = match ряд_лого {
                        Some(r) if c >= 0 && (c as usize) < столбцов => клетка_лого(r, c as usize),
                        _ => (0.0, 0.0),
                    };
                    let s = ch.to_string();
                    if а > 0.3 {
                        л.set_source_rgba(1.0, 1.0, 1.0, (а * (0.3 + 0.95 * свет.powf(1.3))).min(1.0));
                        л.move_to((c as usize * клетка) as f64, базовая);
                        let _ = л.show_text(&s);
                    } else {
                        т.move_to((j * клетка) as f64, базовая);
                        let _ = т.show_text(&s);
                    }
                }
            }
            сохрани(&mut тусклый, &format!("{папка}/c{h}-{i}.png"))?;
            if ряд_лого.is_some() {
                сохрани(&mut лого, &format!("{папка}/l{h}-{i}.png"))?;
            }
        }

        // «GlowerOS» по буквам: картинки одной ширины, буквы — слева
        let кегль = (знак * 0.14).round();
        выбери(&кп, true, кегль);
        let кл_имени = кп.text_extents("M").map_err(е)?.x_advance().round().max(1.0) as usize;
        let fe = кп.font_extents().map_err(е)?;
        let вн = (кегль * 1.25).ceil() as i32;
        let всего_имени = ИМЯ.chars().count() + 1;
        for набрано in 0..=ИМЯ.chars().count() {
            let mut п = ImageSurface::create(Format::ARgb32, (всего_имени * кл_имени) as i32, вн).map_err(е)?;
            {
                let к = Context::new(&п).map_err(е)?;
                выбери(&к, true, кегль);
                к.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                for (j, ch) in ИМЯ.chars().take(набрано).enumerate() {
                    к.move_to((j * кл_имени) as f64, fe.ascent());
                    let _ = к.show_text(&ch.to_string());
                }
            }
            сохрани(&mut п, &format!("{папка}/n{h}-{набрано}.png"))?;
        }
    }
    Ok(())
}

fn выход(x: f64) -> f64 {
    1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3)
}

/// Где полоса света в момент t — край цветной части в долях знака от его
/// левого края, на верхней кромке (край идёт наискосок, на знак влево к низу)
pub fn край_цвета(t: f64) -> f64 {
    -0.4 + 2.4 * выход((t - ПОЛОСА.0) / ПОЛОСА.1)
}

/// Логотип после входа в момент t. цветной и тёмный — знак, уже
/// уменьшенный под экран; тёмный — серый и притемнённый
pub fn блик(к: &gtk::cairo::Context, р: &Разметка, t: f64, цветной: &gtk::cairo::ImageSurface, тёмный: &gtk::cairo::ImageSurface) {
    let з = р.знак;
    let (x0, y0) = ((р.cx - з / 2.0).round(), (р.cy - з / 2.0).round());
    let основа = плавно((t - ОСНОВА.0) / ОСНОВА.1);
    if основа <= 0.0 {
        return;
    }
    к.save().ok();
    let _ = к.set_source_surface(тёмный, x0, y0);
    let _ = к.paint_with_alpha(основа);

    // Цвет — за полосой: всё, что левее наклонного края
    let край = x0 + край_цвета(t) * з;
    к.save().ok();
    к.move_to(x0 - з, y0);
    к.line_to(край, y0);
    к.line_to(край - з, y0 + з);
    к.line_to(x0 - з, y0 + з);
    к.close_path();
    к.clip();
    let _ = к.set_source_surface(цветной, x0, y0);
    let _ = к.paint_with_alpha(основа);
    к.restore().ok();

    // Сама полоса — светлая, наискосок и только по логотипу
    let путь = (t - ПОЛОСА.0) / ПОЛОСА.1;
    if путь > 0.0 && путь < 1.0 {
        let xc = край - з * 0.5;
        let г = gtk::cairo::LinearGradient::new(xc - з * 0.12, 0.0, xc + з * 0.12, 0.0);
        г.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.0);
        г.add_color_stop_rgba(0.5, 1.0, 1.0, 1.0, 0.5 * основа);
        г.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, 0.0);
        // Наклон: на каждую точку вниз — точка влево, вокруг середины знака
        let ym = y0 + з / 2.0;
        г.set_matrix(gtk::cairo::Matrix::new(1.0, 0.0, 1.0, 1.0, -ym, 0.0));
        к.set_operator(gtk::cairo::Operator::Add);
        let _ = к.set_source(&г);
        к.mask_surface(цветной, x0, y0).ok();
    }
    к.restore().ok();
}

/// Три точки под подписью: бегут по очереди, пока стол грузится
fn три_точки(к: &gtk::cairo::Context, cx: f64, y: f64, t: f64, а: f64, шаг: f64) {
    for i in 0..3 {
        let ф = (t * 1.4 - i as f64 * 0.18).rem_euclid(1.0);
        let яр = 0.25 + 0.75 * (std::f64::consts::PI * (ф * 2.0).min(1.0)).sin().max(0.0);
        к.set_source_rgba(1.0, 1.0, 1.0, яр * а);
        к.new_sub_path();
        к.arc(cx + (i as f64 - 1.0) * шаг * 4.7, y, шаг, 0.0, 2.0 * std::f64::consts::PI);
        let _ = к.fill();
    }
}

/// Размеры — от высоты экрана: знак посередине чуть выше центра, под ним
/// подпись и три точки
pub struct Разметка {
    pub cx: f64,
    pub cy: f64,
    pub знак: f64,
    pub текст_y: f64,
    pub точки_y: f64,
}

pub fn разметка(w: f64, h: f64) -> Разметка {
    let cx = w / 2.0;
    let cy = h / 2.0 - h / 22.0;
    let знак = (h / 5.2).floor();
    let текст_y = cy + знак / 2.0 + знак * 0.32;
    let точки_y = текст_y + знак * 0.24 + 10.0;
    Разметка { cx, cy, знак, текст_y, точки_y }
}

fn плавно(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Знак, уменьшенный один раз и хорошо (а не растянутый на каждом кадре):
/// цветной и тёмный — серый и притемнённый, основа для блика
fn знаки(сторона: f64) -> Option<(gtk::cairo::ImageSurface, gtk::cairo::ImageSurface)> {
    let путь = crate::apps::файл_системы("logo-dark.png").or_else(|| crate::apps::файл_системы("logo.png"))?;
    let исходная = gtk::gdk_pixbuf::Pixbuf::from_file(путь).ok()?;
    let ш = сторона.max(1.0) as i32;
    let м = исходная.scale_simple(ш, ш, gtk::gdk_pixbuf::InterpType::Hyper)?;
    let серая = м.copy()?;
    м.saturate_and_pixelate(&серая, 0.0, false);
    let в_поверхность = |п: &gtk::gdk_pixbuf::Pixbuf, темнее: f64| -> Option<gtk::cairo::ImageSurface> {
        let пов = gtk::cairo::ImageSurface::create(gtk::cairo::Format::ARgb32, ш, ш).ok()?;
        {
            let к = gtk::cairo::Context::new(&пов).ok()?;
            к.set_source_pixbuf(п, 0.0, 0.0);
            к.paint().ok()?;
            if темнее > 0.0 {
                к.set_operator(gtk::cairo::Operator::Atop);
                к.set_source_rgba(0.0, 0.0, 0.0, темнее);
                к.paint().ok()?;
            }
        }
        Some(пов)
    };
    Some((в_поверхность(&м, 0.0)?, в_поверхность(&серая, 0.65)?))
}

struct Картинки {
    размер: (i32, i32),
    знаки: Option<(gtk::cairo::ImageSurface, gtk::cairo::ImageSurface)>,
}

pub fn запусти(доводы: Vec<String>) -> ! {
    if let Some(i) = доводы.iter().position(|д| д == "--кадры") {
        let папка = доводы.get(i + 1).cloned().unwrap_or_else(|| ".".into());
        let логотип = доводы.get(i + 2).cloned().unwrap_or_else(|| "assets/logo-dark.png".into());
        match нарисуй_кадры(&папка, &логотип, доводы.get(i + 3..).unwrap_or(&[])) {
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

    let подпись = if вход { String::new() } else { crate::i18n::tr("Добро пожаловать") };
    let начало = Instant::now();
    let гаснет: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));
    let картинки: Rc<RefCell<Option<Картинки>>> = Rc::new(RefCell::new(None));

    {
        let (гаснет, картинки) = (гаснет.clone(), картинки.clone());
        окно.connect_draw(move |о, к| {
            let (ш, в) = (о.allocated_width(), о.allocated_height());
            let (w, h) = (ш as f64, в as f64);
            let р = разметка(w, h);
            // Картинки — под этот размер экрана, один раз
            if картинки.borrow().as_ref().map(|к| к.размер) != Some((ш, в)) {
                *картинки.borrow_mut() = Some(Картинки { размер: (ш, в), знаки: знаки(р.знак) });
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

            // Фон — чёрный, как у заставки загрузки
            к.set_source_rgb(0.0, 0.0, 0.0);
            let _ = к.paint();

            if let Some((цветной, тёмный)) = картинки.borrow().as_ref().and_then(|к| к.знаки.as_ref()) {
                блик(к, &р, t, цветной, тёмный);
            }

            // Подпись — системным шрифтом, и три точки под ней
            if !подпись.is_empty() {
                let слой = pangocairo::functions::create_layout(к);
                let mut шрифт = gtk::pango::FontDescription::new();
                шрифт.set_family("Sans");
                шрифт.set_weight(gtk::pango::Weight::Normal);
                шрифт.set_absolute_size((р.знак / 9.0).clamp(15.0, 28.0) * gtk::pango::SCALE as f64);
                слой.set_font_description(Some(&шрифт));
                слой.set_text(&подпись);
                let (тш, _) = слой.pixel_size();
                let а = плавно((t - ПОДПИСЬ.0) / ПОДПИСЬ.1);
                к.set_source_rgba(1.0, 1.0, 1.0, 0.86 * а);
                к.move_to((р.cx - тш as f64 / 2.0).round(), р.текст_y.round());
                pangocairo::functions::show_layout(к, &слой);
                три_точки(к, р.cx, р.точки_y, t, а, (р.знак / 69.0).max(2.0));
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
    fn блик_открывает_весь_знак() {
        // До полосы цвета нет, после — весь знак цветной (край прошёл и низ)
        assert!(super::край_цвета(0.0) <= -0.4 + 1e-9);
        assert!(super::край_цвета(super::ПОЛОСА.0 + super::ПОЛОСА.1) - 1.0 >= 1.0);
        // Основа проявляется раньше, чем пойдёт полоса, а подпись — после неё
        assert!(super::ОСНОВА.0 + super::ОСНОВА.1 <= super::ПОЛОСА.0);
        assert!(super::ПОЛОСА.0 + super::ПОЛОСА.1 <= super::ПОДПИСЬ.0);
    }

    #[test]
    fn случай_как_в_сценарии_plymouth() {
        let mut с = super::Случай::new();
        let a = с.next();
        assert!((a - (20251107.0 * 16807.0 % 2147483647.0) / 2147483647.0).abs() < 1e-12);
    }

    #[test]
    fn разметка_от_высоты_экрана() {
        let р = super::разметка(1920.0, 1080.0);
        assert_eq!(р.знак, 207.0);
        assert_eq!(super::строки::знак(1080.0), 311.0);
    }
}
