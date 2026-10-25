//! Фото — нативный просмотр изображений, настоящим окном (glower-photos).
//!
//! Открывает картинку и листает соседние в той же папке: стрелками, колесом
//! с Ctrl — масштаб, перетаскиванием — сдвиг. Повернуть, сделать обоями,
//! удалить в корзину, показать в Проводнике, на весь экран.

use crate::agent::скажи;
use crate::apps::{кнопка, подготовь};
use gtk::gdk_pixbuf::{InterpType, Pixbuf, PixbufRotation};
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use serde_json::json;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

const СТИЛЬ: &str = r#"
.photos .stage { background-color: #0b0c10; }
.photos .empty { font-size: 15px; opacity: 0.6; }
"#;

const РАСШИРЕНИЯ: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "tif", "tiff", "ico", "avif", "heic"];

pub fn картинка_ли(путь: &Path) -> bool {
    путь.extension().and_then(|р| р.to_str()).is_some_and(|р| РАСШИРЕНИЯ.contains(&р.to_lowercase().as_str()))
}

/// Картинки папки по имени — в том порядке, в каком их листают
pub fn соседи(папка: &Path) -> Vec<PathBuf> {
    let mut список: Vec<PathBuf> = std::fs::read_dir(папка)
        .map(|ч| ч.filter_map(|з| з.ok()).map(|з| з.path()).filter(|п| п.is_file() && картинка_ли(п)).collect())
        .unwrap_or_default();
    список.sort_by_key(|п| п.file_name().map(|и| и.to_string_lossy().to_lowercase()).unwrap_or_default());
    список
}

struct Фото {
    окно: gtk::Window,
    холст: gtk::DrawingArea,
    строка: gtk::Label,
    список: RefCell<Vec<PathBuf>>,
    номер: Cell<usize>,
    кадр: RefCell<Option<Pixbuf>>,
    /// Масштаб; 0 — «вписать в окно»
    масштаб: Cell<f64>,
    сдвиг: Cell<(f64, f64)>,
    тянем: Cell<Option<(f64, f64)>>,
    весь_экран: Cell<bool>,
}

type Ф = Rc<Фото>;

fn текущий(ф: &Ф) -> Option<PathBuf> {
    ф.список.borrow().get(ф.номер.get()).cloned()
}

fn загрузи(ф: &Ф) {
    ф.масштаб.set(0.0);
    ф.сдвиг.set((0.0, 0.0));
    let Some(путь) = текущий(ф) else {
        *ф.кадр.borrow_mut() = None;
        ф.окно.set_title("Фото");
        ф.строка.set_text("");
        ф.холст.queue_draw();
        return;
    };
    // Снимок с телефона лежит боком, а повернуть его просит EXIF
    let кадр = Pixbuf::from_file(&путь).ok().map(|к| к.apply_embedded_orientation().unwrap_or(к));
    let имя = путь.file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_default();
    ф.окно.set_title(&format!("{имя} — Фото"));
    let байт = std::fs::metadata(&путь).map(|м| м.len()).unwrap_or(0);
    let вес = if байт > 1048576 { format!("{:.1} МБ", байт as f64 / 1048576.0).replace('.', ",") } else { format!("{} КБ", байт / 1024) };
    ф.строка.set_text(&match &кадр {
        Some(к) => format!("{} из {}   ·   {}×{}   ·   {вес}", ф.номер.get() + 1, ф.список.borrow().len(), к.width(), к.height()),
        None => format!("{} из {}   ·   эту картинку не прочитать", ф.номер.get() + 1, ф.список.borrow().len()),
    });
    *ф.кадр.borrow_mut() = кадр;
    ф.холст.queue_draw();
}

fn листай(ф: &Ф, шаг: i64) {
    let всего = ф.список.borrow().len() as i64;
    if всего == 0 {
        return;
    }
    ф.номер.set(((ф.номер.get() as i64 + шаг).rem_euclid(всего)) as usize);
    загрузи(ф);
}

/// Масштаб, которым картинка вписывается в окно (крупнее 1:1 не растягиваем)
fn вписать(ф: &Ф, к: &Pixbuf) -> f64 {
    let ш = ф.холст.allocated_width() as f64;
    let в = ф.холст.allocated_height() as f64;
    (ш / к.width() as f64).min(в / к.height() as f64).min(1.0)
}

