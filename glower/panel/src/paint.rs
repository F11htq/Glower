//! Paint — рисование, настоящим окном (glower-paint).
//!
//! Холст — картинка в памяти (cairo), инструменты те же, что были у
//! страничного: карандаш, кисть, ластик, линия, прямоугольник, эллипс,
//! заливка, баллончик, а ещё текст. Отмена на 30 шагов, открыть и
//! сохранить PNG, вставить картинку из буфера обмена.

use crate::apps::{кнопка, папка_изображений, подготовь};
use gtk::cairo;
use gtk::gdk_pixbuf::Pixbuf;
use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::f64::consts::PI;
use std::path::PathBuf;
use std::rc::Rc;

const СТИЛЬ: &str = r#"
.paint .tools { padding: 8px; border-right: 1px solid @glower_line; }
.paint .tools button { min-width: 38px; min-height: 38px; padding: 0; font-size: 18px; }
.paint .tools button.on { background-color: alpha(@glower_accent, 0.45); box-shadow: inset 0 0 0 1px @glower_accent; }
.paint .stage { background-color: @glower_bg2; }
.paint button.sw { min-width: 18px; min-height: 18px; padding: 0; border-radius: 11px; border: 1px solid alpha(@glower_fg, 0.25); }
.paint button.sw.on { box-shadow: 0 0 0 2px @glower_accent; }
"#;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Инструмент {
    Карандаш,
    Кисть,
    Ластик,
    Линия,
    Прямоугольник,
    Эллипс,
    Заливка,
    Баллончик,
    Текст,
}

const ИНСТРУМЕНТЫ: &[(Инструмент, &str, &str)] = &[
    (Инструмент::Карандаш, "✏️", "Карандаш"),
    (Инструмент::Кисть, "🖌️", "Кисть"),
    (Инструмент::Ластик, "🧽", "Ластик"),
    (Инструмент::Линия, "📏", "Линия"),
    (Инструмент::Прямоугольник, "▭", "Прямоугольник"),
    (Инструмент::Эллипс, "◯", "Эллипс"),
    (Инструмент::Заливка, "🪣", "Заливка"),
    (Инструмент::Баллончик, "💨", "Баллончик"),
    (Инструмент::Текст, "🔤", "Текст"),
];

const ПАЛИТРА: &[&str] = &["#000000", "#ffffff", "#ef4444", "#f97316", "#facc15", "#22c55e", "#06b6d4", "#3a86ff", "#8b5cf6", "#ec4899", "#78350f", "#64748b"];

struct Холст {
    картинка: RefCell<cairo::ImageSurface>,
    /// Снимок до начала фигуры: линию и рамку рисуем заново на каждом шаге
    до_фигуры: RefCell<Option<cairo::ImageSurface>>,
    отмена: RefCell<Vec<cairo::ImageSurface>>,
    повтор: RefCell<Vec<cairo::ImageSurface>>,
    инструмент: Cell<Инструмент>,
    цвет: Cell<(f64, f64, f64)>,
    толщина: Cell<f64>,
    начало: Cell<Option<(f64, f64)>>,
    было: Cell<(f64, f64)>,
    файл: RefCell<Option<PathBuf>>,
    изменён: Cell<bool>,
    жребий: Cell<u64>,
}

type Х = Rc<Холст>;

pub fn цвет_из_hex(hex: &str) -> (f64, f64, f64) {
    let h = hex.trim_start_matches('#');
    let к = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0) as f64 / 255.0;
    (к(0), к(2), к(4))
}

fn копия(п: &cairo::ImageSurface) -> cairo::ImageSurface {
    let н = cairo::ImageSurface::create(cairo::Format::ARgb32, п.width(), п.height()).expect("холст");
    {
        let к = cairo::Context::new(&н).expect("кисть");
        let _ = к.set_source_surface(п, 0.0, 0.0);
        let _ = к.paint();
    }
    н
}

