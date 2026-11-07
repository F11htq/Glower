//! Заставка между загрузкой и рабочим столом — как в Windows.
//!
//! Заставка загрузки (Plymouth, linux/plymouth) собирает логотип из строк
//! кода белым по чёрному; строки для неё рисует `--кадры` (модуль
//! `строки`). После входа, на том же чёрном фоне, — настоящий логотип:
//! он собирается из полос вместе с тонкими лучами (с `--показ`), а
//! обычно сразу стоит собранным, под надписью бежит черта загрузки.
//!
//!   glower-splash          — после входа: «Добро пожаловать», поверх всего;
//!                            гаснет, когда рабочий стол нарисован
//!                            ($XDG_RUNTIME_DIR/glower-desktop-ready);
//!   glower-splash --вход   — под экраном входа, пока тот загружается;
//!   glower-splash --показ  — сборка целиком, посмотреть;
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

/// Сколько длится сборка логотипа, секунд. Дальше — спокойная заставка:
/// черта загрузки под надписью и редкие тихие лучи
pub const ИНТРО: f64 = 2.6;
/// На сколько полос режется логотип и надпись
pub const ПОЛОС_ЗНАКА: usize = 24;
pub const ПОЛОС_СЛОВА: usize = 8;
/// Лучей, пролетающих при сборке, и тихих — потом
pub const ЛУЧЕЙ: usize = 30;
pub const ФОНОВЫХ: usize = 8;
/// Период тихих лучей и бегунка на черте
pub const ФОН_ПЕРИОД: f64 = 4.0;
pub const БЕГУНОК_ПЕРИОД: f64 = 1.7;
/// Цвета лучей: голубой, белый, тёплый — как в знаке
pub const ЦВЕТА: [(f64, f64, f64); 3] = [(0.36, 0.76, 1.0), (1.0, 1.0, 1.0), (1.0, 0.72, 0.42)];
const ХВОСТ: (f64, f64, f64) = (0.25, 0.45, 0.85);

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

/// Полоса логотипа или надписи: прилетает сбоку и встаёт на место
#[derive(Clone, Copy)]
pub struct Полоса {
    pub задержка: f64,
    pub длит: f64,
    /// Откуда прилетает — в долях размера знака, со знаком стороны
    pub сдвиг: f64,
    pub сбой_t: f64,
    /// Короткий сбой уже на месте (как помеха в сигнале), в долях знака; 0 — без сбоя
    pub сбой: f64,
}

/// Луч: тонкая светлая строка поперёк экрана
#[derive(Clone, Copy)]
pub struct Луч {
    /// Высота — доля полосы вокруг логотипа
    pub y: f64,
    /// Длина — в долях знака
    pub длина: f64,
    pub цвет: usize,
    pub начало: f64,
    pub длит: f64,
    /// 1 — слева направо, −1 — справа налево
    pub напр: f64,
    pub толстый: bool,
}

pub struct Сценарий {
    pub знак: Vec<Полоса>,
    pub слово: Vec<Полоса>,
    pub лучи: Vec<Луч>,
    pub фон: Vec<Луч>,
}

/// Порядок чисел: полосы знака, полосы надписи, лучи, тихие лучи — по пять
/// чисел на каждую
pub fn сценарий() -> Сценарий {
    let mut с = Случай::new();
    let mut полоса = |нач: f64, разброс: f64, длит: f64, мин: f64, макс: f64, сбой_нач: f64| {
        let (r0, r1, r2, r3, r4) = (с.next(), с.next(), с.next(), с.next(), с.next());
        let сторона = if r2 < 0.5 { -1.0 } else { 1.0 };
        Полоса {
            задержка: нач + разброс * r0,
            длит,
            сдвиг: сторона * (мин + (макс - мин) * r1),
            сбой_t: сбой_нач + 0.5 * r3,
            сбой: if (r4 - 0.5).abs() > 0.25 { (r4 - 0.5) * 0.12 } else { 0.0 },
        }
    };
    let знак = (0..ПОЛОС_ЗНАКА).map(|_| полоса(0.30, 0.85, 0.75, 1.2, 4.0, 1.45)).collect();
    let слово = (0..ПОЛОС_СЛОВА).map(|_| полоса(1.05, 0.45, 0.60, 0.8, 2.4, 1.75)).collect();
    let mut луч = |k: usize, фон: bool| {
        let (r0, r1, r2, r3, r4) = (с.next(), с.next(), с.next(), с.next(), с.next());
        Луч {
            y: r0,
            длина: 0.4 + 2.2 * r1,
            цвет: if r2 < 0.55 { 0 } else if r2 < 0.8 { 1 } else { 2 },
            начало: if фон { r3 * ФОН_ПЕРИОД } else { 0.05 + 1.4 * r3 },
            длит: if фон { 0.9 + 0.6 * r4 } else { 0.5 + 0.6 * r4 },
            напр: if k % 2 == 0 { 1.0 } else { -1.0 },
            толстый: k % 3 == 0,
        }
    };
    let лучи = (0..ЛУЧЕЙ).map(|k| луч(k, false)).collect();
    let фон = (0..ФОНОВЫХ).map(|k| луч(k, true)).collect();
    Сценарий { знак, слово, лучи, фон }
}