fn масштаб(ф: &Ф) -> f64 {
    match ф.кадр.borrow().as_ref() {
        Some(к) if ф.масштаб.get() == 0.0 => вписать(ф, к),
        _ => ф.масштаб.get(),
    }
}

fn приблизь(ф: &Ф, во_сколько: f64) {
    let м = (масштаб(ф) * во_сколько).clamp(0.05, 16.0);
    ф.масштаб.set(м);
    ф.холст.queue_draw();
}

fn поверни(ф: &Ф, по_часовой: bool) {
    let повёрнут = ф.кадр.borrow().as_ref().and_then(|к| к.rotate_simple(if по_часовой { PixbufRotation::Clockwise } else { PixbufRotation::Counterclockwise }));
    if повёрнут.is_some() {
        *ф.кадр.borrow_mut() = повёрнут;
        ф.масштаб.set(0.0);
        ф.холст.queue_draw();
    }
}

fn удали(ф: &Ф) {
    let Some(путь) = текущий(ф) else { return };
    if let Err(е) = gio::File::for_path(&путь).trash(gio::Cancellable::NONE) {
        let д = gtk::MessageDialog::new(Some(&ф.окно), gtk::DialogFlags::MODAL, gtk::MessageType::Warning, gtk::ButtonsType::Close, "Не удалось удалить");
        д.set_secondary_text(Some(&е.to_string()));
        д.run();
        unsafe { д.destroy() };
        return;
    }
    let н = ф.номер.get();
    ф.список.borrow_mut().remove(н);
    let всего = ф.список.borrow().len();
    ф.номер.set(if всего == 0 { 0 } else { н.min(всего - 1) });
    загрузи(ф);
}

fn открой_диалогом(ф: &Ф) {
    let д = gtk::FileChooserDialog::with_buttons(
        Some("Открыть изображение"),
        Some(&ф.окно),
        gtk::FileChooserAction::Open,
        &[("Отмена", gtk::ResponseType::Cancel), ("Открыть", gtk::ResponseType::Accept)],
    );
    let фильтр = gtk::FileFilter::new();
    фильтр.set_name(Some("Изображения"));
    фильтр.add_mime_type("image/*");
    д.add_filter(фильтр);
    if д.run() == gtk::ResponseType::Accept {
        if let Some(п) = д.filename() {
            открой_путь(ф, &п);
        }
    }
    unsafe { д.destroy() };
}

fn открой_путь(ф: &Ф, путь: &Path) {
    if путь.is_dir() {
        *ф.список.borrow_mut() = соседи(путь);
        ф.номер.set(0);
    } else {
        let папка = путь.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut список = соседи(&папка);
        if !список.iter().any(|п| п == путь) {
            список.push(путь.to_path_buf());
        }
        ф.номер.set(список.iter().position(|п| п == путь).unwrap_or(0));
        *ф.список.borrow_mut() = список;
    }
    загрузи(ф);
}

fn рисуй(ф: &Ф, к: &gtk::cairo::Context) {
    let ш = ф.холст.allocated_width() as f64;
    let в = ф.холст.allocated_height() as f64;
    let Some(кадр) = ф.кадр.borrow().clone() else {
        // Пусто — так и говорим
        к.set_source_rgba(1.0, 1.0, 1.0, 0.55);
        к.select_font_face("sans-serif", gtk::cairo::FontSlant::Normal, gtk::cairo::FontWeight::Normal);
        к.set_font_size(15.0);
        let текст = if ф.список.borrow().is_empty() { "Здесь нет изображений — откройте файл (Ctrl+O)" } else { "Эту картинку не прочитать" };
        if let Ok(р) = к.text_extents(текст) {
            к.move_to((ш - р.width()) / 2.0, в / 2.0);
            let _ = к.show_text(текст);
        }
        return;
    };
    let м = масштаб(ф);
    let (кш, кв) = (кадр.width() as f64 * м, кадр.height() as f64 * м);
    let (сx, сy) = ф.сдвиг.get();
    let x = (ш - кш) / 2.0 + сx;
    let y = (в - кв) / 2.0 + сy;
    // Сжимаем заранее хорошим фильтром: cairo при сильном уменьшении сыплет
    let показ = if м < 1.0 && кш >= 1.0 && кв >= 1.0 {
        кадр.scale_simple(кш.round() as i32, кв.round() as i32, InterpType::Bilinear).unwrap_or(кадр)
    } else {
        кадр
    };
    к.save().ok();
    к.translate(x, y);
    if м >= 1.0 {
        к.scale(м, м);
    }
    к.set_source_pixbuf(&показ, 0.0, 0.0);
    if м > 2.0 {
        // Крупно — пиксели видны как есть, без мыла
        let узор = к.source();
        узор.set_filter(gtk::cairo::Filter::Nearest);
    }
    let _ = к.paint();
    к.restore().ok();
}