fn белый(ш: i32, в: i32) -> cairo::ImageSurface {
    let п = cairo::ImageSurface::create(cairo::Format::ARgb32, ш, в).expect("холст");
    {
        let к = cairo::Context::new(&п).expect("кисть");
        к.set_source_rgb(1.0, 1.0, 1.0);
        let _ = к.paint();
    }
    п
}

fn запомни(х: &Х) {
    let к = копия(&х.картинка.borrow());
    let mut о = х.отмена.borrow_mut();
    о.push(к);
    if о.len() > 30 {
        о.remove(0);
    }
    х.повтор.borrow_mut().clear();
    х.изменён.set(true);
}

/// Заливка: всё связное того же цвета (с допуском) — новым цветом
pub fn залей(п: &mut cairo::ImageSurface, x: i32, y: i32, цвет: (f64, f64, f64)) {
    let (ш, в) = (п.width(), п.height());
    if x < 0 || y < 0 || x >= ш || y >= в {
        return;
    }
    let шаг = п.stride() as usize;
    п.flush();
    let Ok(mut данные) = п.data() else { return };
    let i = |x: i32, y: i32| y as usize * шаг + x as usize * 4;
    let н = [(цвет.2 * 255.0) as u8, (цвет.1 * 255.0) as u8, (цвет.0 * 255.0) as u8, 255u8];
    let н0 = i(x, y);
    let был = [данные[н0], данные[н0 + 1], данные[н0 + 2], данные[н0 + 3]];
    if был == н {
        return;
    }
    let похож = |д: &[u8]| (д[0] as i32 - был[0] as i32).abs() + (д[1] as i32 - был[1] as i32).abs() + (д[2] as i32 - был[2] as i32).abs() <= 30;
    let mut стопка = vec![(x, y)];
    let mut видел = vec![false; (ш * в) as usize];
    while let Some((cx, cy)) = стопка.pop() {
        if cx < 0 || cy < 0 || cx >= ш || cy >= в {
            continue;
        }
        let м = (cy * ш + cx) as usize;
        if видел[м] {
            continue;
        }
        видел[м] = true;
        let к = i(cx, cy);
        if !похож(&данные[к..к + 4]) {
            continue;
        }
        данные[к..к + 4].copy_from_slice(&н);
        стопка.extend([(cx + 1, cy), (cx - 1, cy), (cx, cy + 1), (cx, cy - 1)]);
    }
}

fn случай(х: &Х) -> f64 {
    let mut s = х.жребий.get();
    s ^= s << 13;
    s ^= s >> 7;
    s ^= s << 17;
    х.жребий.set(s);
    (s >> 11) as f64 / (1u64 << 53) as f64
}

fn кисть(х: &Х, к: &cairo::Context) {
    let (r, g, b) = х.цвет.get();
    match х.инструмент.get() {
        Инструмент::Ластик => к.set_source_rgb(1.0, 1.0, 1.0),
        Инструмент::Кисть => к.set_source_rgba(r, g, b, 0.35),
        _ => к.set_source_rgb(r, g, b),
    }
    к.set_line_width(if х.инструмент.get() == Инструмент::Кисть { х.толщина.get() * 2.0 } else { х.толщина.get() });
    к.set_line_cap(cairo::LineCap::Round);
    к.set_line_join(cairo::LineJoin::Round);
}

