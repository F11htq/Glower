//! История буфера обмена — Win+V, как в Windows.
//!
//! Что копировали, записывает glower-clip-note: панель держит его под
//! «wl-paste --watch», а тот слушает буфер через протокол data-control —
//! у кого бы ни был фокус. Записи лежат файлами в каталоге сеанса (текст и
//! картинки), здесь мы их только показываем.
//!
//! Выбрали запись — она снова в буфере и сразу вставляется туда, где стоял
//! курсор (Ctrl+V за человека), как в Windows.

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::Duration;

thread_local! {
    static ОКНО: RefCell<Option<gtk::Window>> = const { RefCell::new(None) };
}

pub fn каталог() -> PathBuf {
    glib::user_runtime_dir().join("glower-clip")
}

/// Слушать буфер: «wl-paste --watch glower-clip-note». Упал — поднимаем
/// снова через пару секунд (сеанс оконного сервера мог ещё не подняться)
pub fn следи() {
    let _ = std::fs::create_dir_all(каталог());
    let пошёл = std::process::Command::new("wl-paste")
        .args(["--watch", "glower-clip-note"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    match пошёл {
        Ok(mut ребёнок) => {
            let (куда, откуда) = async_channel::bounded::<()>(1);
            std::thread::spawn(move || {
                let _ = ребёнок.wait();
                let _ = куда.send_blocking(());
            });
            glib::MainContext::default().spawn_local(async move {
                let _ = откуда.recv().await;
                glib::timeout_add_local_once(Duration::from_secs(3), следи);
            });
        }
        // wl-clipboard нет — истории не будет, но и падать незачем
        Err(_) => eprintln!("glower-panel: нет wl-paste — истории буфера не будет"),
    }
}

/// Записи истории: закреплённые первыми, дальше — свежие сверху
fn записи() -> Vec<PathBuf> {
    let mut все: Vec<PathBuf> = std::fs::read_dir(каталог())
        .map(|ч| {
            ч.filter_map(|з| з.ok())
                .map(|з| з.path())
                .filter(|п| matches!(п.extension().and_then(|р| р.to_str()), Some("txt") | Some("png")))
                .collect()
        })
        .unwrap_or_default();
    все.sort_by(|а, б| {
        закреплено(б).cmp(&закреплено(а)).then_with(|| б.file_name().cmp(&а.file_name()))
    });
    все
}

fn закреплено(п: &Path) -> bool {
    PathBuf::from(format!("{}.pin", п.display())).exists()
}

pub fn открыто() -> bool {
    ОКНО.with(|о| о.borrow().is_some())
}

pub fn закрой() {
    if let Some(о) = ОКНО.with(|о| о.borrow_mut().take()) {
        // SAFETY: окно создано нами и больше нигде не хранится
        unsafe { о.destroy() };
    }
}

/// Win+V: открыть историю, второе нажатие — закрыть
pub fn переключи() {
    if открыто() {
        return закрой();
    }
    покажи();
}

/// Положить запись в буфер и вставить её туда, где стоял курсор
fn вставь(путь: &Path) {
    let Ok(данные) = std::fs::read(путь) else { return };
    let картинка = путь.extension().and_then(|р| р.to_str()) == Some("png");
    закрой();
    std::thread::spawn(move || {
        use std::io::Write;
        let mut к = std::process::Command::new("wl-copy");
        if картинка {
            к.args(["--type", "image/png"]);
        }
        if let Ok(mut р) = к.stdin(std::process::Stdio::piped()).spawn() {
            if let Some(mut в) = р.stdin.take() {
                let _ = в.write_all(&данные);
            }
            let _ = р.wait();
        }
        // Окно истории ушло, фокус вернулся программе — теперь Ctrl+V.
        // Под glower-wm его нажимает сам сервер; под labwc — wtype
        std::thread::sleep(Duration::from_millis(180));
        if crate::core::скажи_серверу("paste").is_none() {
            let _ = std::process::Command::new("wtype").args(["-M", "ctrl", "v", "-m", "ctrl"]).status();
        }
    });
}

fn покажи() {
    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_decorated(false);
    окно.set_app_paintable(true);
    if let Some(в) = gtk::prelude::GtkWindowExt::screen(&окно).and_then(|э| э.rgba_visual()) {
        окно.set_visual(Some(&в));
    }
    окно.init_layer_shell();
    окно.set_layer(Layer::Overlay);
    окно.set_namespace("glower-clipboard");
    for край in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        окно.set_anchor(край, true);
    }
    окно.set_exclusive_zone(-1);
    окно.set_keyboard_mode(KeyboardMode::Exclusive);
    окно.style_context().add_class("cliphist");
    // Прозрачный слой на весь экран: щелчок мимо карточки закрывает её
    окно.connect_draw(|_, к| {
        к.set_operator(gtk::cairo::Operator::Source);
        к.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        let _ = к.paint();
        glib::Propagation::Proceed
    });
    окно.connect_key_press_event(|_, е| {
        if е.keyval() == gdk::keys::constants::Escape {
            закрой();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    let фон = gtk::EventBox::new();
    фон.set_visible_window(false);
    фон.connect_button_press_event(|_, _| {
        закрой();
        glib::Propagation::Stop
    });

    let карточка = gtk::Box::new(gtk::Orientation::Vertical, 8);
    карточка.style_context().add_class("card");
    карточка.set_size_request(380, -1);
    карточка.set_halign(gtk::Align::Center);
    карточка.set_valign(gtk::Align::End);
    карточка.set_margin_bottom(96);
    // Щелчки внутри карточки фону не отдаём
    let внутри = gtk::EventBox::new();
    внутри.connect_button_press_event(|_, _| glib::Propagation::Stop);
    внутри.add(&карточка);

    let шапка = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let заголовок = gtk::Label::new(Some("Буфер обмена"));
    заголовок.style_context().add_class("title");
    заголовок.set_xalign(0.0);
    шапка.pack_start(&заголовок, true, true, 0);
    let очистить = gtk::Button::with_label("Очистить всё");
    очистить.style_context().add_class("flat");
    шапка.pack_end(&очистить, false, false, 0);
    карточка.add(&шапка);

    let список = gtk::ListBox::new();
    список.set_selection_mode(gtk::SelectionMode::Browse);
    список.set_activate_on_single_click(true);
    let листание = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    листание.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    листание.set_max_content_height(460);
    листание.set_propagate_natural_height(true);
    листание.add(&список);
    карточка.add(&листание);

    наполни(&список);
    список.connect_row_activated(|_, р| {
        if let Some(п) = путь_строки(р) {
            вставь(&п);
        }
    });
    {
        let список = список.clone();
        очистить.connect_clicked(move |_| {
            for п in записи() {
                if !закреплено(&п) {
                    let _ = std::fs::remove_file(&п);
                }
            }
            наполни(&список);
        });
    }

    фон.add(&внутри);
    окно.add(&фон);
    окно.show_all();
    if let Some(р) = список.row_at_index(0) {
        список.select_row(Some(&р));
        р.grab_focus();
    }
    ОКНО.with(|о| *о.borrow_mut() = Some(окно));
}

fn путь_строки(р: &gtk::ListBoxRow) -> Option<PathBuf> {
    // SAFETY: под этим ключом лежит только PathBuf
    unsafe { р.data::<PathBuf>("glower-clip-path").map(|п| п.as_ref().clone()) }
}

fn наполни(список: &gtk::ListBox) {
    for р in список.children() {
        список.remove(&р);
    }
    let записи = записи();
    if записи.is_empty() {
        let р = gtk::ListBoxRow::new();
        р.set_activatable(false);
        р.set_selectable(false);
        let л = gtk::Label::new(Some("Здесь появится то, что вы скопируете"));
        л.style_context().add_class("dim");
        л.set_margin_top(24);
        л.set_margin_bottom(24);
        р.add(&л);
        список.add(&р);
    }
    for п in записи {
        список.add(&строка(список, &п));
    }
    список.show_all();
}

fn строка(список: &gtk::ListBox, путь: &Path) -> gtk::ListBoxRow {
    let р = gtk::ListBoxRow::new();
    unsafe { р.set_data("glower-clip-path", путь.to_path_buf()) };
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    ряд.style_context().add_class("item");
    let содержимое: gtk::Widget = if путь.extension().and_then(|р| р.to_str()) == Some("png") {
        match gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(путь, 320, 110, true) {
            Ok(к) => gtk::Image::from_pixbuf(Some(&к)).upcast(),
            Err(_) => gtk::Label::new(Some("🖼 картинка")).upcast(),
        }
    } else {
        let текст = std::fs::read_to_string(путь).unwrap_or_default();
        let кусок: String = текст.chars().take(300).collect();
        let л = gtk::Label::new(Some(кусок.trim()));
        л.set_xalign(0.0);
        л.set_line_wrap(true);
        л.set_line_wrap_mode(gtk::pango::WrapMode::WordChar);
        л.set_lines(3);
        л.set_ellipsize(gtk::pango::EllipsizeMode::End);
        л.set_max_width_chars(40);
        // Скопированное — не надписи системы, переводить нельзя
        л.style_context().add_class("no-i18n");
        л.upcast()
    };
    содержимое.set_halign(gtk::Align::Start);
    ряд.pack_start(&содержимое, true, true, 0);

    let кнопки = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let закреп = crate::apps::кнопка("📌", if закреплено(путь) { "Открепить" } else { "Закрепить" });
    if закреплено(путь) {
        закреп.style_context().add_class("on");
    }
    let удалить = crate::apps::кнопка("✕", "Удалить");
    for б in [&закреп, &удалить] {
        б.style_context().add_class("flat");
        кнопки.add(б);
    }
    {
        let (путь, список) = (путь.to_path_buf(), список.clone());
        закреп.connect_clicked(move |_| {
            let метка = PathBuf::from(format!("{}.pin", путь.display()));
            if метка.exists() {
                let _ = std::fs::remove_file(&метка);
            } else {
                let _ = std::fs::write(&метка, b"");
            }
            наполни(&список);
        });
    }
    {
        let (путь, список) = (путь.to_path_buf(), список.clone());
        удалить.connect_clicked(move |_| {
            let _ = std::fs::remove_file(&путь);
            let _ = std::fs::remove_file(format!("{}.pin", путь.display()));
            наполни(&список);
        });
    }
    ряд.pack_end(&кнопки, false, false, 0);
    р.add(&ряд);
    р
}
