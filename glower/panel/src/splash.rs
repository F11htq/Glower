//! Заставка между загрузкой и рабочим столом — как в Windows.
//!
//! При загрузке Plymouth (linux/plymouth) держит на чёрном экране мигающий
//! курсор. Под экраном входа (`--вход`) из него собирается знак из строк
//! кода — модуль `код`; экран входа ждёт, пока знак соберётся. После
//! входа, на том же чёрном фоне, — блик: тёмная основа логотипа
//! проявляется из черноты, по ней наискосок проходит полоса света, и за
//! ней знак становится цветным; потом «Добро пожаловать» и три точки, пока
//! стол грузится. Время анимации считается от первого кадра на экране.
//!
//!   glower-splash          — после входа, поверх всего; гаснет, когда
//!                            рабочий стол нарисован
//!                            ($XDG_RUNTIME_DIR/glower-desktop-ready);
//!   glower-splash --вход   — под экраном входа, пока тот загружается.
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
///
/// Перед этим — пауза: монитор гаснет и просыпается, пока экран переходит
/// от экрана входа к рабочему столу, и начало анимации человек не видел
pub const ПАУЗА: f64 = 1.5;
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

/// Знак из строк кода — заставка под экраном входа, белым по чёрному.
///
/// 1. Посреди экрана дважды мигает курсор — тот же, что мигал при загрузке
///    (Plymouth держит только его: на многих машинах его видно секунду-две,
///    и анимация там просто не успевала показаться).
/// 2. Курсор вытягивается в линию через весь экран, линия гаснет.
/// 3. Строки настоящего кода GlowerOS несутся с обеих сторон волной от
///    середины к краям, последние знакоместа проходят рывками и встают так,
///    что яркие знаки складываются в логотип.
/// 4. Лишний код гаснет, под знаком печатается GlowerOS, мигает курсор.
///
/// Экран входа ждёт, пока знак соберётся (файл `код_показан()`).
pub mod код {
    /// Рядов на высоту знака, ещё по стольку сверху и снизу — просто код
    pub const РЯДОВ: usize = 40;
    pub const ЛИШНИХ: usize = 6;
    /// Знак из кода крупнее обычного — иначе мелкие детали теряются
    pub const КРУПНЕЕ: f64 = 1.5;
    /// Ширина строки — в размерах знака
    pub const ШИРИНА: f64 = 20.0;
    pub const ИМЯ: &str = "GlowerOS";
    /// Когда знак собран и название напечатано, секунд
    pub const ГОТОВО: f64 = 4.3;
    /// Сколько курсор просто мигает, прежде чем начнётся сборка: монитор
    /// только что погас и проснулся — экран переходит от Plymouth к экрану
    /// входа, — и первые секунду-две человек ничего не видит
    pub const ПАУЗА: f64 = 1.5;

    /// Строки — из настоящего кода системы
    pub const ИСХОДНИКИ: [&str; 6] = [
        include_str!("../../wm/src/hang.rs"),
        include_str!("../../agent/antenna.mjs"),
        include_str!("splash.rs"),
        include_str!("../../wm/src/state.rs"),
        include_str!("../../agent/clock.mjs"),
        include_str!("../../agent/games.mjs"),
    ];

    pub fn знак(h: f64) -> f64 {
        (h / 5.2 * КРУПНЕЕ).floor()
    }

    /// Строки кода подряд: пустые и комментарии пропускаем
    pub fn строки() -> Vec<&'static str> {
        ИСХОДНИКИ
            .iter()
            .flat_map(|и| и.lines())
            .map(str::trim)
            .filter(|с| с.chars().count() >= 12 && !["//", "/*", "*", "#"].iter().any(|н| с.starts_with(н)))
            .collect()
    }

    /// Курсор посреди экрана мигает дважды
    pub fn курсор(t: f64) -> bool {
        if t < 0.0 {
            // Пока монитор просыпается — мигает, как в Plymouth: полсекунды
            // горит, полсекунды нет
            return ((t + ПАУЗА) / 0.5).floor() as i64 % 2 == 0;
        }
        t < 0.62 && (t / 0.16).floor() as i64 % 2 == 0
    }

    /// Линия из курсора: (доля ширины 0..1, прозрачность)
    pub fn линия(t: f64) -> Option<(f64, f64)> {
        if !(0.62..1.25).contains(&t) {
            return None;
        }
        Some((super::плавно((t - 0.62) / 0.28), 1.0 - super::плавно((t - 0.95) / 0.3)))
    }

    /// Ряд: несётся, тормозит, последние знакоместа проходит рывками.
    /// (сдвиг в знакоместах, видно)
    pub fn ряд(задержка: f64, длит: f64, сдвиг: f64, t: f64) -> (f64, f64) {
        if t < задержка {
            return (сдвиг, 0.0);
        }
        let p = ((t - задержка) / длит).min(1.0);
        let mut dx = сдвиг * (1.0 - p).powi(4);
        if dx.abs() < 8.0 {
            dx = dx.round();
        }
        (dx, (p * 3.0).min(1.0))
    }

    /// Тусклый код: 0,3 и гаснет до 0,05, когда знак собран
    pub fn тусклость(t: f64) -> f64 {
        0.3 - 0.25 * super::плавно((t - 2.9) / 0.8)
    }

    /// Сколько букв названия напечатано
    pub fn набрано(t: f64) -> usize {
        if t < 3.35 {
            return 0;
        }
        (((t - 3.5) / 0.08).floor().max(0.0) as usize).min(ИМЯ.chars().count())
    }
}