/// Фигура от начала до точки — на чистом снимке «до фигуры»
fn фигура(х: &Х, x: f64, y: f64) {
    let Some((sx, sy)) = х.начало.get() else { return };
    let Some(до) = х.до_фигуры.borrow().as_ref().map(копия) else { return };
    let к = cairo::Context::new(&до).expect("кисть");
    кисть(х, &к);
    match х.инструмент.get() {
        Инструмент::Линия => {
            к.move_to(sx, sy);
            к.line_to(x, y);
        }
        Инструмент::Прямоугольник => к.rectangle(sx.min(x), sy.min(y), (x - sx).abs(), (y - sy).abs()),
        _ => {
            let (rx, ry) = (((x - sx) / 2.0).abs().max(0.5), ((y - sy) / 2.0).abs().max(0.5));
            к.save().ok();
            к.translate((sx + x) / 2.0, (sy + y) / 2.0);
            к.scale(rx, ry);
            к.arc(0.0, 0.0, 1.0, 0.0, 2.0 * PI);
            к.restore().ok();
        }
    }
    let _ = к.stroke();
    drop(к);
    *х.картинка.borrow_mut() = до;
}

fn штрих(х: &Х, x: f64, y: f64) {
    let (px, py) = х.было.get();
    let п = х.картинка.borrow();
    let к = cairo::Context::new(&*п).expect("кисть");
    кисть(х, &к);
    if х.инструмент.get() == Инструмент::Баллончик {
        let r = х.толщина.get() * 1.6;
        for _ in 0..14 {
            let (а, д) = (случай(х) * 2.0 * PI, случай(х) * r);
            к.rectangle(x + а.cos() * д, y + а.sin() * д, 1.4, 1.4);
        }
        let _ = к.fill();
    } else {
        к.move_to(px, py);
        к.line_to(x, y);
        let _ = к.stroke();
    }
    х.было.set((x, y));
}

fn в_холст(холст: &gtk::DrawingArea, х: &Х, x: f64, y: f64) -> (f64, f64) {
    let п = х.картинка.borrow();
    let (ш, в) = (холст.allocated_width() as f64, холст.allocated_height() as f64);
    let (пш, пв) = (п.width() as f64, п.height() as f64);
    let (ox, oy) = (((ш - пш) / 2.0).max(0.0).floor(), ((в - пв) / 2.0).max(0.0).floor());
    (x - ox, y - oy)
}

fn сохрани(окно: &gtk::Window, х: &Х, как: bool) -> bool {
    let путь = match (как, х.файл.borrow().clone()) {
        (false, Some(п)) => п,
        _ => {
            let д = gtk::FileChooserNative::new(Some("Сохранить рисунок"), Some(окно), gtk::FileChooserAction::Save, Some("Сохранить"), Some("Отмена"));
            let _ = std::fs::create_dir_all(папка_изображений());
            д.set_current_folder(папка_изображений());
            д.set_current_name("Рисунок.png");
            д.set_do_overwrite_confirmation(true);
            if д.run() != gtk::ResponseType::Accept {
                return false;
            }
            let Some(п) = д.filename() else { return false };
            if п.extension().is_none() { п.with_extension("png") } else { п }
        }
    };
    let п = х.картинка.borrow();
    let Some(к) = gtk::gdk::pixbuf_get_from_surface(&п, 0, 0, п.width(), п.height()) else { return false };
    if к.savev(&путь, "png", &[]).is_err() {
        return false;
    }
    окно.set_title(&format!("{} — Paint", путь.file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_default()));
    *х.файл.borrow_mut() = Some(путь);
    х.изменён.set(false);
    true
}

fn из_картинки(к: &Pixbuf) -> cairo::ImageSurface {
    let п = белый(к.width(), к.height());
    {
        let с = cairo::Context::new(&п).expect("кисть");
        с.set_source_pixbuf(к, 0.0, 0.0);
        let _ = с.paint();
    }
    п
}