/// Где полоса в момент t: (сдвиг в долях знака, прозрачность, след 0..1)
pub fn полоса_в(п: &Полоса, t: f64) -> (f64, f64, f64) {
    let p = ((t - п.задержка) / п.длит).clamp(0.0, 1.0);
    if t < п.задержка {
        return (п.сдвиг, 0.0, 0.0);
    }
    let е = 1.0 - (1.0 - p).powi(3);
    let mut dx = п.сдвиг * (1.0 - е);
    if п.сбой != 0.0 && t >= п.сбой_t && t < п.сбой_t + 0.08 {
        dx += п.сбой;
    }
    (dx, (p * 2.5).min(1.0), (1.0 - е) * (p * 4.0).min(1.0))
}

/// Луч в момент t на экране шириной w: (левый край, прозрачность) или None.
/// Тихие лучи повторяются с периодом и светят втрое слабее
pub fn луч_в(л: &Луч, t: f64, w: f64, длина: f64, фон: bool) -> Option<(f64, f64)> {
    let местное = if фон {
        if t < ИНТРО {
            return None;
        }
        (t - ИНТРО + ФОН_ПЕРИОД - л.начало).rem_euclid(ФОН_ПЕРИОД)
    } else {
        t - л.начало
    };
    let q = местное / л.длит;
    if !(0.0..1.0).contains(&q) {
        return None;
    }
    let путь = q * (w + длина);
    let x = if л.напр > 0.0 { -длина + путь } else { w - путь };
    let а = 4.0 * q * (1.0 - q) * if фон { 0.3 } else { 1.0 };
    Some((x, а))
}

/// Черта загрузки: (видно, доля пути бегунка 0..1)
pub fn черта_в(t: f64) -> (f64, f64) {
    let видно = ((t - 2.2) / 0.4).clamp(0.0, 1.0);
    let q = ((t - 2.2).max(0.0) / БЕГУНОК_ПЕРИОД).rem_euclid(1.0);
    (видно, q)
}

/// Блик сверху вниз по собранному логотипу: (доля пути, прозрачность)
pub fn блик_в(t: f64) -> Option<(f64, f64)> {
    let q = (t - 1.95) / 0.5;
    if !(0.0..1.0).contains(&q) {
        return None;
    }
    Some((q, 4.0 * q * (1.0 - q) * 0.7))
}

/// Толщина луча на экране
pub fn толщина_луча(знак: f64, толстый: bool) -> f64 {
    (знак * if толстый { 0.016 } else { 0.010 }).floor().max(1.0)
}

/// Полосы режутся по целым точкам: i-я — от floor(i·h/n) до floor((i+1)·h/n)
pub fn край_полосы(i: usize, h: f64, n: usize) -> f64 {
    (i as f64 * h / n as f64).floor()
}