pub fn запусти(доводы: Vec<String>) -> ! {
    подготовь("glower-photos", "Фото");
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль фото");
    gtk::StyleContext::add_provider_for_screen(&gtk::gdk::Screen::default().expect("экран"), &стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_default_size(1000, 700);
    окно.set_icon_name(Some("image-viewer"));
    окно.style_context().add_class("glower-app");
    crate::apps::строка_заголовка(&окно);
    окно.style_context().add_class("photos");
    let колонка = gtk::Box::new(gtk::Orientation::Vertical, 0);
    окно.add(&колонка);

    let панель = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    панель.style_context().add_class("toolbar");
    let к_назад = кнопка("‹", "Предыдущее (←)");
    let к_вперёд = кнопка("›", "Следующее (→)");
    let к_меньше = кнопка("⊖", "Мельче (Ctrl + колесо, −)");
    let к_вписать = кнопка("⤢", "Вписать в окно (0)");
    let к_больше = кнопка("⊕", "Крупнее (Ctrl + колесо, +)");
    let к_влево = кнопка("⟲", "Повернуть влево");
    let к_вправо = кнопка("⟳", "Повернуть вправо");
    let пустота = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    пустота.set_hexpand(true);
    let к_обои = кнопка("🖼 Сделать обоями", "Поставить на рабочий стол");
    let к_папка = кнопка("📁", "Показать в Проводнике");
    let к_открыть = кнопка("📂", "Открыть файл (Ctrl+O)");
    let к_удалить = кнопка("🗑", "В корзину (Delete)");
    let к_экран = кнопка("⛶", "На весь экран (F11)");
    for б in [&к_назад, &к_вперёд, &к_меньше, &к_вписать, &к_больше, &к_влево, &к_вправо] {
        панель.add(б);
    }
    панель.add(&пустота);
    for б in [&к_обои, &к_папка, &к_открыть, &к_удалить, &к_экран] {
        панель.add(б);
    }
    колонка.add(&панель);

    let холст = gtk::DrawingArea::new();
    холст.style_context().add_class("stage");
    холст.set_vexpand(true);
    холст.add_events(
        gtk::gdk::EventMask::SCROLL_MASK
            | gtk::gdk::EventMask::SMOOTH_SCROLL_MASK
            | gtk::gdk::EventMask::BUTTON_PRESS_MASK
            | gtk::gdk::EventMask::BUTTON_RELEASE_MASK
            | gtk::gdk::EventMask::POINTER_MOTION_MASK,
    );
    колонка.add(&холст);
    let строка = gtk::Label::new(None);
    строка.set_xalign(0.0);
    строка.style_context().add_class("statusbar");
    колонка.add(&строка);

    let ф: Ф = Rc::new(Фото {
        окно: окно.clone(),
        холст: холст.clone(),
        строка: строка.clone(),
        список: RefCell::new(Vec::new()),
        номер: Cell::new(0),
        кадр: RefCell::new(None),
        масштаб: Cell::new(0.0),
        сдвиг: Cell::new((0.0, 0.0)),
        тянем: Cell::new(None),
        весь_экран: Cell::new(false),
    });
    {
        let фв = Rc::downgrade(&ф);
        crate::fileops::принимай_в_окно(&окно, move |пути| {
            if let (Some(ф), Some(п)) = (фв.upgrade(), пути.first()) {
                открой_путь(&ф, п);
            }
        });
    }

    {
        let ф1 = ф.clone();
        холст.connect_draw(move |_, к| {
            к.set_source_rgb(0.043, 0.047, 0.063);
            let _ = к.paint();
            рисуй(&ф1, к);
            glib::Propagation::Stop
        });
        let ф1 = ф.clone();
        холст.connect_scroll_event(move |_, е| {
            let вверх = match е.direction() {
                gtk::gdk::ScrollDirection::Up => true,
                gtk::gdk::ScrollDirection::Down => false,
                gtk::gdk::ScrollDirection::Smooth => е.delta().1 < 0.0,
                _ => return glib::Propagation::Proceed,
            };
            if е.state().contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                приблизь(&ф1, if вверх { 1.15 } else { 1.0 / 1.15 });
            } else {
                листай(&ф1, if вверх { -1 } else { 1 });
            }
            glib::Propagation::Stop
        });
        let ф1 = ф.clone();
        холст.connect_button_press_event(move |_, е| {
            if е.event_type() == gtk::gdk::EventType::DoubleButtonPress {
                // Двойной щелчок: вписать ↔ один к одному
                ф1.масштаб.set(if ф1.масштаб.get() == 0.0 { 1.0 } else { 0.0 });
                ф1.сдвиг.set((0.0, 0.0));
                ф1.холст.queue_draw();
            } else if е.button() == 1 {
                let (x, y) = е.position();
                ф1.тянем.set(Some((x, y)));
            }
            glib::Propagation::Stop
        });
        let ф1 = ф.clone();
        холст.connect_motion_notify_event(move |_, е| {
            if let Some((x0, y0)) = ф1.тянем.get() {
                let (x, y) = е.position();
                let (сx, сy) = ф1.сдвиг.get();
                ф1.сдвиг.set((сx + x - x0, сy + y - y0));
                ф1.тянем.set(Some((x, y)));
                ф1.холст.queue_draw();
            }
            glib::Propagation::Proceed
        });
        let ф1 = ф.clone();
        холст.connect_button_release_event(move |_, _| {
            ф1.тянем.set(None);
            glib::Propagation::Proceed
        });

        let ф1 = ф.clone();
        к_назад.connect_clicked(move |_| листай(&ф1, -1));
        let ф1 = ф.clone();
        к_вперёд.connect_clicked(move |_| листай(&ф1, 1));
        let ф1 = ф.clone();
        к_меньше.connect_clicked(move |_| приблизь(&ф1, 1.0 / 1.25));
        let ф1 = ф.clone();
        к_больше.connect_clicked(move |_| приблизь(&ф1, 1.25));
        let ф1 = ф.clone();
        к_вписать.connect_clicked(move |_| {
            ф1.масштаб.set(0.0);
            ф1.сдвиг.set((0.0, 0.0));
            ф1.холст.queue_draw();
        });
        let ф1 = ф.clone();
        к_влево.connect_clicked(move |_| поверни(&ф1, false));
        let ф1 = ф.clone();
        к_вправо.connect_clicked(move |_| поверни(&ф1, true));
        let ф1 = ф.clone();
        к_удалить.connect_clicked(move |_| удали(&ф1));
        let ф1 = ф.clone();
        к_открыть.connect_clicked(move |_| открой_диалогом(&ф1));
        let ф1 = ф.clone();
        к_папка.connect_clicked(move |_| {
            if let Some(п) = текущий(&ф1) {
                let _ = std::process::Command::new("glower-files").arg(п).spawn();
            }
        });
        let ф1 = ф.clone();
        к_обои.connect_clicked(move |_| {
            if let Some(п) = текущий(&ф1) {
                // Обои живут в оболочке — просим её, как Параметры
                скажи("параметры-дело", json!({ "что": "set", "k": "wallpaper", "v": format!("файл:{}", п.display()) }));
            }
        });
        let ф1 = ф.clone();
        к_экран.connect_clicked(move |_| {
            if ф1.весь_экран.get() { ф1.окно.unfullscreen() } else { ф1.окно.fullscreen() }
        });
        let ф1 = ф.clone();
        окно.connect_window_state_event(move |_, е| {
            let весь = е.new_window_state().contains(gtk::gdk::WindowState::FULLSCREEN);
            ф1.весь_экран.set(весь);
            glib::Propagation::Proceed
        });
        let (ф1, панель2, строка2) = (ф.clone(), панель.clone(), строка.clone());
        окно.connect_key_press_event(move |окно, е| {
            use gtk::gdk::keys::constants as к;
            let ключ = е.keyval();
            let ctrl = е.state().contains(gtk::gdk::ModifierType::CONTROL_MASK);
            if ctrl && crate::notepad::латинская(е) == "o" {
                открой_диалогом(&ф1);
            } else if ключ == к::Left || ключ == к::Page_Up || ключ == к::BackSpace {
                листай(&ф1, -1);
            } else if ключ == к::Right || ключ == к::Page_Down || ключ == к::space {
                листай(&ф1, 1);
            } else if ключ == к::Home {
                ф1.номер.set(0);
                загрузи(&ф1);
            } else if ключ == к::End {
                ф1.номер.set(ф1.список.borrow().len().saturating_sub(1));
                загрузи(&ф1);
            } else if ключ == к::plus || ключ == к::KP_Add || ключ == к::equal {
                приблизь(&ф1, 1.25);
            } else if ключ == к::minus || ключ == к::KP_Subtract {
                приблизь(&ф1, 1.0 / 1.25);
            } else if ключ == к::_0 || ключ == к::KP_0 {
                ф1.масштаб.set(0.0);
                ф1.сдвиг.set((0.0, 0.0));
                ф1.холст.queue_draw();
            } else if ключ == к::_1 {
                ф1.масштаб.set(1.0);
                ф1.холст.queue_draw();
            } else if ключ == к::Delete {
                удали(&ф1);
            } else if ключ == к::F11 || (ключ == к::Escape && ф1.весь_экран.get()) {
                if ф1.весь_экран.get() { окно.unfullscreen() } else { окно.fullscreen() }
            } else {
                return glib::Propagation::Proceed;
            }
            glib::Propagation::Stop
        });
        // На весь экран — только картинка
        let (панель3, строка3) = (панель2.clone(), строка2.clone());
        окно.connect_window_state_event(move |_, е| {
            let весь = е.new_window_state().contains(gtk::gdk::WindowState::FULLSCREEN);
            панель3.set_visible(!весь);
            строка3.set_visible(!весь);
            glib::Propagation::Proceed
        });
    }
    окно.connect_delete_event(|_, _| {
        gtk::main_quit();
        glib::Propagation::Proceed
    });
    окно.show_all();

    // Что открыть: файл или папку из доводов, иначе — Изображения
    let путь = доводы.first().map(PathBuf::from).filter(|п| п.exists()).unwrap_or_else(|| {
        let корень = std::env::var("GLOWER_ROOT").map(PathBuf::from).unwrap_or_else(|_| glib::home_dir().join("GlowerOS"));
        [корень.join("Изображения"), glib::home_dir().join("Изображения"), glib::home_dir().join("Pictures")]
            .into_iter()
            .find(|п| п.is_dir())
            .unwrap_or_else(glib::home_dir)
    });
    открой_путь(&ф, &путь);
    gtk::main();
    std::process::exit(0);
}

#[cfg(test)]
mod проверки {
    use super::*;

    #[test]
    fn узнаёт_картинки_и_листает_по_имени() {
        assert!(картинка_ли(Path::new("/x/Снимок.PNG")));
        assert!(!картинка_ли(Path::new("/x/заметка.txt")));
        let папка = std::env::temp_dir().join(format!("glower-photos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&папка);
        std::fs::create_dir_all(&папка).unwrap();
        for имя in ["б.jpg", "А.png", "в.txt"] {
            std::fs::write(папка.join(имя), b"").unwrap();
        }
        let имена: Vec<String> = соседи(&папка).iter().map(|п| п.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(имена, vec!["А.png", "б.jpg"]);
        let _ = std::fs::remove_dir_all(&папка);
    }
}
