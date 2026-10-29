//! Подсказка посреди экрана при смене рабочего стола: «Рабочий стол 2» и
//! точки по числу столов, как в Windows 11. Появляется на мгновение и
//! гаснет сама; щелчки проходят сквозь неё.

use gtk::glib;
use gtk::prelude::*;
use gtk_layer_shell::{KeyboardMode, Layer, LayerShell};
use std::cell::RefCell;
use std::time::Instant;

pub const СТИЛЬ: &str = r#"
window.osd { background-color: transparent; }
.osd-card {
  background-color: alpha(@glower_bg, 0.90);
  border-radius: 22px;
  padding: 20px 40px 16px 40px;
  box-shadow: inset 0 0 0 1px alpha(@glower_fg, 0.12), 0 16px 44px rgba(0, 0, 0, 0.50);
}
.osd-card label { color: @glower_fg; font-size: 20px; font-weight: 600; }
.osd-dot { min-width: 10px; min-height: 10px; border-radius: 5px; background-color: alpha(@glower_fg, 0.30); }
.osd-dot.on { min-width: 28px; background-image: linear-gradient(90deg, @glower_accent, @glower_accent2); }
"#;

struct Подсказка {
    окно: gtk::Window,
    надпись: gtk::Label,
    точки: gtk::Box,
    /// Когда показали — по нему и гаснет; новое переключение продлевает
    когда: Instant,
}

thread_local! {
    static П: RefCell<Option<Подсказка>> = const { RefCell::new(None) };
}

const ДЕРЖАТЬ: f64 = 0.9;
const ГАСНУТЬ: f64 = 0.25;

fn создай() -> Подсказка {
    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_decorated(false);
    окно.set_app_paintable(true);
    if let Some(в) = gtk::prelude::GtkWindowExt::screen(&окно).and_then(|э| э.rgba_visual()) {
        окно.set_visual(Some(&в));
    }
    окно.style_context().add_class("osd");
    окно.init_layer_shell();
    окно.set_layer(Layer::Overlay);
    окно.set_namespace("glower-osd");
    окно.set_keyboard_mode(KeyboardMode::None);
    окно.set_exclusive_zone(-1);
    окно.connect_draw(|_, к| {
        к.set_operator(gtk::cairo::Operator::Source);
        к.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        let _ = к.paint();
        glib::Propagation::Proceed
    });
    // Сквозь подсказку щёлкают по тому, что под ней
    окно.connect_realize(|о| {
        if let Some(г) = о.window() {
            г.input_shape_combine_region(&gtk::cairo::Region::create(), 0, 0);
        }
    });
    let карточка = gtk::Box::new(gtk::Orientation::Vertical, 12);
    карточка.style_context().add_class("osd-card");
    let надпись = gtk::Label::new(None);
    карточка.add(&надпись);
    let точки = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    точки.set_halign(gtk::Align::Center);
    карточка.add(&точки);
    окно.add(&карточка);
    Подсказка { окно, надпись, точки, когда: Instant::now() }
}

/// Показать «Рабочий стол n+1 из всего»
pub fn стол(n: u64, всего: u64) {
    let новая = П.with(|п| п.borrow().is_none());
    if новая {
        П.with(|п| *п.borrow_mut() = Some(создай()));
    }
    П.with(|п| {
        let mut п = п.borrow_mut();
        let Some(п) = п.as_mut() else { return };
        п.надпись.set_text(&format!("{} {}", crate::i18n::tr("Рабочий стол"), n + 1));
        for р in п.точки.children() {
            п.точки.remove(&р);
        }
        for i in 0..всего.max(1) {
            let т = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            т.style_context().add_class("osd-dot");
            if i == n {
                т.style_context().add_class("on");
            }
            т.set_valign(gtk::Align::Center);
            п.точки.add(&т);
        }
        п.когда = Instant::now();
        п.окно.set_opacity(1.0);
        п.окно.show_all();
    });
    if новая {
        // Один ход на всё время жизни: гасит, когда подсказку давно не продлевали
        if let Some(о) = П.with(|п| п.borrow().as_ref().map(|п| п.окно.clone())) {
            о.add_tick_callback(|о, _| {
                let прошло = П.with(|п| п.borrow().as_ref().map(|п| п.когда.elapsed().as_secs_f64())).unwrap_or(9.0);
                if прошло < ДЕРЖАТЬ {
                    о.set_opacity(1.0);
                } else if прошло < ДЕРЖАТЬ + ГАСНУТЬ {
                    о.set_opacity(1.0 - (прошло - ДЕРЖАТЬ) / ГАСНУТЬ);
                } else if о.is_visible() {
                    о.hide();
                }
                glib::ControlFlow::Continue
            });
        }
    }
}
