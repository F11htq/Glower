//! Свойства файла или папки (glower-props путь…).
//!
//! Что это, где лежит, сколько весит (папку считаем в стороне, окно не
//! замирает), когда создано и менялось, права и чем открывается.

use crate::apps::подготовь;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const СТИЛЬ: &str = r#"
.props .name { font-size: 18px; font-weight: bold; }
.props .key { opacity: 0.6; }
"#;

pub fn размер_словами(б: u64) -> String {
    let ч = б as f64;
    let кр = if ч >= 1073741824.0 {
        format!("{:.1} ГБ", ч / 1073741824.0)
    } else if ч >= 1048576.0 {
        format!("{:.1} МБ", ч / 1048576.0)
    } else if ч >= 1024.0 {
        format!("{:.0} КБ", ч / 1024.0)
    } else {
        return format!("{б} байт");
    };
    format!("{} ({} байт)", кр.replace('.', ","), б)
}

/// Размер папки и сколько в ней файлов — обходом, без ссылок
pub fn посчитай(п: &Path) -> (u64, u64, u64) {
    let (mut байт, mut файлов, mut папок) = (0u64, 0u64, 0u64);
    let mut стопка = vec![п.to_path_buf()];
    while let Some(п) = стопка.pop() {
        let Ok(ч) = std::fs::read_dir(&п) else { continue };
        for з in ч.flatten() {
            let Ok(м) = з.path().symlink_metadata() else { continue };
            if м.is_dir() {
                папок += 1;
                стопка.push(з.path());
            } else {
                файлов += 1;
                байт += м.len();
            }
        }
    }
    (байт, файлов, папок)
}

fn когда(с: Option<std::time::SystemTime>) -> String {
    с.and_then(|в| в.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|д| glib::DateTime::from_unix_local(д.as_secs() as i64).ok())
        .and_then(|д| д.format("%d.%m.%Y, %H:%M").ok())
        .map(|т| т.to_string())
        .unwrap_or_else(|| "—".into())
}

