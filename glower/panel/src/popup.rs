//! Окошки панели: календарь, быстрые настройки, питание.
//!
//! Окошко — отдельная прозрачная поверхность во весь экран под полосой, а
//! само меню лежит в ней у нужного края. Так же устроена и страничная
//! панель, и по той же причине: поверхность слоя не узнаёт о щелчке мимо
//! себя, а закрыть меню щелчком мимо человек ожидает. Здесь «мимо» — это
//! прозрачная часть той же поверхности.
//!
//! Обычный выпадающий GtkPopover здесь не годится: в GTK3 он рисуется
//! внутри своего окна, и в полосе высотой в тридцать точек от него
//! осталась бы полоска.

use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::RefCell;

/// Где показать: под кнопкой справа или посередине (часы).
#[derive(Clone, Copy, PartialEq)]
pub enum Где {
    Справа,
    Посередине,
    /// Левым краем под самой кнопкой — для плеера рядом с «Программами»
    ПодКнопкой,
}

struct Открытое {
    имя: &'static str,
    окно: gtk::Window,
    кнопка: gtk::Button,
}

thread_local! {
    static ОТКРЫТОЕ: RefCell<Option<Открытое>> = const { RefCell::new(None) };
}

/// Закрыть то, что открыто. Открытое всегда одно.
pub fn закрой() {
    let было = ОТКРЫТОЕ.with(|о| о.borrow_mut().take());
    if let Some(о) = было {
        о.кнопка.style_context().remove_class("open");
        // SAFETY: окно создано нами и больше нигде не хранится
        unsafe { о.окно.destroy() };
    }
}

/// Открыть окошко под кнопкой. Нажали ту же кнопку второй раз — закрываем:
/// так ведут себя меню любой системы.
///
/// `наполни` получает коробку, в которую кладёт содержимое; её можно
/// очищать и наполнять заново (так окошко меняет вид, не закрываясь).
pub fn открой<F>(кнопка: &gtk::Button, имя: &'static str, где: Где, ширина: i32, наполни: F)
where
    F: FnOnce(&gtk::Box),
{
    let то_же = ОТКРЫТОЕ.with(|о| о.borrow().as_ref().is_some_and(|о| о.имя == имя));
    закрой();
    if то_же {
        return;
    }

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_decorated(false);
    окно.set_app_paintable(true);
    if let Some(экран) = gtk::prelude::GtkWindowExt::screen(&окно) {
        if let Some(в) = экран.rgba_visual() {
            окно.set_visual(Some(&в));
        }
    }
    окно.init_layer_shell();
    окно.set_layer(Layer::Top);
    окно.set_namespace("glower-panel-menu");
    for край in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        окно.set_anchor(край, true);
    }
    // Ноль — значит «не заходи на чужую полосу»: окошко встаёт под панелью,
    // а не поверх неё, и сама полоса остаётся видна и нажимаема.
    окно.set_exclusive_zone(0);
    // Клавиатура — пока открыто: чтобы Escape закрывал
    окно.set_keyboard_mode(KeyboardMode::Exclusive);
    окно.connect_draw(|_, к| {
        // Прозрачное именно заливкой: окно рисуем сами, и всё, что не
        // закрашено, осталось бы мусором из прошлых кадров
        к.set_operator(gtk::cairo::Operator::Source);
        к.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        let _ = к.paint();
        gtk::glib::Propagation::Proceed
    });
    окно.connect_key_press_event(|_, с| {
        if с.keyval() == gtk::gdk::keys::constants::Escape {
            закрой();
        }
        gtk::glib::Propagation::Proceed
    });

    // Щелчок мимо меню — закрыть
    let фон = gtk::EventBox::new();
    фон.set_visible_window(false);
    фон.connect_button_press_event(|_, _| {
        закрой();
        gtk::glib::Propagation::Stop
    });

    // Само меню щелчки глотает: иначе нажатие на подпись внутри него
    // доходило бы до фона и закрывало всё
    let меню_ящик = gtk::EventBox::new();
    меню_ящик.set_visible_window(false);
    меню_ящик.connect_button_press_event(|_, _| gtk::glib::Propagation::Stop);
    меню_ящик.set_valign(gtk::Align::Start);
    меню_ящик.set_margin_top(6);
    match где {
        Где::Справа => {
            меню_ящик.set_halign(gtk::Align::End);
            меню_ящик.set_margin_end(8);
        }
        Где::Посередине => меню_ящик.set_halign(gtk::Align::Center),
        Где::ПодКнопкой => {
            меню_ящик.set_halign(gtk::Align::Start);
            // Полоса во всю ширину экрана, так что место кнопки в ней — это
            // и место на экране
            let x = кнопка.translate_coordinates(&кнопка.toplevel().unwrap_or_else(|| кнопка.clone().upcast()), 0, 0).map(|(x, _)| x).unwrap_or(8);
            меню_ящик.set_margin_start(x.max(8));
        }
    }

    let меню = gtk::Box::new(gtk::Orientation::Vertical, 2);
    меню.style_context().add_class("menu");
    меню.set_size_request(ширина, -1);
    меню_ящик.add(&меню);
    фон.add(&меню_ящик);
    окно.add(&фон);

    кнопка.style_context().add_class("open");
    наполни(&меню);
    окно.show_all();
    ОТКРЫТОЕ.with(|о| {
        *о.borrow_mut() = Some(Открытое { имя, окно, кнопка: кнопка.clone() })
    });
}

/// Очистить коробку меню, чтобы наполнить заново.
pub fn очисти(меню: &gtk::Box) {
    for ребёнок in меню.children() {
        меню.remove(&ребёнок);
    }
}

/// Строка-кнопка меню.
pub fn строка<F: Fn() + 'static>(меню: &gtk::Box, текст: &str, опасно: bool, дело: F) -> gtk::Button {
    let б = gtk::Button::new();
    crate::apps::знак_кнопки(&б, текст);
    б.style_context().add_class("row");
    if опасно {
        б.style_context().add_class("danger");
    }
    if let Some(п) = б.child() {
        match п.downcast::<gtk::Label>() {
            Ok(п) => п.set_xalign(0.0),
            Err(п) => п.set_halign(gtk::Align::Start),
        }
    }
    б.connect_clicked(move |_| дело());
    меню.add(&б);
    б
}

/// Подпись без действия: «Ищу сети…», «Батареи нет».
pub fn тихо(меню: &gtk::Box, текст: &str) -> gtk::Label {
    let п = gtk::Label::new(Some(текст));
    п.style_context().add_class("quiet");
    п.set_line_wrap(true);
    п.set_max_width_chars(30);
    меню.add(&п);
    п
}
