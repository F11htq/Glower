//! Ножницы — нативные, настоящим окном (glower-snip).
//!
//! Снимает экран сам, программой grim. Область выделяется поверх
//! застывшего кадра: так рамку тянут по тому, что было на экране в миг
//! нажатия, и ничего не успевает сдвинуться. Готовый снимок сразу кладётся
//! в буфер обмена, а окно открывается, чтобы его подписать или сохранить.
//!
//!   glower-snip           — окно
//!   glower-snip экран     — снять экран целиком (PrintScreen)
//!   glower-snip область   — выделить область (Win+Shift+S)
//!   glower-snip запись    — записать видео с экрана (Win+Shift+R); если
//!                           запись уже идёт — остановить её

use crate::apps::{кнопка, подготовь};
use gtk::gdk_pixbuf::Pixbuf;
use gtk::glib;
use gtk::prelude::*;
use gtk_layer_shell::LayerShell;
use std::cell::{Cell, RefCell};
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

const СТИЛЬ: &str = r#"
.snip .stage { background-color: #0b0c10; }
.snip .color { min-width: 22px; min-height: 22px; border-radius: 11px; padding: 0; border: 2px solid rgba(255, 255, 255, 0.18); }
.snip .color.on { border-color: #ffffff; }
.snip button.on { background-image: linear-gradient(140deg, #5b8cff, #8b5cf6); }
.snip .note { font-size: 12px; opacity: 0.7; }
window.record-bar { border-radius: 14px; background-color: rgba(22, 24, 32, 0.95); }
window.record-bar .dot { color: #ef4444; font-size: 16px; }
"#;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Инструмент {
    Перо,
    Маркер,
    Стрелка,
    Рамка,
    Замазать,
    Текст,
}

#[derive(Clone, Debug)]
pub struct Штрих {
    pub вид: Инструмент,
    pub цвет: (f64, f64, f64),
    pub толщина: f64,
    pub точки: Vec<(f64, f64)>,
    pub текст: String,
}

impl Штрих {
    fn начало(&self) -> (f64, f64) {
        self.точки.first().copied().unwrap_or_default()
    }
    fn конец(&self) -> (f64, f64) {
        self.точки.last().copied().unwrap_or_default()
    }
    /// Рамку и стрелку нулевого размера не храним: это просто щелчок
    pub fn пустой(&self) -> bool {
        let ((x1, y1), (x2, y2)) = (self.начало(), self.конец());
        matches!(self.вид, Инструмент::Стрелка | Инструмент::Рамка | Инструмент::Замазать) && (x2 - x1).abs() < 3.0 && (y2 - y1).abs() < 3.0
    }
}

pub fn нарисуй_штрих(к: &gtk::cairo::Context, ш: &Штрих) {
    к.save().ok();
    к.set_line_cap(gtk::cairo::LineCap::Round);
    к.set_line_join(gtk::cairo::LineJoin::Round);
    let (r, g, b) = ш.цвет;
    к.set_source_rgb(r, g, b);
    к.set_line_width(ш.толщина);
    let ((x1, y1), (x2, y2)) = (ш.начало(), ш.конец());
    match ш.вид {
        Инструмент::Перо | Инструмент::Маркер => {
            if ш.вид == Инструмент::Маркер {
                к.set_source_rgba(r, g, b, 0.35);
                к.set_line_width(ш.толщина * 4.0);
            }
            for (i, (x, y)) in ш.точки.iter().enumerate() {
                if i == 0 { к.move_to(*x, *y) } else { к.line_to(*x, *y) }
            }
            if ш.точки.len() == 1 {
                к.line_to(x1 + 0.1, y1);
            }
            let _ = к.stroke();
        }
        Инструмент::Рамка => {
            к.rectangle(x1, y1, x2 - x1, y2 - y1);
            let _ = к.stroke();
        }
        Инструмент::Стрелка => {
            let угол = (y2 - y1).atan2(x2 - x1);
            let длина = (ш.толщина * 4.0).max(12.0);
            к.move_to(x1, y1);
            к.line_to(x2, y2);
            let _ = к.stroke();
            к.move_to(x2, y2);
            к.line_to(x2 - длина * (угол - 0.4).cos(), y2 - длина * (угол - 0.4).sin());
            к.line_to(x2 - длина * (угол + 0.4).cos(), y2 - длина * (угол + 0.4).sin());
            к.close_path();
            let _ = к.fill();
        }
        Инструмент::Замазать => {
            // Плотный прямоугольник надёжнее размытия: из размытого текст иногда читают
            к.set_source_rgb(0.067, 0.094, 0.153);
            к.rectangle(x1, y1, x2 - x1, y2 - y1);
            let _ = к.fill();
        }
        Инструмент::Текст => {
            к.select_font_face("sans-serif", gtk::cairo::FontSlant::Normal, gtk::cairo::FontWeight::Bold);
            к.set_font_size(ш.толщина * 6.0);
            if let Ok(р) = к.font_extents() {
                к.move_to(x1, y1 + р.ascent());
            }
            к.text_path(&ш.текст);
            к.set_source_rgba(0.0, 0.0, 0.0, 0.55);
            к.set_line_width(3.0);
            let _ = к.stroke_preserve();
            к.set_source_rgb(r, g, b);
            let _ = к.fill();
        }
    }
    к.restore().ok();
}

/* ---------- снимок ---------- */

fn временный() -> PathBuf {
    std::env::temp_dir().join(format!("glower-снимок-{}-{}.png", std::process::id(), glib::monotonic_time()))
}

/// Весь экран — файлом PNG
fn сними_экран() -> Result<Pixbuf, String> {
    let файл = временный();
    let итог = std::process::Command::new("grim").arg(&файл).output();
    match итог {
        Err(е) if е.kind() == std::io::ErrorKind::NotFound => Err("нет grim".into()),
        Err(е) => Err(е.to_string()),
        Ok(о) if !о.status.success() => Err(String::from_utf8_lossy(&о.stderr).trim().to_string()),
        Ok(_) => {
            let к = Pixbuf::from_file(&файл).map_err(|е| е.to_string());
            let _ = std::fs::remove_file(&файл);
            к
        }
    }
}

/// В буфер обмена. wl-copy держит его и после нашего выхода — как в Windows,
/// где снимок остаётся в буфере, хоть окно и закрыли
fn в_буфер(кадр: &Pixbuf) {
    let файл = временный();
    if кадр.savev(&файл, "png", &[]).is_ok() {
        if let Ok(данные) = std::fs::read(&файл) {
            let пошло = std::process::Command::new("wl-copy")
                .args(["--type", "image/png"])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut д| {
                    д.stdin.take().map(|mut в| в.write_all(&данные)).transpose()?;
                    Ok(())
                });
            let _ = std::fs::remove_file(&файл);
            if пошло.is_ok() {
                return;
            }
        }
    }
    gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD).set_image(кадр);
}

/// Рамка на экране — в пикселях кадра: экран и кадр могут быть разного
/// размера (масштаб). Совсем маленькая рамка — это щелчок, а не выбор
pub fn рамка_в_кадре(р: (f64, f64, f64, f64), экран: (f64, f64), кадр: (i32, i32)) -> Option<(i32, i32, i32, i32)> {
    let (x1, y1, x2, y2) = р;
    let (кx, кy) = (кадр.0 as f64 / экран.0, кадр.1 as f64 / экран.1);
    let л = ((x1.min(x2) * кx).round() as i32).clamp(0, кадр.0 - 1);
    let в = ((y1.min(y2) * кy).round() as i32).clamp(0, кадр.1 - 1);
    let ш = (((x2 - x1).abs() * кx).round() as i32).min(кадр.0 - л);
    let д = (((y2 - y1).abs() * кy).round() as i32).min(кадр.1 - в);
    (ш >= 4 && д >= 4).then_some((л, в, ш, д))
}

/// Выделить область поверх застывшего кадра. Esc — передумали
fn выдели(кадр: Pixbuf, готово: impl FnOnce(Option<Pixbuf>) + 'static) {
    выдели_с_рамкой(кадр, false, move |итог| готово(итог.map(|(к, _)| к)));
}

/// Рамка выделения в точках экрана: x, y, ширина, высота
type Рамка = (i32, i32, i32, i32);

/// То же, но отдаёт ещё и рамку на экране. «весь_по_щелчку» — щелчок без
/// рамки выбирает весь экран (так начинают запись всего экрана)
fn выдели_с_рамкой(кадр: Pixbuf, весь_по_щелчку: bool, готово: impl FnOnce(Option<(Pixbuf, Рамка)>) + 'static) {
    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.init_layer_shell();
    окно.set_layer(gtk_layer_shell::Layer::Overlay);
    for край in [gtk_layer_shell::Edge::Top, gtk_layer_shell::Edge::Bottom, gtk_layer_shell::Edge::Left, gtk_layer_shell::Edge::Right] {
        окно.set_anchor(край, true);
    }
    окно.set_exclusive_zone(-1);
    окно.set_keyboard_mode(gtk_layer_shell::KeyboardMode::Exclusive);
    окно.set_namespace("glower-snip");
    let холст = gtk::DrawingArea::new();
    холст.add_events(gtk::gdk::EventMask::BUTTON_PRESS_MASK | gtk::gdk::EventMask::BUTTON_RELEASE_MASK | gtk::gdk::EventMask::POINTER_MOTION_MASK);
    окно.add(&холст);
    let рамка: Rc<Cell<Option<(f64, f64, f64, f64)>>> = Rc::new(Cell::new(None));
    let готово: Rc<RefCell<Option<Box<dyn FnOnce(Option<(Pixbuf, Рамка)>)>>>> = Rc::new(RefCell::new(Some(Box::new(готово))));
    {
        let (кадр_р, рамка_р) = (кадр.clone(), рамка.clone());
        холст.connect_draw(move |х, к| {
            let (ш, в) = (х.allocated_width() as f64, х.allocated_height() as f64);
            к.save().ok();
            к.scale(ш / кадр_р.width() as f64, в / кадр_р.height() as f64);
            к.set_source_pixbuf(&кадр_р, 0.0, 0.0);
            let _ = к.paint();
            к.restore().ok();
            // Всё, кроме выделенного, — притушено
            к.set_source_rgba(0.0, 0.0, 0.0, 0.45);
            к.rectangle(0.0, 0.0, ш, в);
            if let Some((x1, y1, x2, y2)) = рамка_р.get() {
                к.rectangle(x2.min(x1), y1.min(y2), (x2 - x1).abs(), (y2 - y1).abs());
                к.set_fill_rule(gtk::cairo::FillRule::EvenOdd);
            }
            let _ = к.fill();
            if let Some((x1, y1, x2, y2)) = рамка_р.get() {
                к.set_source_rgb(1.0, 1.0, 1.0);
                к.set_line_width(1.5);
                к.rectangle(x2.min(x1) + 0.5, y1.min(y2) + 0.5, (x2 - x1).abs(), (y2 - y1).abs());
                let _ = к.stroke();
            }
            glib::Propagation::Stop
        });
        let рамка2 = рамка.clone();
        холст.connect_button_press_event(move |_, е| {
            let (x, y) = е.position();
            рамка2.set(Some((x, y, x, y)));
            glib::Propagation::Stop
        });
        let рамка2 = рамка.clone();
        холст.connect_motion_notify_event(move |х, е| {
            if е.state().contains(gtk::gdk::ModifierType::BUTTON1_MASK) {
                if let Some((x1, y1, _, _)) = рамка2.get() {
                    let (x, y) = е.position();
                    рамка2.set(Some((x1, y1, x, y)));
                    х.queue_draw();
                }
            }
            glib::Propagation::Proceed
        });
        let (рамка2, окно2, готово2, кадр2) = (рамка.clone(), окно.clone(), готово.clone(), кадр.clone());
        холст.connect_button_release_event(move |х, _| {
            let Some((x1, y1, x2, y2)) = рамка2.get() else { return glib::Propagation::Stop };
            let (эш, эв) = (х.allocated_width() as f64, х.allocated_height() as f64);
            let (x1, y1, x2, y2) = match рамка_в_кадре((x1, y1, x2, y2), (эш, эв), (кадр2.width(), кадр2.height())) {
                Some(_) => (x1, y1, x2, y2),
                None if весь_по_щелчку => (0.0, 0.0, эш, эв),
                None => {
                    рамка2.set(None);
                    х.queue_draw();
                    return glib::Propagation::Stop;
                }
            };
            let Some((л, в, ш, д)) = рамка_в_кадре((x1, y1, x2, y2), (эш, эв), (кадр2.width(), кадр2.height())) else { return glib::Propagation::Stop };
            let кусок = кадр2.new_subpixbuf(л, в, ш, д).copy();
            // На экране — в точках экрана: их и ждёт wf-recorder
            let на_экране = (x1.min(x2).round() as i32, y1.min(y2).round() as i32, (x2 - x1).abs().round() as i32, (y2 - y1).abs().round() as i32);
            unsafe { окно2.destroy() };
            if let Some(г) = готово2.borrow_mut().take() {
                г(кусок.map(|к| (к, на_экране)));
            }
            glib::Propagation::Stop
        });
        let (окно2, готово2) = (окно.clone(), готово.clone());
        окно.connect_key_press_event(move |_, е| {
            if е.keyval() == gtk::gdk::keys::constants::Escape {
                unsafe { окно2.destroy() };
                if let Some(г) = готово2.borrow_mut().take() {
                    г(None);
                }
            }
            glib::Propagation::Stop
        });
    }
    окно.show_all();
    if let Some(о) = окно.window() {
        if let Some(курсор) = gtk::gdk::Cursor::from_name(&окно.display(), "crosshair") {
            о.set_cursor(Some(&курсор));
        }
    }
}

/* ---------- запись экрана ---------- */

/// Где лежит номер процесса идущей записи: по нему второе Win+Shift+R
/// находит её и останавливает
fn метка_записи() -> PathBuf {
    glib::user_runtime_dir().join("glower-record.pid")
}

fn папка_записей() -> PathBuf {
    let корень = std::env::var("GLOWER_ROOT").map(PathBuf::from).unwrap_or_else(|_| glib::home_dir().join("GlowerOS"));
    let видео = [корень.join("Видео"), glib::home_dir().join("Видео"), glib::home_dir().join("Videos")]
        .into_iter()
        .find(|п| п.is_dir())
        .unwrap_or_else(|| корень.join("Видео"));
    видео.join("Записи экрана")
}

/// Идёт запись — остановить. false — записи не было
fn останови_запись() -> bool {
    let Some(pid) = std::fs::read_to_string(метка_записи()).ok().and_then(|т| т.trim().parse::<i32>().ok()) else { return false };
    // Процесс мог уже умереть, а метка — остаться
    if !PathBuf::from(format!("/proc/{pid}")).exists() {
        let _ = std::fs::remove_file(метка_записи());
        return false;
    }
    // SIGINT — wf-recorder дописывает файл и выходит, видео не битое
    let _ = std::process::Command::new("kill").args(["-INT", &pid.to_string()]).status();
    true
}

/// Выбрать область (щелчок — весь экран) и записывать
fn записывай(н: &Н) {
    if останови_запись() {
        return;
    }
    let видно = н.окно.is_visible();
    н.окно.hide();
    let н1 = н.clone();
    glib::timeout_add_local_once(Duration::from_millis(300), move || match сними_экран() {
        Err(е) => {
            if видно {
                н1.окно.show_all();
            }
            нечем_снимать(if видно { Some(&н1.окно) } else { None }, &е);
            if !видно {
                gtk::main_quit();
            }
        }
        Ok(кадр) => {
            let н2 = н1.clone();
            выдели_с_рамкой(кадр, true, move |итог| match итог {
                Some((_, рамка)) => начни_запись(&н2, рамка, видно),
                None if видно => н2.окно.show_all(),
                None => gtk::main_quit(),
            });
        }
    });
}

fn начни_запись(н: &Н, (x, y, ш, в): Рамка, окно_было: bool) {
    let папка = папка_записей();
    let _ = std::fs::create_dir_all(&папка);
    let имя = glib::DateTime::now_local().ok().and_then(|д| д.format("Запись %Y-%m-%d %H-%M-%S.mp4").ok()).map(|с| с.to_string()).unwrap_or_else(|| "Запись.mp4".into());
    let файл = папка.join(имя);
    // Кодеку нужны чётные стороны
    let (ш, в) = (ш - ш % 2, в - в % 2);
    let пошёл = std::process::Command::new("wf-recorder")
        .args(["-y", "-g", &format!("{x},{y} {ш}x{в}"), "-f"])
        .arg(&файл)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn();
    let mut ребёнок = match пошёл {
        Ok(р) => р,
        Err(_) => {
            let д = gtk::MessageDialog::new(None::<&gtk::Window>, gtk::DialogFlags::MODAL, gtk::MessageType::Info, gtk::ButtonsType::Close, "Записывать нечем");
            д.set_secondary_text(Some("Видео с экрана пишет программа «wf-recorder», а её нет на машине. Поставьте её в Магазине или командой sudo apt install wf-recorder."));
            д.run();
            unsafe { д.destroy() };
            if окно_было { н.окно.show_all() } else { gtk::main_quit() }
            return;
        }
    };
    let _ = std::fs::write(метка_записи(), ребёнок.id().to_string());
    let плашка = плашка_записи();
    let (куда, откуда) = async_channel::bounded::<(bool, String)>(1);
    std::thread::spawn(move || {
        let mut ошибка = String::new();
        if let Some(mut е) = ребёнок.stderr.take() {
            use std::io::Read;
            let _ = е.read_to_string(&mut ошибка);
        }
        let итог = ребёнок.wait().map(|с| с.success() || с.code().is_none()).unwrap_or(false);
        let _ = куда.send_blocking((итог, ошибка));
    });
    let н1 = н.clone();
    glib::MainContext::default().spawn_local(async move {
        let (вышло, ошибка) = откуда.recv().await.unwrap_or((false, String::new()));
        let _ = std::fs::remove_file(метка_записи());
        unsafe { плашка.destroy() };
        let есть = std::fs::metadata(&файл).is_ok_and(|м| м.len() > 0);
        if есть {
            crate::agent::скажи("уведомление", serde_json::json!({ "заголовок": "Запись сохранена", "текст": файл.file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_default(), "значок": "🎬", "откуда": "Ножницы" }));
            // Показать в Проводнике — как Windows открывает готовую запись
            let _ = std::process::Command::new("glower-files").arg(&файл).spawn();
        } else if !вышло {
            let хвост: String = ошибка.lines().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
            let д = gtk::MessageDialog::new(None::<&gtk::Window>, gtk::DialogFlags::MODAL, gtk::MessageType::Warning, gtk::ButtonsType::Close, "Запись не получилась");
            д.set_secondary_text(Some(&хвост));
            д.run();
            unsafe { д.destroy() };
        }
        if окно_было {
            н1.окно.show_all();
        } else {
            gtk::main_quit();
        }
    });
}

/// Плашка сверху посередине: красная точка, время и «Стоп»
fn плашка_записи() -> gtk::Window {
    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.init_layer_shell();
    окно.set_layer(gtk_layer_shell::Layer::Overlay);
    окно.set_anchor(gtk_layer_shell::Edge::Top, true);
    окно.set_layer_shell_margin(gtk_layer_shell::Edge::Top, 40);
    окно.set_keyboard_mode(gtk_layer_shell::KeyboardMode::None);
    окно.set_namespace("glower-record");
    окно.style_context().add_class("glower-app");
    окно.style_context().add_class("record-bar");
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    ряд.set_margin(8);
    let точка = gtk::Label::new(Some("●"));
    точка.style_context().add_class("dot");
    let время = gtk::Label::new(Some("0:00"));
    время.style_context().add_class("no-i18n");
    let стоп = кнопка("⏹ Стоп", "Остановить запись (Win+Shift+R)");
    стоп.connect_clicked(|б| {
        б.set_sensitive(false);
        останови_запись();
    });
    ряд.add(&точка);
    ряд.add(&время);
    ряд.add(&стоп);
    окно.add(&ряд);
    окно.show_all();
    let начало = std::time::Instant::now();
    let время = время.downgrade();
    glib::timeout_add_local(Duration::from_millis(500), move || {
        let Some(в) = время.upgrade() else { return glib::ControlFlow::Break };
        let с = начало.elapsed().as_secs();
        в.set_text(&format!("{}:{:02}", с / 60, с % 60));
        glib::ControlFlow::Continue
    });
    окно
}

/* ---------- окно ---------- */

struct Ножницы {
    окно: gtk::Window,
    холст: gtk::DrawingArea,
    заметка: gtk::Label,
    кадр: RefCell<Option<Pixbuf>>,
    штрихи: RefCell<Vec<Штрих>>,
    отменённые: RefCell<Vec<Штрих>>,
    текущий: RefCell<Option<Штрих>>,
    инструмент: Cell<Инструмент>,
    цвет: Cell<(f64, f64, f64)>,
    толщина: Cell<f64>,
    имя: RefCell<String>,
}

type Н = Rc<Ножницы>;

/// Где на холсте картинка: сдвиг и масштаб «вписать»
fn положение(н: &Н) -> Option<(f64, f64, f64)> {
    let к = н.кадр.borrow().clone()?;
    let (ш, в) = (н.холст.allocated_width() as f64 - 24.0, н.холст.allocated_height() as f64 - 24.0);
    let м = (ш / к.width() as f64).min(в / к.height() as f64).min(1.0);
    Some(((н.холст.allocated_width() as f64 - к.width() as f64 * м) / 2.0, (н.холст.allocated_height() as f64 - к.height() as f64 * м) / 2.0, м))
}

fn в_кадре(н: &Н, x: f64, y: f64) -> Option<(f64, f64)> {
    let (сx, сy, м) = положение(н)?;
    Some(((x - сx) / м, (y - сy) / м))
}

/// Готовая картинка: кадр и все штрихи поверх
fn собери(н: &Н) -> Option<Pixbuf> {
    let кадр = н.кадр.borrow().clone()?;
    let поверхность = gtk::cairo::ImageSurface::create(gtk::cairo::Format::ARgb32, кадр.width(), кадр.height()).ok()?;
    {
        let к = gtk::cairo::Context::new(&поверхность).ok()?;
        к.set_source_pixbuf(&кадр, 0.0, 0.0);
        let _ = к.paint();
        for ш in н.штрихи.borrow().iter() {
            нарисуй_штрих(&к, ш);
        }
    }
    gtk::gdk::pixbuf_get_from_surface(&поверхность, 0, 0, кадр.width(), кадр.height())
}

fn новый_кадр(н: &Н, кадр: Pixbuf) {
    в_буфер(&кадр);
    *н.кадр.borrow_mut() = Some(кадр);
    н.штрихи.borrow_mut().clear();
    н.отменённые.borrow_mut().clear();
    *н.имя.borrow_mut() = format!(
        "Снимок {}",
        glib::DateTime::now_local().and_then(|д| д.format("%Y-%m-%d %H-%M-%S")).map(|т| т.to_string()).unwrap_or_default()
    );
    н.заметка.set_text("В буфере обмена");
    н.холст.queue_draw();
    н.окно.show_all();
    н.окно.present();
}

fn папка_снимков() -> PathBuf {
    let корень = std::env::var("GLOWER_ROOT").map(PathBuf::from).unwrap_or_else(|_| glib::home_dir().join("GlowerOS"));
    let изображения = [корень.join("Изображения"), glib::home_dir().join("Изображения"), glib::home_dir().join("Pictures")]
        .into_iter()
        .find(|п| п.is_dir())
        .unwrap_or_else(|| корень.join("Изображения"));
    изображения.join("Снимки")
}

fn сохрани(н: &Н, спросить: bool) {
    let Some(готово) = собери(н) else { return };
    let mut путь = папка_снимков().join(format!("{}.png", н.имя.borrow()));
    if спросить {
        let д = gtk::FileChooserDialog::with_buttons(
            Some("Сохранить снимок"),
            Some(&н.окно),
            gtk::FileChooserAction::Save,
            &[("Отмена", gtk::ResponseType::Cancel), ("Сохранить", gtk::ResponseType::Accept)],
        );
        д.set_do_overwrite_confirmation(true);
        let _ = std::fs::create_dir_all(папка_снимков());
        д.set_current_folder(папка_снимков());
        д.set_current_name(&format!("{}.png", н.имя.borrow()));
        let ответ = д.run();
        let выбрано = д.filename();
        unsafe { д.destroy() };
        match (ответ, выбрано) {
            (gtk::ResponseType::Accept, Some(п)) => путь = п,
            _ => return,
        }
    }
    if let Some(р) = путь.parent() {
        let _ = std::fs::create_dir_all(р);
    }
    match готово.savev(&путь, "png", &[]) {
        Ok(()) => н.заметка.set_text(&format!("Сохранено: {}", путь.display())),
        Err(е) => {
            let д = gtk::MessageDialog::new(Some(&н.окно), gtk::DialogFlags::MODAL, gtk::MessageType::Warning, gtk::ButtonsType::Close, "Не вышло сохранить");
            д.set_secondary_text(Some(&е.to_string()));
            д.run();
            unsafe { д.destroy() };
        }
    }
}

/// grim нет — предложить поставить: это небольшой пакет
fn нечем_снимать(окно: Option<&gtk::Window>, е: &str) {
    if е == "нет grim" {
        let д = gtk::MessageDialog::new(окно, gtk::DialogFlags::MODAL, gtk::MessageType::Question, gtk::ButtonsType::None, "Для снимков не хватает программы");
        д.set_secondary_text(Some("Экран снимает системная программа «grim», а её нет на машине. Поставить её сейчас? Это небольшой пакет."));
        д.add_button("Не сейчас", gtk::ResponseType::Cancel);
        д.add_button("Поставить", gtk::ResponseType::Accept);
        let ответ = д.run();
        unsafe { д.destroy() };
        if ответ == gtk::ResponseType::Accept {
            crate::agent::позови("pkg.install", serde_json::json!({ "name": "grim", "source": "apt" }), |_| {});
        }
        return;
    }
    let д = gtk::MessageDialog::new(окно, gtk::DialogFlags::MODAL, gtk::MessageType::Warning, gtk::ButtonsType::Close, "Не вышло снять экран");
    д.set_secondary_text(Some(е));
    д.run();
    unsafe { д.destroy() };
}

/// Снять: окно прячем, ждём, пока оконный сервер его уберёт, снимаем
fn снимай(н: &Н, область: bool, задержка: u32) {
    let видно = н.окно.is_visible();
    н.окно.hide();
    let н1 = н.clone();
    glib::timeout_add_local_once(Duration::from_millis(300 + задержка as u64 * 1000), move || match сними_экран() {
        Err(е) => {
            if видно {
                н1.окно.show_all();
            }
            нечем_снимать(if видно { Some(&н1.окно) } else { None }, &е);
            if !видно {
                gtk::main_quit();
            }
        }
        Ok(кадр) if область => {
            let н2 = н1.clone();
            выдели(кадр, move |кусок| match кусок {
                Some(к) => {
                    н2.окно.show_all();
                    новый_кадр(&н2, к);
                }
                None if видно => н2.окно.show_all(),
                // Передумали, а окна и не было — просто уходим
                None => gtk::main_quit(),
            });
        }
        Ok(кадр) => {
            н1.окно.show_all();
            новый_кадр(&н1, кадр);
        }
    });
}

pub fn запусти(доводы: Vec<String>) -> ! {
    подготовь("glower-snip", "Ножницы");
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль ножниц");
    gtk::StyleContext::add_provider_for_screen(&gtk::gdk::Screen::default().expect("экран"), &стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_default_size(900, 640);
    окно.set_title("Ножницы");
    окно.set_icon_name(Some("applets-screenshooter"));
    окно.style_context().add_class("glower-app");
    crate::apps::строка_заголовка(&окно);
    окно.style_context().add_class("snip");
    let колонка = gtk::Box::new(gtk::Orientation::Vertical, 0);
    окно.add(&колонка);

    let верх = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    верх.style_context().add_class("toolbar");
    let к_область = кнопка("✂️ Область", "Выделить прямоугольник на экране (Win+Shift+S)");
    let к_экран = кнопка("🖥 Весь экран", "Снять экран целиком (PrintScreen)");
    let задержка = gtk::ComboBoxText::new();
    for (id, имя) in [("0", "Без задержки"), ("3", "3 секунды"), ("5", "5 секунд"), ("10", "10 секунд")] {
        задержка.append(Some(id), &crate::i18n::tr(имя));
    }
    задержка.set_active_id(Some("0"));
    let к_запись = кнопка("⏺ Запись", "Записать видео с экрана (Win+Shift+R)");
    верх.add(&к_область);
    верх.add(&к_экран);
    верх.add(&к_запись);
    верх.add(&задержка);
    let пустота = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    пустота.set_hexpand(true);
    верх.add(&пустота);
    let заметка = gtk::Label::new(None);
    заметка.style_context().add_class("note");
    верх.add(&заметка);
    колонка.add(&верх);

    let холст = gtk::DrawingArea::new();
    холст.style_context().add_class("stage");
    холст.set_vexpand(true);
    холст.add_events(gtk::gdk::EventMask::BUTTON_PRESS_MASK | gtk::gdk::EventMask::BUTTON_RELEASE_MASK | gtk::gdk::EventMask::POINTER_MOTION_MASK);
    колонка.add(&холст);

    let низ = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    низ.style_context().add_class("toolbar");
    колонка.add(&низ);

    let н: Н = Rc::new(Ножницы {
        окно: окно.clone(),
        холст: холст.clone(),
        заметка,
        кадр: RefCell::new(None),
        штрихи: RefCell::new(Vec::new()),
        отменённые: RefCell::new(Vec::new()),
        текущий: RefCell::new(None),
        инструмент: Cell::new(Инструмент::Перо),
        цвет: Cell::new((0.94, 0.27, 0.27)),
        толщина: Cell::new(4.0),
        имя: RefCell::new("Снимок".into()),
    });

    // Инструменты
    let инструменты: Rc<RefCell<Vec<(gtk::Button, Инструмент)>>> = Rc::new(RefCell::new(Vec::new()));
    for (знак, подпись, и) in [
        ("document-edit-symbolic", "Перо", Инструмент::Перо),
        ("applications-graphics-symbolic", "Маркер", Инструмент::Маркер),
        ("стрелка", "Стрелка", Инструмент::Стрелка),
        ("прямоугольник", "Рамка", Инструмент::Рамка),
        ("замазать", "Замазать личное", Инструмент::Замазать),
        ("insert-text-symbolic", "Надпись", Инструмент::Текст),
    ] {
        // Значки — те же, что у Paint: из темы или рисованные
        let б = gtk::Button::new();
        б.add(&crate::paint::значок_инструмента(знак));
        б.set_tooltip_text(Some(подпись));
        б.style_context().add_class("image-button");
        if и == Инструмент::Перо {
            б.style_context().add_class("on");
        }
        let (н1, все) = (н.clone(), инструменты.clone());
        б.connect_clicked(move |б| {
            for (х, _) in все.borrow().iter() {
                х.style_context().remove_class("on");
            }
            б.style_context().add_class("on");
            н1.инструмент.set(и);
        });
        инструменты.borrow_mut().push((б.clone(), и));
        низ.add(&б);
    }
    низ.add(&gtk::Separator::new(gtk::Orientation::Vertical));
    let кружки: Rc<RefCell<Vec<gtk::Button>>> = Rc::new(RefCell::new(Vec::new()));
    for (i, (r, g, b)) in [(0.94, 0.27, 0.27), (0.96, 0.62, 0.04), (0.13, 0.77, 0.37), (0.23, 0.51, 0.96), (0.66, 0.33, 0.97), (0.07, 0.09, 0.15), (1.0, 1.0, 1.0)].into_iter().enumerate() {
        let б = gtk::Button::new();
        б.style_context().add_class("color");
        if i == 0 {
            б.style_context().add_class("on");
        }
        let стиль = gtk::CssProvider::new();
        let _ = стиль.load_from_data(format!("button {{ background-image: none; background-color: rgb({},{},{}); }}", (r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8).as_bytes());
        б.style_context().add_provider(&стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2);
        let (н1, все) = (н.clone(), кружки.clone());
        б.connect_clicked(move |б| {
            for х in все.borrow().iter() {
                х.style_context().remove_class("on");
            }
            б.style_context().add_class("on");
            н1.цвет.set((r, g, b));
        });
        кружки.borrow_mut().push(б.clone());
        низ.add(&б);
    }
    let толщина = gtk::Scale::with_range(gtk::Orientation::Horizontal, 2.0, 16.0, 1.0);
    толщина.set_value(4.0);
    толщина.set_draw_value(false);
    толщина.set_size_request(80, -1);
    толщина.set_tooltip_text(Some("Толщина линии"));
    {
        let н1 = н.clone();
        толщина.connect_value_changed(move |т| н1.толщина.set(т.value()));
    }
    низ.add(&толщина);
    let к_отмена = кнопка("↶", "Отменить (Ctrl+Z)");
    let к_вернуть = кнопка("↷", "Вернуть (Ctrl+Shift+Z)");
    низ.add(&к_отмена);
    низ.add(&к_вернуть);
    let пустота2 = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    пустота2.set_hexpand(true);
    низ.add(&пустота2);
    let к_копия = кнопка("📋", "Копировать в буфер обмена (Ctrl+C)");
    let к_как = кнопка("…", "Сохранить как… (Ctrl+Shift+S)");
    let к_сохранить = кнопка("💾 Сохранить", "В Изображения → Снимки (Ctrl+S)");
    к_сохранить.style_context().add_class("accent");
    низ.add(&к_копия);
    низ.add(&к_как);
    низ.add(&к_сохранить);

    let отмени = {
        let н1 = н.clone();
        move || {
            let ш = н1.штрихи.borrow_mut().pop();
            if let Some(ш) = ш {
                н1.отменённые.borrow_mut().push(ш);
                н1.холст.queue_draw();
            }
        }
    };
    let верни = {
        let н1 = н.clone();
        move || {
            let ш = н1.отменённые.borrow_mut().pop();
            if let Some(ш) = ш {
                н1.штрихи.borrow_mut().push(ш);
                н1.холст.queue_draw();
            }
        }
    };
    let скопируй = {
        let н1 = н.clone();
        move || {
            if let Some(к) = собери(&н1) {
                в_буфер(&к);
                н1.заметка.set_text("В буфере обмена");
            }
        }
    };
    {
        let о = отмени.clone();
        к_отмена.connect_clicked(move |_| о());
        let в = верни.clone();
        к_вернуть.connect_clicked(move |_| в());
        let с = скопируй.clone();
        к_копия.connect_clicked(move |_| с());
        let н1 = н.clone();
        к_сохранить.connect_clicked(move |_| сохрани(&н1, false));
        let н1 = н.clone();
        к_как.connect_clicked(move |_| сохрани(&н1, true));
        let (н1, з) = (н.clone(), задержка.clone());
        к_область.connect_clicked(move |_| снимай(&н1, true, з.active_id().and_then(|и| и.parse().ok()).unwrap_or(0)));
        let (н1, з) = (н.clone(), задержка.clone());
        к_экран.connect_clicked(move |_| снимай(&н1, false, з.active_id().and_then(|и| и.parse().ok()).unwrap_or(0)));
    }

    // Холст: кадр по центру, штрихи поверх
    {
        let н1 = н.clone();
        холст.connect_draw(move |х, к| {
            к.set_source_rgb(0.043, 0.047, 0.063);
            let _ = к.paint();
            let Some((сx, сy, м)) = положение(&н1) else {
                к.set_source_rgba(1.0, 1.0, 1.0, 0.55);
                к.set_font_size(15.0);
                let текст = "PrintScreen — весь экран, Win+Shift+S — область";
                if let Ok(р) = к.text_extents(текст) {
                    к.move_to((х.allocated_width() as f64 - р.width()) / 2.0, х.allocated_height() as f64 / 2.0);
                    let _ = к.show_text(текст);
                }
                return glib::Propagation::Stop;
            };
            к.translate(сx, сy);
            к.scale(м, м);
            if let Some(кадр) = н1.кадр.borrow().as_ref() {
                к.set_source_pixbuf(кадр, 0.0, 0.0);
                let _ = к.paint();
            }
            for ш in н1.штрихи.borrow().iter() {
                нарисуй_штрих(к, ш);
            }
            if let Some(ш) = н1.текущий.borrow().as_ref() {
                нарисуй_штрих(к, ш);
            }
            glib::Propagation::Stop
        });
        let н1 = н.clone();
        холст.connect_button_press_event(move |_, е| {
            if е.button() != 1 {
                return glib::Propagation::Proceed;
            }
            let (x, y) = е.position();
            let Some(т) = в_кадре(&н1, x, y) else { return glib::Propagation::Stop };
            let mut штрих = Штрих { вид: н1.инструмент.get(), цвет: н1.цвет.get(), толщина: н1.толщина.get(), точки: vec![т], текст: String::new() };
            if штрих.вид == Инструмент::Текст {
                let д = gtk::Dialog::with_buttons(Some("Надпись"), Some(&н1.окно), gtk::DialogFlags::MODAL, &[("Отмена", gtk::ResponseType::Cancel), ("Готово", gtk::ResponseType::Accept)]);
                д.set_default_response(gtk::ResponseType::Accept);
                let поле = gtk::Entry::new();
                поле.set_activates_default(true);
                поле.set_margin(14);
                поле.set_width_chars(30);
                д.content_area().add(&поле);
                д.show_all();
                let ответ = д.run();
                штрих.текст = поле.text().to_string();
                unsafe { д.destroy() };
                if ответ == gtk::ResponseType::Accept && !штрих.текст.trim().is_empty() {
                    н1.штрихи.borrow_mut().push(штрих);
                    н1.отменённые.borrow_mut().clear();
                    н1.холст.queue_draw();
                }
                return glib::Propagation::Stop;
            }
            *н1.текущий.borrow_mut() = Some(штрих);
            glib::Propagation::Stop
        });
        let н1 = н.clone();
        холст.connect_motion_notify_event(move |_, е| {
            let (x, y) = е.position();
            let Some(т) = в_кадре(&н1, x, y) else { return glib::Propagation::Proceed };
            let mut идёт = false;
            if let Some(ш) = н1.текущий.borrow_mut().as_mut() {
                if matches!(ш.вид, Инструмент::Перо | Инструмент::Маркер) {
                    ш.точки.push(т);
                } else {
                    ш.точки.truncate(1);
                    ш.точки.push(т);
                }
                идёт = true;
            }
            if идёт {
                н1.холст.queue_draw();
            }
            glib::Propagation::Proceed
        });
        let н1 = н.clone();
        холст.connect_button_release_event(move |_, _| {
            if let Some(ш) = н1.текущий.borrow_mut().take() {
                if !ш.пустой() {
                    н1.штрихи.borrow_mut().push(ш);
                    н1.отменённые.borrow_mut().clear();
                }
                н1.холст.queue_draw();
            }
            glib::Propagation::Stop
        });
    }

    {
        let н1 = н.clone();
        окно.connect_key_press_event(move |_, е| {
            if !е.state().contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                return glib::Propagation::Proceed;
            }
            let shift = е.state().contains(gtk::gdk::ModifierType::SHIFT_MASK);
            match (crate::notepad::латинская(е).as_str(), shift) {
                ("z", false) => отмени(),
                ("z", true) | ("y", _) => верни(),
                ("c", _) => скопируй(),
                ("s", false) => сохрани(&н1, false),
                ("s", true) => сохрани(&н1, true),
                ("n", _) => снимай(&н1, true, 0),
                _ => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        });
    }
    окно.connect_delete_event(|_, _| {
        gtk::main_quit();
        glib::Propagation::Proceed
    });
    {
        let н1 = н.clone();
        к_запись.connect_clicked(move |_| записывай(&н1));
    }

    // С доводом — сперва снимок, окно потом: в кадр оно не попадёт
    match доводы.first().map(String::as_str) {
        Some("экран") | Some("screen") => снимай(&н, false, 0),
        Some("область") | Some("area") => снимай(&н, true, 0),
        Some("запись") | Some("record") => {
            // Запись уже идёт — это просьба её остановить
            if останови_запись() {
                std::process::exit(0);
            }
            записывай(&н)
        }
        _ => окно.show_all(),
    }
    gtk::main();
    std::process::exit(0);
}

#[cfg(test)]
mod проверки {
    use super::*;

    #[test]
    fn рамка_переводится_в_пиксели_кадра() {
        // Экран 1280×800 с масштабом 2: кадр вдвое крупнее
        assert_eq!(рамка_в_кадре((100.0, 50.0, 300.0, 250.0), (1280.0, 800.0), (2560, 1600)), Some((200, 100, 400, 400)));
        // Тянули справа налево и за край — всё равно внутри кадра
        assert_eq!(рамка_в_кадре((1300.0, 900.0, 1200.0, 700.0), (1280.0, 800.0), (1280, 800)), Some((1200, 700, 80, 100)));
        assert_eq!(рамка_в_кадре((10.0, 10.0, 12.0, 11.0), (1280.0, 800.0), (1280, 800)), None);
    }

    #[test]
    fn щелчок_рамкой_не_штрих() {
        let ш = Штрих { вид: Инструмент::Рамка, цвет: (1.0, 0.0, 0.0), толщина: 4.0, точки: vec![(10.0, 10.0), (11.0, 11.0)], текст: String::new() };
        assert!(ш.пустой());
        let п = Штрих { вид: Инструмент::Перо, точки: vec![(10.0, 10.0)], ..ш.clone() };
        assert!(!п.пустой());
    }

    #[test]
    fn рисует_штрихи_на_картинке() {
        let п = gtk::cairo::ImageSurface::create(gtk::cairo::Format::ARgb32, 40, 40).unwrap();
        let к = gtk::cairo::Context::new(&п).unwrap();
        for вид in [Инструмент::Перо, Инструмент::Маркер, Инструмент::Стрелка, Инструмент::Рамка, Инструмент::Замазать] {
            нарисуй_штрих(&к, &Штрих { вид, цвет: (1.0, 0.0, 0.0), толщина: 3.0, точки: vec![(5.0, 5.0), (30.0, 30.0)], текст: String::new() });
        }
        drop(к);
        let данные = п.take_data().unwrap();
        assert!(данные.iter().any(|б| *б != 0));
    }
}