pub fn запусти(доводы: Vec<String>) -> ! {
    подготовь("glower-paint", "Paint");
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль Paint");
    gtk::StyleContext::add_provider_for_screen(&gtk::gdk::Screen::default().expect("экран"), &стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_title("Без имени — Paint");
    окно.set_default_size(1000, 700);
    окно.set_icon_name(Some("applications-graphics"));
    окно.style_context().add_class("glower-app");
    окно.style_context().add_class("paint");

    let открыть_файл = доводы.iter().find(|д| !д.starts_with("--")).map(PathBuf::from);
    let начальная = открыть_файл.as_ref().and_then(|п| Pixbuf::from_file(п).ok()).map(|к| из_картинки(&к)).unwrap_or_else(|| белый(960, 600));
    let х: Х = Rc::new(Холст {
        картинка: RefCell::new(начальная),
        до_фигуры: RefCell::new(None),
        отмена: RefCell::new(Vec::new()),
        повтор: RefCell::new(Vec::new()),
        инструмент: Cell::new(Инструмент::Карандаш),
        цвет: Cell::new(цвет_из_hex("#3a86ff")),
        толщина: Cell::new(5.0),
        начало: Cell::new(None),
        было: Cell::new((0.0, 0.0)),
        файл: RefCell::new(открыть_файл.clone().filter(|п| п.is_file())),
        изменён: Cell::new(false),
        жребий: Cell::new(glib::real_time() as u64 | 1),
    });
    if let Some(п) = х.файл.borrow().as_ref() {
        окно.set_title(&format!("{} — Paint", п.file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_default()));
    }

    let колонка = gtk::Box::new(gtk::Orientation::Vertical, 0);
    окно.add(&колонка);
    let верх = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    верх.style_context().add_class("toolbar");
    колонка.add(&верх);
    let середина = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    середина.set_vexpand(true);
    колонка.add(&середина);

    // Инструменты слева
    let инстр = gtk::Box::new(gtk::Orientation::Vertical, 6);
    инстр.style_context().add_class("tools");
    середина.add(&инстр);
    let кнопки_инстр: Rc<RefCell<Vec<(Инструмент, gtk::Button)>>> = Rc::new(RefCell::new(Vec::new()));
    for (и, знак, имя) in ИНСТРУМЕНТЫ {
        let б = кнопка(знак, имя);
        if *и == Инструмент::Карандаш {
            б.style_context().add_class("on");
        }
        let (х2, все, и) = (х.clone(), кнопки_инстр.clone(), *и);
        б.connect_clicked(move |б| {
            х2.инструмент.set(и);
            for (_, к) in все.borrow().iter() {
                к.style_context().remove_class("on");
            }
            б.style_context().add_class("on");
        });
        кнопки_инстр.borrow_mut().push((и, б.clone()));
        инстр.add(&б);
    }

    // Холст
    let холст = gtk::DrawingArea::new();
    холст.style_context().add_class("stage");
    холст.set_hexpand(true);
    холст.set_vexpand(true);
    холст.add_events(gtk::gdk::EventMask::BUTTON_PRESS_MASK | gtk::gdk::EventMask::BUTTON_RELEASE_MASK | gtk::gdk::EventMask::POINTER_MOTION_MASK);
    let листание = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    листание.add(&холст);
    листание.set_hexpand(true);
    середина.add(&листание);
    let по_размеру = {
        let (х, холст) = (х.clone(), холст.clone());
        move || {
            let п = х.картинка.borrow();
            холст.set_size_request(п.width(), п.height());
            холст.queue_draw();
        }
    };
    по_размеру();
    {
        let х = х.clone();
        холст.connect_draw(move |в, к| {
            let п = х.картинка.borrow();
            let (ш, вы) = (в.allocated_width() as f64, в.allocated_height() as f64);
            let (ox, oy) = (((ш - п.width() as f64) / 2.0).max(0.0).floor(), ((вы - п.height() as f64) / 2.0).max(0.0).floor());
            gtk::render_background(&в.style_context(), к, 0.0, 0.0, ш, вы);
            к.set_source_rgba(0.0, 0.0, 0.0, 0.25);
            к.rectangle(ox + 3.0, oy + 3.0, п.width() as f64, п.height() as f64);
            let _ = к.fill();
            let _ = к.set_source_surface(&*п, ox, oy);
            let _ = к.paint();
            glib::Propagation::Stop
        });
    }
    {
        let (х, окно2) = (х.clone(), окно.clone());
        холст.connect_button_press_event(move |в, е| {
            if е.button() != 1 {
                return glib::Propagation::Proceed;
            }
            let (x, y) = в_холст(в, &х, е.position().0, е.position().1);
            запомни(&х);
            match х.инструмент.get() {
                Инструмент::Заливка => {
                    let mut п = х.картинка.borrow_mut();
                    залей(&mut п, x as i32, y as i32, х.цвет.get());
                    п.mark_dirty();
                }
                Инструмент::Текст => {
                    if let Some(текст) = спроси_текст(&окно2) {
                        let п = х.картинка.borrow();
                        let к = cairo::Context::new(&*п).expect("кисть");
                        let (r, g, b) = х.цвет.get();
                        к.set_source_rgb(r, g, b);
                        к.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
                        к.set_font_size(12.0 + х.толщина.get() * 2.0);
                        к.move_to(x, y);
                        let _ = к.show_text(&текст);
                    }
                }
                Инструмент::Линия | Инструмент::Прямоугольник | Инструмент::Эллипс => {
                    х.начало.set(Some((x, y)));
                    *х.до_фигуры.borrow_mut() = Some(копия(&х.картинка.borrow()));
                }
                _ => {
                    х.начало.set(Some((x, y)));
                    х.было.set((x, y + 0.1));
                    штрих(&х, x, y);
                }
            }
            в.queue_draw();
            glib::Propagation::Stop
        });
    }
    {
        let х = х.clone();
        холст.connect_motion_notify_event(move |в, е| {
            if х.начало.get().is_none() {
                return glib::Propagation::Proceed;
            }
            let (x, y) = в_холст(в, &х, е.position().0, е.position().1);
            match х.инструмент.get() {
                Инструмент::Линия | Инструмент::Прямоугольник | Инструмент::Эллипс => фигура(&х, x, y),
                _ => штрих(&х, x, y),
            }
            в.queue_draw();
            glib::Propagation::Stop
        });
    }
    {
        let х = х.clone();
        холст.connect_button_release_event(move |_, _| {
            х.начало.set(None);
            *х.до_фигуры.borrow_mut() = None;
            glib::Propagation::Stop
        });
    }

    // Цвета, толщина и дела — сверху
    let образцы: Rc<RefCell<Vec<gtk::Button>>> = Rc::new(RefCell::new(Vec::new()));
    for hex in ПАЛИТРА {
        let б = gtk::Button::new();
        б.style_context().add_class("sw");
        б.set_valign(gtk::Align::Center);
        let с = gtk::CssProvider::new();
        let _ = с.load_from_data(format!("button {{ background-image: none; background-color: {hex}; }}").as_bytes());
        б.style_context().add_provider(&с, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2);
        if *hex == "#3a86ff" {
            б.style_context().add_class("on");
        }
        let (х2, все, hex) = (х.clone(), образцы.clone(), hex.to_string());
        б.connect_clicked(move |б| {
            х2.цвет.set(цвет_из_hex(&hex));
            for к in все.borrow().iter() {
                к.style_context().remove_class("on");
            }
            б.style_context().add_class("on");
        });
        образцы.borrow_mut().push(б.clone());
        верх.add(&б);
    }
    let свой = gtk::ColorButton::new();
    свой.set_tooltip_text(Some("Свой цвет"));
    свой.set_valign(gtk::Align::Center);
    {
        let (х2, все) = (х.clone(), образцы.clone());
        свой.connect_color_set(move |к| {
            let ц = к.rgba();
            х2.цвет.set((ц.red(), ц.green(), ц.blue()));
            for б in все.borrow().iter() {
                б.style_context().remove_class("on");
            }
        });
    }
    верх.add(&свой);
    let толщина = gtk::Scale::with_range(gtk::Orientation::Horizontal, 1.0, 40.0, 1.0);
    толщина.set_value(5.0);
    толщина.set_size_request(100, -1);
    толщина.set_tooltip_text(Some("Толщина"));
    толщина.set_value_pos(gtk::PositionType::Right);
    {
        let х2 = х.clone();
        толщина.connect_value_changed(move |т| х2.толщина.set(т.value()));
    }
    верх.add(&толщина);
    let пусто = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    пусто.set_hexpand(true);
    верх.add(&пусто);

    let отменить = кнопка("↶", "Отменить (Ctrl+Z)");
    let вернуть = кнопка("↷", "Вернуть (Ctrl+Y)");
    let новый = кнопка("📄", "Чистый холст");
    let открыть = кнопка("📂", "Открыть картинку");
    let сохранить = кнопка("💾 Сохранить", "Сохранить в PNG (Ctrl+S)");
    сохранить.style_context().add_class("accent");
    for б in [&отменить, &вернуть, &новый, &открыть, &сохранить] {
        верх.add(б);
    }
    let отмена = {
        let (х, холст) = (х.clone(), холст.clone());
        Rc::new(move || {
            let был = х.отмена.borrow_mut().pop();
            if let Some(б) = был {
                let сейчас = std::mem::replace(&mut *х.картинка.borrow_mut(), б);
                х.повтор.borrow_mut().push(сейчас);
                холст.queue_draw();
            }
        })
    };
    let повтор = {
        let (х, холст) = (х.clone(), холст.clone());
        Rc::new(move || {
            let был = х.повтор.borrow_mut().pop();
            if let Some(б) = был {
                let сейчас = std::mem::replace(&mut *х.картинка.borrow_mut(), б);
                х.отмена.borrow_mut().push(сейчас);
                холст.queue_draw();
            }
        })
    };
    {
        let о = отмена.clone();
        отменить.connect_clicked(move |_| о());
    }
    {
        let п = повтор.clone();
        вернуть.connect_clicked(move |_| п());
    }
    {
        let (х, по_размеру, окно2) = (х.clone(), по_размеру.clone(), окно.clone());
        новый.connect_clicked(move |_| {
            запомни(&х);
            *х.картинка.borrow_mut() = белый(960, 600);
            *х.файл.borrow_mut() = None;
            окно2.set_title("Без имени — Paint");
            по_размеру();
        });
    }
    {
        let (х, по_размеру, окно2) = (х.clone(), по_размеру.clone(), окно.clone());
        открыть.connect_clicked(move |_| {
            let д = gtk::FileChooserNative::new(Some("Открыть картинку"), Some(&окно2), gtk::FileChooserAction::Open, Some("Открыть"), Some("Отмена"));
            д.set_current_folder(папка_изображений());
            let ф = gtk::FileFilter::new();
            ф.set_name(Some("Картинки"));
            ф.add_pixbuf_formats();
            д.add_filter(ф);
            if д.run() == gtk::ResponseType::Accept {
                if let Some(п) = д.filename() {
                    if let Ok(к) = Pixbuf::from_file(&п) {
                        запомни(&х);
                        *х.картинка.borrow_mut() = из_картинки(&к);
                        окно2.set_title(&format!("{} — Paint", п.file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_default()));
                        *х.файл.borrow_mut() = Some(п);
                        х.изменён.set(false);
                        по_размеру();
                    }
                }
            }
        });
    }
    {
        let (х, окно2) = (х.clone(), окно.clone());
        сохранить.connect_clicked(move |_| {
            сохрани(&окно2, &х, false);
        });
    }
    {
        let (х, окно2, по_размеру) = (х.clone(), окно.clone(), по_размеру.clone());
        окно.connect_key_press_event(move |_, е| {
            let ctrl = е.state().contains(gtk::gdk::ModifierType::CONTROL_MASK);
            let к = е.keyval().to_lower();
            use gtk::gdk::keys::constants as кл;
            if ctrl && к == кл::z {
                отмена();
            } else if ctrl && к == кл::y {
                повтор();
            } else if ctrl && к == кл::s {
                сохрани(&окно2, &х, е.state().contains(gtk::gdk::ModifierType::SHIFT_MASK));
            } else if ctrl && к == кл::v {
                // Вставка картинки из буфера — поверх, в левый верхний угол
                if let Some(к) = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD).wait_for_image() {
                    запомни(&х);
                    {
                        let п = х.картинка.borrow();
                        let с = cairo::Context::new(&*п).expect("кисть");
                        с.set_source_pixbuf(&к, 0.0, 0.0);
                        let _ = с.paint();
                    }
                    по_размеру();
                }
            } else {
                return glib::Propagation::Proceed;
            }
            glib::Propagation::Stop
        });
    }
    // Несохранённый рисунок не теряем молча
    {
        let х = х.clone();
        окно.connect_delete_event(move |о, _| {
            if !х.изменён.get() {
                return glib::Propagation::Proceed;
            }
            let д = gtk::MessageDialog::new(Some(о), gtk::DialogFlags::MODAL, gtk::MessageType::Question, gtk::ButtonsType::None, "Сохранить рисунок?");
            д.set_secondary_text(Some("Иначе изменения пропадут."));
            д.add_button("Не сохранять", gtk::ResponseType::Reject);
            д.add_button("Отмена", gtk::ResponseType::Cancel);
            д.add_button("Сохранить", gtk::ResponseType::Accept);
            let ответ = д.run();
            unsafe { д.destroy() };
            match ответ {
                gtk::ResponseType::Reject => glib::Propagation::Proceed,
                gtk::ResponseType::Accept if сохрани(о, &х, false) => glib::Propagation::Proceed,
                _ => glib::Propagation::Stop,
            }
        });
    }
    окно.connect_destroy(|_| gtk::main_quit());
    окно.show_all();
    gtk::main();
    std::process::exit(0);
}