pub fn запусти(доводы: Vec<String>) -> ! {
    подготовь("glower-props", "Свойства");
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль свойств");
    gtk::StyleContext::add_provider_for_screen(&gtk::gdk::Screen::default().expect("экран"), &стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
    let пути: Vec<PathBuf> = доводы.iter().filter(|д| !д.starts_with("--")).map(PathBuf::from).collect();
    if пути.is_empty() {
        eprintln!("glower-props: какой файл показать?");
        std::process::exit(2);
    }
    for п in &пути {
        окно_свойств(п);
    }
    gtk::main();
    std::process::exit(0);
}

thread_local! {
    static ОКОН: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

fn окно_свойств(путь: &Path) {
    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    let имя = путь.file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_else(|| путь.to_string_lossy().to_string());
    окно.set_title(&format!("Свойства: {имя}"));
    окно.set_default_size(420, -1);
    окно.style_context().add_class("glower-app");
    crate::apps::строка_заголовка(&окно);
    окно.style_context().add_class("props");
    ОКОН.with(|о| о.set(о.get() + 1));
    окно.connect_destroy(|_| {
        ОКОН.with(|о| {
            о.set(о.get() - 1);
            if о.get() == 0 {
                gtk::main_quit();
            }
        })
    });
    let к = gtk::Box::new(gtk::Orientation::Vertical, 12);
    к.set_margin(18);
    окно.add(&к);

    let файл = gio::File::for_path(путь);
    let сведения = файл.query_info("standard::*", gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE).ok();
    let тип = сведения.as_ref().and_then(|с| с.content_type()).map(|т| т.to_string()).unwrap_or_default();
    let шапка = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let значок = сведения.as_ref().and_then(|с| с.icon()).map(|и| gtk::Image::from_gicon(&и, gtk::IconSize::Dialog)).unwrap_or_else(|| gtk::Image::from_icon_name(Some("text-x-generic"), gtk::IconSize::Dialog));
    значок.set_pixel_size(56);
    шапка.add(&значок);
    let и = gtk::Label::new(Some(&имя));
    и.style_context().add_class("name");
    и.set_xalign(0.0);
    и.set_line_wrap(true);
    и.set_selectable(true);
    шапка.add(&и);
    к.add(&шапка);

    let сетка = gtk::Grid::new();
    сетка.set_row_spacing(8);
    сетка.set_column_spacing(16);
    к.add(&сетка);
    let строка = {
        let сетка = сетка.clone();
        let n = std::rc::Rc::new(std::cell::Cell::new(0));
        move |ключ: &str, значение: &str| -> gtk::Label {
            let р = n.get();
            n.set(р + 1);
            let кл = gtk::Label::new(Some(ключ));
            кл.style_context().add_class("key");
            кл.set_xalign(1.0);
            кл.set_valign(gtk::Align::Start);
            сетка.attach(&кл, 0, р, 1, 1);
            let з = gtk::Label::new(Some(значение));
            з.set_xalign(0.0);
            з.set_line_wrap(true);
            з.set_max_width_chars(40);
            з.set_selectable(true);
            сетка.attach(&з, 1, р, 1, 1);
            з
        }
    };
    let м = путь.metadata().ok();
    let папка = м.as_ref().is_some_and(|м| м.is_dir());
    строка("Тип", &if папка { "Папка".to_string() } else { format!("{} ({тип})", gio::content_type_get_description(&тип)) });
    строка("Где", &путь.parent().map(|р| р.to_string_lossy().to_string()).unwrap_or_default());
    if папка {
        let з = строка("Размер", "Считаю…");
        let п = путь.to_path_buf();
        let (куда, откуда) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let _ = куда.send_blocking(посчитай(&п));
        });
        glib::MainContext::default().spawn_local(async move {
            if let Ok((байт, файлов, папок)) = откуда.recv().await {
                з.set_text(&format!("{}\nфайлов: {файлов}, папок: {папок}", размер_словами(байт)));
            }
        });
    } else {
        строка("Размер", &размер_словами(м.as_ref().map(|м| м.len()).unwrap_or(0)));
    }
    строка("Создан", &когда(м.as_ref().and_then(|м| м.created().ok())));
    строка("Изменён", &когда(м.as_ref().and_then(|м| м.modified().ok())));
    строка("Открыт", &когда(м.as_ref().and_then(|м| м.accessed().ok())));
    if !папка {
        if let Some(п) = gio::AppInfo::default_for_type(&тип, false) {
            строка("Открывается", &п.name());
        }
    }

    // Права: только то, что человеку понятно
    if let Some(м) = &м {
        let режим = м.permissions().mode();
        let чтение = gtk::CheckButton::with_label("Только для чтения");
        чтение.set_active(режим & 0o200 == 0);
        к.add(&чтение);
        let п = путь.to_path_buf();
        чтение.connect_toggled(move |ч| {
            if let Ok(м) = п.metadata() {
                let mut права = м.permissions();
                let р = права.mode();
                права.set_mode(if ч.is_active() { р & !0o222 } else { р | 0o200 });
                let _ = std::fs::set_permissions(&п, права);
            }
        });
        if !папка {
            let запуск = gtk::CheckButton::with_label("Разрешить запускать как программу");
            запуск.set_active(режим & 0o100 != 0);
            к.add(&запуск);
            let п = путь.to_path_buf();
            запуск.connect_toggled(move |з| {
                if let Ok(м) = п.metadata() {
                    let mut права = м.permissions();
                    let р = права.mode();
                    // запуск — тем же, кому можно читать
                    права.set_mode(if з.is_active() { р | ((р & 0o444) >> 2) } else { р & !0o111 });
                    let _ = std::fs::set_permissions(&п, права);
                }
            });
        }
    }
    let закрыть = gtk::Button::with_label("Закрыть");
    закрыть.set_halign(gtk::Align::End);
    let о = окно.clone();
    закрыть.connect_clicked(move |_| о.close());
    к.add(&закрыть);
    окно.show_all();
    // Фокус — на «Закрыть»: иначе его забирала первая выделяемая подпись,
    // и имя файла открывалось целиком выделенным
    закрыть.grab_focus();
}

#[cfg(test)]
mod проверки {
    use super::*;

    #[test]
    fn размер_по_русски() {
        assert_eq!(размер_словами(512), "512 байт");
        assert_eq!(размер_словами(2048), "2 КБ (2048 байт)");
        assert_eq!(размер_словами(5 * 1048576), "5,0 МБ (5242880 байт)");
    }

    #[test]
    fn считает_папку() {
        let д = std::env::temp_dir().join(format!("glower-props-{}", std::process::id()));
        std::fs::create_dir_all(д.join("в")).unwrap();
        std::fs::write(д.join("а.txt"), b"12345").unwrap();
        std::fs::write(д.join("в").join("б.txt"), b"123").unwrap();
        assert_eq!(посчитай(&д), (8, 2, 1));
        std::fs::remove_dir_all(&д).unwrap();
    }
}