struct Ряд {
    тусклый: gtk::cairo::ImageSurface,
    лого: Option<gtk::cairo::ImageSurface>,
    y: f64,
    задержка: f64,
    длит: f64,
    сдвиг: f64,
}

struct КодЛого {
    ряды: Vec<Ряд>,
    клетка: f64,
    ш_ряда: f64,
    ш_лого: f64,
    ряд_в: f64,
}

/// Ряды кода под этот экран — один раз: тусклый код и знаки логотипа
/// (яркость — по светлоте логотипа под знаком)
fn собери_код(h: f64) -> Option<КодЛого> {
    use gtk::cairo::{Context, FontSlant, FontWeight, Format, ImageSurface};
    use код::*;
    let путь = crate::apps::файл_системы("logo-dark.png").or_else(|| crate::apps::файл_системы("logo.png"))?;
    let исходный = gtk::gdk_pixbuf::Pixbuf::from_file(путь).ok()?;
    let cy = h / 2.0 - h / 22.0;
    let знак = код::знак(h);
    let ряд_в = знак / РЯДОВ as f64;
    let шрифт = ряд_в * 0.98;
    let выбери = |к: &Context, жирный: bool| {
        к.select_font_face("DejaVu Sans Mono", FontSlant::Normal, if жирный { FontWeight::Bold } else { FontWeight::Normal });
        к.set_font_size(шрифт);
    };
    let пробная = ImageSurface::create(Format::ARgb32, 8, 8).ok()?;
    let кп = Context::new(&пробная).ok()?;
    выбери(&кп, true);
    let клетка = кп.text_extents("M").ok()?.x_advance().round().max(1.0) as usize;
    let fe = кп.font_extents().ok()?;
    let столбцов = (знак / клетка as f64).ceil() as usize;
    let mut всего = (ШИРИНА * знак / клетка as f64).ceil() as usize;
    if (всего - столбцов) % 2 == 1 {
        всего += 1;
    }
    let лево = (всего - столбцов) / 2;
    let в = ряд_в.ceil() as i32;
    let базовая = в as f64 / 2.0 + (fe.ascent() - fe.descent()) / 2.0;

    // Светлота и прозрачность логотипа по клеткам
    let м = исходный.scale_simple((столбцов * 4) as i32, (РЯДОВ * 4) as i32, gtk::gdk_pixbuf::InterpType::Hyper)?;
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

    let строки = код::строки();
    let mut с = Случай::new();
    let mut номер = (с.next() * строки.len() as f64) as usize;
    let середина = (РЯДОВ as f64 - 1.0) / 2.0;
    let mut ряды = vec![];
    for i in 0..РЯДОВ + 2 * ЛИШНИХ {
        let mut знаки: Vec<char> = vec![];
        while знаки.len() < всего {
            знаки.extend(строки[номер % строки.len()].chars());
            знаки.extend("  ".chars());
            номер += 1;
        }
        знаки.truncate(всего);
        let ряд_лого = i.checked_sub(ЛИШНИХ).filter(|r| *r < РЯДОВ);
        let тусклый = ImageSurface::create(Format::ARgb32, (всего * клетка) as i32, в).ok()?;
        let лого = ImageSurface::create(Format::ARgb32, (столбцов * клетка) as i32, в).ok()?;
        {
            let т = Context::new(&тусклый).ok()?;
            let л = Context::new(&лого).ok()?;
            выбери(&т, false);
            выбери(&л, true);
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
        let (r0, r1, r2, r3) = (с.next(), с.next(), с.next(), с.next());
        let сторона = if r2 < 0.5 { -1.0 } else { 1.0 };
        let даль = ((i as f64 - ЛИШНИХ as f64 - середина).abs() / (середина + ЛИШНИХ as f64)).min(1.0);
        ряды.push(Ряд {
            тусклый,
            лого: ряд_лого.map(|_| лого),
            y: (cy - знак / 2.0 + (i as f64 - ЛИШНИХ as f64) * ряд_в).round(),
            задержка: 0.85 + 1.0 * даль + 0.3 * r0,
            длит: 0.8 + 0.5 * r3,
            сдвиг: сторона * ((2.5 + 6.0 * r1) * знак / клетка as f64).round(),
        });
    }
    Some(КодЛого { ряды, клетка: клетка as f64, ш_ряда: (всего * клетка) as f64, ш_лого: (столбцов * клетка) as f64, ряд_в })
}

/// Знак из кода в момент t
fn рисуй_код(к: &gtk::cairo::Context, w: f64, h: f64, кл: &КодЛого, t: f64) {
    use gtk::cairo::{FontSlant, FontWeight};
    let cx = w / 2.0;
    let cy = h / 2.0 - h / 22.0;
    let знак = код::знак(h);
    к.save().ok();
    if код::курсор(t) {
        к.set_source_rgb(1.0, 1.0, 1.0);
        к.rectangle((cx - кл.клетка / 2.0).round(), (cy - кл.ряд_в).round(), кл.клетка.ceil(), (кл.ряд_в * 2.0).ceil());
        let _ = к.fill();
    }
    if let Some((u, а)) = код::линия(t) {
        let шл = кл.клетка + u * (w - кл.клетка);
        let вл = (кл.ряд_в * 2.0 * (1.0 - u) + 2.0 * u).max(1.0);
        к.set_source_rgba(1.0, 1.0, 1.0, а);
        к.rectangle((cx - шл / 2.0).round(), (cy - вл / 2.0).round(), шл.round(), вл.round());
        let _ = к.fill();
    }
    let тускл = код::тусклость(t);
    for р in &кл.ряды {
        let (dx, видно) = код::ряд(р.задержка, р.длит, р.сдвиг, t);
        if видно <= 0.0 {
            continue;
        }
        let сдвиг = dx * кл.клетка;
        let _ = к.set_source_surface(&р.тусклый, (cx - кл.ш_ряда / 2.0).floor() + сдвиг, р.y);
        let _ = к.paint_with_alpha(видно * тускл);
        if let Some(л) = &р.лого {
            let _ = к.set_source_surface(л, (cx - кл.ш_лого / 2.0).floor() + сдвиг, р.y);
            let _ = к.paint_with_alpha(видно);
        }
    }
    // Название печатается по букве, курсор мигает
    if t > 3.35 {
        let кегль = (знак * 0.14).round();
        к.select_font_face("DejaVu Sans Mono", FontSlant::Normal, FontWeight::Bold);
        к.set_font_size(кегль);
        let кл_имени = к.text_extents("M").map(|е| е.x_advance()).unwrap_or(кегль * 0.6).round();
        let вверх = к.font_extents().map(|е| е.ascent()).unwrap_or(кегль);
        let всего = код::ИМЯ.chars().count() as f64 + 1.0;
        let x0 = (cx - всего * кл_имени / 2.0).round();
        let y0 = (cy + знак / 2.0 + знак * 0.16).round();
        let n = код::набрано(t);
        к.set_source_rgb(1.0, 1.0, 1.0);
        for (j, ch) in код::ИМЯ.chars().take(n).enumerate() {
            к.move_to(x0 + j as f64 * кл_имени, y0 + вверх);
            let _ = к.show_text(&ch.to_string());
        }
        if n < код::ИМЯ.chars().count() || ((t - 3.5) / 0.5).floor() as i64 % 2 == 0 {
            к.rectangle(x0 + n as f64 * кл_имени + кл_имени * 0.1, y0 + кегль * 0.1, кл_имени * 0.8, кегль);
            let _ = к.fill();
        }
    }
    к.restore().ok();
}

/// Знак из кода собран — экрану входа можно показываться
pub fn код_показан() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into())).join("glower-splash-shown")
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
    код: Option<КодЛого>,
}