fn спроси_текст(окно: &gtk::Window) -> Option<String> {
    let д = gtk::Dialog::with_buttons(Some("Текст"), Some(окно), gtk::DialogFlags::MODAL, &[("Отмена", gtk::ResponseType::Cancel), ("Готово", gtk::ResponseType::Accept)]);
    let поле = gtk::Entry::new();
    поле.set_width_chars(30);
    поле.set_margin(12);
    {
        let д = д.clone();
        поле.connect_activate(move |_| д.response(gtk::ResponseType::Accept));
    }
    д.content_area().add(&поле);
    д.show_all();
    let ответ = д.run();
    let т = поле.text().to_string();
    unsafe { д.destroy() };
    (ответ == gtk::ResponseType::Accept && !т.trim().is_empty()).then_some(т)
}

#[cfg(test)]
mod проверки {
    use super::*;

    #[test]
    fn заливка_не_выходит_за_границу() {
        let mut п = белый(20, 20);
        {
            let к = cairo::Context::new(&п).unwrap();
            к.set_source_rgb(0.0, 0.0, 0.0);
            к.rectangle(10.0, 0.0, 1.0, 20.0);
            let _ = к.fill();
        }
        залей(&mut п, 2, 2, (1.0, 0.0, 0.0));
        п.flush();
        let шаг = п.stride() as usize;
        let д = п.data().unwrap();
        // слева от черты — красное (B G R A), справа — белое
        assert_eq!(&д[2 * шаг + 2 * 4..2 * шаг + 2 * 4 + 4], &[0, 0, 255, 255]);
        assert_eq!(&д[2 * шаг + 15 * 4..2 * шаг + 15 * 4 + 3], &[255, 255, 255]);
    }

    #[test]
    fn цвет_из_строки() {
        assert_eq!(цвет_из_hex("#ff0000"), (1.0, 0.0, 0.0));
    }
}
