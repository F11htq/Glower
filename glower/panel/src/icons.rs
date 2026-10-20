//! Значки полосы — те же, что у страничной панели, только нарисованные
//! прямо линиями: ни картинок, ни темы значков искать не надо.

use gtk::cairo::{Context, LineCap, LineJoin};
use gtk::prelude::*;
use std::cell::Cell;
use std::f64::consts::PI;
use std::rc::Rc;

/// Вид значка. Рисуем в сетке 24×24, как SVG страничной панели.
#[derive(Clone, Copy)]
pub enum Вид {
    Сетка,
    Ползунки,
    Питание,
    Батарея,
}

/// Что известно о заряде: уровень 0–100 (None — батареи нет) и заряжается ли.
#[derive(Clone, Copy, Default)]
pub struct Заряд {
    pub уровень: Option<u8>,
    pub заряжается: bool,
}

pub struct Значок {
    pub холст: gtk::DrawingArea,
    pub заряд: Rc<Cell<Заряд>>,
}

impl Значок {
    pub fn новый(вид: Вид) -> Значок {
        let холст = gtk::DrawingArea::new();
        холст.set_size_request(15, 15);
        холст.set_valign(gtk::Align::Center);
        let заряд = Rc::new(Cell::new(Заряд::default()));
        let з = заряд.clone();
        холст.connect_draw(move |в, к| {
            let цвет = в.style_context().color(gtk::StateFlags::NORMAL);
            let ш = в.allocated_width() as f64;
            let в_ = в.allocated_height() as f64;
            let м = ш.min(в_) / 24.0;
            к.translate((ш - 24.0 * м) / 2.0, (в_ - 24.0 * м) / 2.0);
            к.scale(м, м);
            к.set_source_rgba(цвет.red(), цвет.green(), цвет.blue(), цвет.alpha());
            к.set_line_width(1.9);
            к.set_line_cap(LineCap::Round);
            к.set_line_join(LineJoin::Round);
            нарисуй(к, вид, з.get());
            glib_proceed()
        });
        Значок { холст, заряд }
    }

    pub fn задай_заряд(&self, з: Заряд) {
        self.заряд.set(з);
        self.холст.queue_draw();
    }
}

fn glib_proceed() -> gtk::glib::Propagation {
    gtk::glib::Propagation::Proceed
}

fn скруглённый(к: &Context, лево: f64, верх: f64, ш: f64, в: f64, р: f64) {
    let (x, y_) = (лево, верх);
    к.new_sub_path();
    к.arc(x + ш - р, y_ + р, р, -PI / 2.0, 0.0);
    к.arc(x + ш - р, y_ + в - р, р, 0.0, PI / 2.0);
    к.arc(x + р, y_ + в - р, р, PI / 2.0, PI);
    к.arc(x + р, y_ + р, р, PI, 1.5 * PI);
    к.close_path();
}

fn нарисуй(к: &Context, вид: Вид, заряд: Заряд) {
    match вид {
        Вид::Сетка => {
            for (лево, верх) in [(3.0, 3.0), (14.0, 3.0), (3.0, 14.0), (14.0, 14.0)] {
                скруглённый(к, лево, верх, 7.0, 7.0, 1.5);
            }
            let _ = к.fill();
        }
        Вид::Ползунки => {
            к.move_to(4.0, 7.0);
            к.line_to(14.0, 7.0);
            к.move_to(18.0, 7.0);
            к.line_to(20.0, 7.0);
            к.move_to(4.0, 17.0);
            к.line_to(8.0, 17.0);
            к.move_to(12.0, 17.0);
            к.line_to(20.0, 17.0);
            let _ = к.stroke();
            к.arc(16.0, 7.0, 2.0, 0.0, 2.0 * PI);
            let _ = к.stroke();
            к.arc(10.0, 17.0, 2.0, 0.0, 2.0 * PI);
            let _ = к.stroke();
        }
        Вид::Питание => {
            к.move_to(12.0, 3.0);
            к.line_to(12.0, 12.0);
            let _ = к.stroke();
            // Та же дуга, что в SVG «M6.5 6.5 a8 8 0 1 0 11 0»: радиус 8,
            // центр ниже хорды, обход снизу — от 226.6° до −46.6°
            к.arc_negative(12.0, 12.31, 8.0, 226.6_f64.to_radians(), (-46.6_f64).to_radians());
            let _ = к.stroke();
        }
        Вид::Батарея => {
            скруглённый(к, 2.0, 7.0, 17.0, 10.0, 2.5);
            let _ = к.stroke();
            к.move_to(21.5, 11.0);
            к.line_to(21.5, 13.0);
            let _ = к.stroke();
            // Заливка — от нуля до десяти единиц: столько места внутри корпуса
            let ширина = match заряд.уровень {
                Some(у) => ((у as f64) / 10.0).round().clamp(0.0, 10.0),
                None => 10.0,
            };
            if ширина > 0.0 {
                if заряд.уровень.is_some_and(|у| у <= 15) && !заряд.заряжается {
                    к.set_source_rgb(0.96, 0.36, 0.36);
                }
                скруглённый(к, 4.5, 9.5, ширина.max(1.5), 5.0, 1.0);
                let _ = к.fill();
            }
            if заряд.заряжается {
                // Молния поверх корпуса
                к.set_source_rgb(1.0, 0.85, 0.3);
                к.move_to(11.5, 8.0);
                к.line_to(8.5, 12.5);
                к.line_to(11.0, 12.5);
                к.line_to(9.5, 16.0);
                к.line_to(13.0, 11.3);
                к.line_to(10.5, 11.3);
                к.close_path();
                let _ = к.fill();
            }
        }
    }
}