pub fn запусти(доводы: Vec<String>) -> ! {
    if gtk::init().is_err() {
        std::process::exit(1);
    }
    crate::i18n::подключи();
    let вход = доводы.iter().any(|д| д == "--вход");
    if !вход {
        let _ = std::fs::remove_file(файл_готовности());
    } else {
        let _ = std::fs::remove_file(код_показан());
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
    // Сорок секунд — от запуска; а время анимации — от первого кадра на
    // экране: пока сеанс поднимается, проходит секунда-другая, и раньше
    // человек видел уже конец анимации
    let начало = Instant::now();
    let первый: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));
    let гаснет: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));
    let картинки: Rc<RefCell<Option<Картинки>>> = Rc::new(RefCell::new(None));

    {
        let (гаснет, картинки, первый) = (гаснет.clone(), картинки.clone(), первый.clone());
        окно.connect_draw(move |о, к| {
            let (ш, в) = (о.allocated_width(), о.allocated_height());
            let (w, h) = (ш as f64, в as f64);
            let р = разметка(w, h);
            // Картинки — под этот размер экрана, один раз
            if картинки.borrow().as_ref().map(|к| к.размер) != Some((ш, в)) {
                *картинки.borrow_mut() = Some(Картинки {
                    размер: (ш, в),
                    знаки: if вход { None } else { знаки(р.знак) },
                    код: if вход { собери_код(h) } else { None },
                });
            }
            // Картинки готовы — этот кадр и есть начало
            let t = match первый.get() {
                Some(с) => с.elapsed().as_secs_f64(),
                None => {
                    первый.set(Some(Instant::now()));
                    0.0
                }
            } - if вход { код::ПАУЗА } else { ПАУЗА };
            if вход && t >= код::ГОТОВО && !код_показан().exists() {
                let _ = std::fs::write(код_показан(), b"1");
            }
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

            if let Some(кар) = картинки.borrow().as_ref() {
                if let Some((цветной, тёмный)) = кар.знаки.as_ref() {
                    блик(к, &р, t, цветной, тёмный);
                }
                if let Some(кл) = кар.код.as_ref() {
                    рисуй_код(к, w, h, кл, t);
                }
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
        assert_eq!(super::код::знак(1080.0), 311.0);
    }

    #[test]
    fn знак_из_кода_собирается_по_порядку() {
        use super::код::*;
        // Курсор — в начале, линия — после него, ряды встают к «готово»
        assert!(курсор(0.0) && !курсор(0.2) && !курсор(1.0));
        // Пока монитор просыпается — мигает с самого первого кадра
        assert!(курсор(-ПАУЗА) && !курсор(-ПАУЗА + 0.6));
        assert!(линия(0.5).is_none() && линия(0.8).is_some() && линия(1.3).is_none());
        assert_eq!(ряд(0.85, 0.8, 40.0, 0.5), (40.0, 0.0));
        assert_eq!(ряд(2.15, 1.3, -40.0, ГОТОВО - 0.8), (0.0, 1.0));
        assert_eq!(набрано(3.0), 0);
        assert_eq!(набрано(ГОТОВО), ИМЯ.chars().count());
        assert!(тусклость(ГОТОВО) < 0.06);
        // Строки кода есть
        assert!(строки().len() > 300);
    }
}
