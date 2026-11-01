//! Заставка между загрузкой и рабочим столом — как в Windows.
//!
//! Без неё путь от включения до стола шёл через чёрные экраны: заставка
//! загрузки гасла, оконный сервер поднимался, панель грузилась. Теперь
//! тот же вид, что у заставки загрузки (Plymouth: те же цвета, логотип и
//! надпись на тех же местах — переход незаметен), а под ними пять точек
//! бегут по кругу, как в Windows, и подпись.
//!
//!   glower-splash          — после входа: «Добро пожаловать», поверх всего;
//!                            гаснет, когда рабочий стол нарисован
//!                            ($XDG_RUNTIME_DIR/glower-desktop-ready);
//!   glower-splash --вход   — под экраном входа, пока тот загружается.
//! Сама уходит через 40 секунд, что бы ни случилось.

use gtk::glib;
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::Cell;
use std::f64::consts::PI;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub fn файл_готовности() -> std::path::PathBuf {
    glib::user_runtime_dir().join("glower-desktop-ready")
}

/// Рабочий стол нарисован — заставке пора
pub fn стол_готов() {
    let _ = std::fs::write(файл_готовности(), b"1");
}

/// Положение точки бегущего круга: как у Windows — разгон снизу, плавно
/// через верх, у низа сбавляют и собираются вместе
pub fn угол_точки(t: f64, i: usize) -> Option<f64> {
    let круг = 2.4;
    let p = ((t - i as f64 * 0.11) / круг).rem_euclid(1.0);
    // Последняя восьмая круга — точки не видны (пауза между кругами)
    if p > 0.88 {
        return None;
    }
    let q = p / 0.88;
    // Скорость меняется по синусу: медленно у низа, быстро сверху
    let путь = q - (2.0 * PI * q).sin() / (2.0 * PI) * 0.75;
    Some(PI / 2.0 + путь * 2.0 * PI * 1.25)
}

pub fn запусти(доводы: Vec<String>) -> ! {
    if gtk::init().is_err() {
        std::process::exit(1);
    }
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

    let знак = crate::apps::картинка_на_тёмном("logo-256.png", 256, 256);
    let слово = crate::apps::картинка_на_тёмном("logo-word.png", 600, 200).or_else(|| crate::apps::картинка_системы("logo-word.png", 600, 200));
    let подпись = if вход { String::new() } else { crate::i18n::tr("Добро пожаловать") };
    let начало = Instant::now();
    let гаснет: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));

    {
        let гаснет = гаснет.clone();
        окно.connect_draw(move |о, к| {
            let (w, h) = (о.allocated_width() as f64, о.allocated_height() as f64);
            let t = начало.elapsed().as_secs_f64();
            let видно = match гаснет.get() {
                Some(с) => (1.0 - с.elapsed().as_secs_f64() / 0.35).max(0.0),
                None => 1.0,
            };
            к.set_operator(gtk::cairo::Operator::Source);
            к.set_source_rgba(0.0, 0.0, 0.0, 0.0);
            let _ = к.paint();
            к.set_operator(gtk::cairo::Operator::Over);
            let _ = к.push_group();
            // Фон — как у заставки загрузки
            let г = gtk::cairo::LinearGradient::new(0.0, 0.0, 0.0, h);
            г.add_color_stop_rgb(0.0, 0.035, 0.04, 0.085);
            г.add_color_stop_rgb(1.0, 0.01, 0.01, 0.03);
            let _ = к.set_source(&г);
            let _ = к.paint();
            // Логотип и надпись — там же, где у Plymouth
            let cx = w / 2.0;
            let cy = h / 2.0 - h / 22.0;
            let размер = (h / 5.2).floor();
            if let Some(з) = &знак {
                if let Some(м) = з.scale_simple(размер as i32, размер as i32, gtk::gdk_pixbuf::InterpType::Bilinear) {
                    к.set_source_pixbuf(&м, cx - размер / 2.0, cy - размер / 2.0);
                    let _ = к.paint();
                }
            }
            let mut низ = cy + размер / 2.0 + размер * 0.22;
            if let Some(с) = &слово {
                let ш = (размер * 1.35).floor();
                let в = (ш * с.height() as f64 / с.width().max(1) as f64).floor();
                if let Some(м) = с.scale_simple(ш as i32, в as i32, gtk::gdk_pixbuf::InterpType::Bilinear) {
                    к.set_source_pixbuf(&м, cx - ш / 2.0, низ);
                    let _ = к.paint();
                }
                низ += в;
            }
            // Точки по кругу
            let r = (размер * 0.16).max(14.0);
            let центр_y = низ + размер * 0.32 + r;
            let точка = (r * 0.13).max(2.2);
            for i in 0..5 {
                if let Some(а) = угол_точки(t, i) {
                    к.set_source_rgba(1.0, 1.0, 1.0, 0.92);
                    к.arc(cx + r * а.cos(), центр_y + r * а.sin(), точка, 0.0, 2.0 * PI);
                    let _ = к.fill();
                }
            }
            if !подпись.is_empty() {
                к.select_font_face("Sans", gtk::cairo::FontSlant::Normal, gtk::cairo::FontWeight::Normal);
                к.set_font_size((размер / 8.5).max(14.0));
                let ш = к.text_extents(&подпись).map(|р| р.x_advance()).unwrap_or(0.0);
                к.set_source_rgba(1.0, 1.0, 1.0, 0.82);
                к.move_to(cx - ш / 2.0, центр_y + r + размер * 0.18 + (размер / 8.5).max(14.0));
                let _ = к.show_text(&подпись);
            }
            let _ = к.pop_group_to_source();
            let _ = к.paint_with_alpha(видно);
            glib::Propagation::Stop
        });
    }

    // 30 кадров в секунду; стол готов — полсекунды на то, чтобы погаснуть
    {
        let окно = окно.clone();
        let гаснет = гаснет.clone();
        glib::timeout_add_local(Duration::from_millis(33), move || {
            if гаснет.get().is_none() && (!вход && файл_готовности().exists() || начало.elapsed() > Duration::from_secs(40)) {
                гаснет.set(Some(Instant::now()));
            }
            if гаснет.get().is_some_and(|с| с.elapsed() > Duration::from_millis(380)) {
                std::process::exit(0);
            }
            окно.queue_draw();
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
    fn точки_бегут_и_исчезают_между_кругами() {
        // В начале круга видны все пять, к концу круга — ни одной
        assert!((0..5).all(|i| super::угол_точки(0.6, i).is_some()));
        assert!((0..5).all(|i| super::угол_точки(2.35 + i as f64 * 0.11, i).is_none()));
        // Первая точка впереди второй
        let (а, б) = (super::угол_точки(0.5, 0).unwrap(), super::угол_точки(0.5, 1).unwrap());
        assert!(а > б);
    }
}