fn смешай(а: (f64, f64, f64), б: (f64, f64, f64), u: f64) -> (f64, f64, f64) {
    (а.0 + (б.0 - а.0) * u, а.1 + (б.1 - а.1) * u, а.2 + (б.2 - а.2) * u)
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

/// Градиент луча: хвост тает в голубую темноту, голова — своего цвета.
/// напр 1 — голова справа
fn градиент_луча(x: f64, длина: f64, напр: f64, цвет: (f64, f64, f64), а: f64) -> gtk::cairo::LinearGradient {
    let (x0, x1) = if напр > 0.0 { (x, x + длина) } else { (x + длина, x) };
    let г = gtk::cairo::LinearGradient::new(x0, 0.0, x1, 0.0);
    for i in 0..=8 {
        let u = i as f64 / 8.0;
        let (r, g, b) = смешай(ХВОСТ, цвет, u.powf(0.7));
        г.add_color_stop_rgba(u, r, g, b, а * u.powf(1.6));
    }
    г
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

/// Вся сцена в момент t: лучи, полосы логотипа и надписи со следами, блик,
/// черта загрузки. Логотип и надпись — уже уменьшенные под экран
pub fn сцена(к: &gtk::cairo::Context, р: &Разметка, w: f64, сц: &Сценарий, t: f64, знак: Option<&gtk::cairo::ImageSurface>, слово: Option<&gtk::cairo::ImageSurface>) {
    к.save().ok();
    // Лучи — за логотипом
    let полоса_лучей = |y: f64| р.луч_верх + y * (р.луч_низ - р.луч_верх);
    for (лучи, фон) in [(&сц.лучи, false), (&сц.фон, true)] {
        for л in лучи.iter() {
            let длина = р.знак * л.длина;
            if let Some((x, а)) = луч_в(л, t, w, длина, фон) {
                let тл = толщина_луча(р.знак, л.толстый);
                let y = полоса_лучей(л.y).floor();
                let _ = к.set_source(&градиент_луча(x, длина, л.напр, ЦВЕТА[л.цвет], а));
                к.rectangle(x, y, длина, тл);
                let _ = к.fill();
            }
        }
    }
    // Полосы со следами
    let полосы = |к: &gtk::cairo::Context, кар: Option<&gtk::cairo::ImageSurface>, пп: &[Полоса], лево: f64, верх: f64, ш: f64, в: f64| {
        let Some(кар) = кар else { return };
        let n = пп.len();
        for (i, п) in пп.iter().enumerate() {
            let (dx, а, след) = полоса_в(п, t);
            if а <= 0.0 {
                continue;
            }
            let y0 = верх + край_полосы(i, в, n);
            let hh = верх + край_полосы(i + 1, в, n) - y0;
            let x = лево + dx * р.знак;
            if след > 0.01 {
                let дл = р.знак * 0.9;
                let (сх, напр) = if п.сдвиг < 0.0 { (x - дл, 1.0) } else { (x + ш, -1.0) };
                let _ = к.set_source(&градиент_луча(сх, дл, напр, (0.8, 0.9, 1.0), 0.7 * след));
                к.rectangle(сх, y0, дл, hh);
                let _ = к.fill();
            }
            к.save().ok();
            к.rectangle(x, y0, ш, hh);
            к.clip();
            let _ = к.set_source_surface(кар, x, верх);
            let _ = к.paint_with_alpha(а);
            к.restore().ok();
        }
    };
    let (зл, зв) = ((р.cx - р.знак / 2.0).round(), (р.cy - р.знак / 2.0).round());
    полосы(к, знак, &сц.знак, зл, зв, р.знак, р.знак);
    полосы(к, слово, &сц.слово, (р.cx - р.слово_ш / 2.0).round(), р.слово_y.round(), р.слово_ш, р.слово_в);

    if let Some((q, а)) = блик_в(t) {
        let бв = (р.знак * 0.035).max(2.0);
        let y = зв - бв + q * (р.слово_y + р.слово_в - зв + бв);
        let бш = р.знак * 1.6;
        let г = gtk::cairo::LinearGradient::new(р.cx - бш / 2.0, 0.0, р.cx + бш / 2.0, 0.0);
        г.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.0);
        г.add_color_stop_rgba(0.5, 1.0, 1.0, 1.0, а);
        г.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, 0.0);
        let _ = к.set_source(&г);
        к.rectangle(р.cx - бш / 2.0, y, бш, бв);
        let _ = к.fill();
    }

    let (видно, q) = черта_в(t);
    if видно > 0.0 {
        let лево = (р.cx - р.слово_ш / 2.0).round();
        к.set_source_rgba(1.0, 1.0, 1.0, 0.12 * видно);
        к.rectangle(лево, р.черта_y, р.слово_ш, р.черта_в);
        let _ = к.fill();
        let сш = (р.слово_ш * 0.3).floor();
        let x = лево - сш + q * (р.слово_ш + сш);
        let а = видно * (3.0 * 4.0 * q * (1.0 - q)).min(1.0);
        let г = gtk::cairo::LinearGradient::new(x, 0.0, x + сш, 0.0);
        г.add_color_stop_rgba(0.0, 0.36, 0.76, 1.0, 0.0);
        г.add_color_stop_rgba(0.5, 0.85, 0.95, 1.0, а);
        г.add_color_stop_rgba(1.0, 0.36, 0.76, 1.0, 0.0);
        к.rectangle(лево, р.черта_y, р.слово_ш, р.черта_в);
        к.clip();
        let _ = к.set_source(&г);
        к.rectangle(x, р.черта_y, сш, р.черта_в);
        let _ = к.fill();
    }
    к.restore().ok();
}

/// Размеры — как в заставке загрузки: от высоты экрана
pub struct Разметка {
    pub cx: f64,
    pub cy: f64,
    pub знак: f64,
    pub слово_ш: f64,
    pub слово_в: f64,
    pub слово_y: f64,
    /// Черта загрузки под надписью
    pub черта_y: f64,
    pub черта_в: f64,
    /// Подписи («Добро пожаловать», «Выключение…») — под чертой
    pub текст_y: f64,
    /// Полоса, в которой летают лучи
    pub луч_верх: f64,
    pub луч_низ: f64,
}

pub fn разметка(w: f64, h: f64, слово_пропорция: f64) -> Разметка {
    let cx = w / 2.0;
    let cy = h / 2.0 - h / 22.0;
    let знак = (h / 5.2).floor();
    let слово_ш = (знак * 1.35).floor();
    let слово_в = (слово_ш * слово_пропорция).floor();
    let слово_y = cy + знак / 2.0 + знак * 0.22;
    let черта_y = (слово_y + слово_в + знак * 0.34).floor();
    let черта_в = (знак / 207.0 * 2.0).floor().max(1.0);
    let текст_y = черта_y + знак * 0.2;
    let луч_верх = cy - знак * 0.75;
    let луч_низ = слово_y + слово_в + знак * 0.15;
    Разметка { cx, cy, знак, слово_ш, слово_в, слово_y, черта_y, черта_в, текст_y, луч_верх, луч_низ }
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

    let пропорция = crate::apps::файл_системы("logo-word.png")
        .and_then(|п| gtk::gdk_pixbuf::Pixbuf::file_info(п).map(|(_, ш, в)| в as f64 / ш.max(1) as f64))
        .unwrap_or(175.0 / 994.0);
    let подпись = if вход { String::new() } else { crate::i18n::tr("Добро пожаловать") };
    // Сборку логотипа уже показал Plymouth — продолжаем с готового
    let с_начала = if доводы.iter().any(|д| д == "--показ") { 0.0 } else { ИНТРО };
    let сц = сценарий();
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
            let t = с_начала + начало.elapsed().as_secs_f64();
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
            let г = gtk::cairo::LinearGradient::new(0.0, 0.0, 0.0, h);
            г.add_color_stop_rgb(0.0, 0.0, 0.0, 0.0);
            г.add_color_stop_rgb(1.0, 0.0, 0.0, 0.0);
            let _ = к.set_source(&г);
            let _ = к.paint();

            if let Some(кар) = картинки.borrow().as_ref() {
                сцена(к, &р, w, &сц, t, кар.знак.as_ref(), кар.слово.as_ref());
            }

            // Подпись — системным шрифтом, проявляется в первые полсекунды
            if !подпись.is_empty() {
                let слой = pangocairo::functions::create_layout(к);
                let mut шрифт = gtk::pango::FontDescription::new();
                шрифт.set_family("Sans");
                шрифт.set_weight(gtk::pango::Weight::Normal);
                шрифт.set_absolute_size((р.знак / 9.0).clamp(15.0, 28.0) * gtk::pango::SCALE as f64);
                слой.set_font_description(Some(&шрифт));
                слой.set_text(&подпись);
                let (тш, _) = слой.pixel_size();
                к.set_source_rgba(1.0, 1.0, 1.0, 0.86 * плавно((t - с_начала - 0.1) / 0.5));
                к.move_to((р.cx - тш as f64 / 2.0).round(), р.текст_y.round());
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
    fn сборка_сходится_и_лучи_гаснут() {
        let сц = super::сценарий();
        assert_eq!(сц.знак.len(), super::ПОЛОС_ЗНАКА);
        // К концу сборки все полосы на месте и непрозрачны
        for п in сц.знак.iter().chain(сц.слово.iter()) {
            let (dx, а, след) = super::полоса_в(п, super::ИНТРО);
            assert!(dx == 0.0 && а == 1.0 && след == 0.0);
            assert!(п.сдвиг.abs() >= 0.8 && п.задержка + п.длит <= 2.2);
        }
        // Полосы летят с обеих сторон
        assert!(сц.знак.iter().any(|п| п.сдвиг < 0.0) && сц.знак.iter().any(|п| п.сдвиг > 0.0));
        // Лучи сборки после неё не светят, тихие — светят слабо
        assert!(сц.лучи.iter().all(|л| super::луч_в(л, super::ИНТРО, 1920.0, 200.0, false).is_none()));
        let тихие: Vec<f64> = (0..400).flat_map(|i| сц.фон.iter().filter_map(move |л| super::луч_в(л, 3.0 + i as f64 * 0.02, 1920.0, 200.0, true).map(|(_, а)| а))).collect();
        assert!(!тихие.is_empty() && тихие.iter().all(|а| *а <= 0.3));
    }

    #[test]
    fn случай_как_в_сценарии_plymouth() {
        // Первые числа генератора — их же считает glower.script
        let mut с = super::Случай::new();
        let a = с.next();
        assert!((a - (20251107.0 * 16807.0 % 2147483647.0) / 2147483647.0).abs() < 1e-12);
    }

    #[test]
    fn разметка_как_у_заставки_загрузки() {
        // 1920×1080: знак 207, надпись 279 в ширину — те же числа, что
        // считает linux/plymouth/glower.script
        let р = super::разметка(1920.0, 1080.0, 175.0 / 994.0);
        assert_eq!(р.знак, 207.0);
        assert_eq!(р.слово_ш, 279.0);
        assert_eq!(р.черта_y, 759.0);
    }
}

