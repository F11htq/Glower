//! Калькулятор — нативный, настоящим окном (glower-calc).
//!
//! Те же кнопки и та же логика, что у страничного: число, действие,
//! число, «=». Наверху — что уже набрано, крупно — текущее.

use crate::apps::подготовь;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

const СТИЛЬ: &str = r#"
.calc .hist { font-size: 15px; opacity: 0.55; min-height: 20px; }
.calc .value { font-size: 44px; font-weight: 300; }
.calc .display { padding: 18px 20px 10px; }
.calc .keys { padding: 8px 12px 14px; }
.calc button { font-size: 17px; border-radius: 12px; min-height: 52px; padding: 0; }
.calc button.fn { background-color: rgba(255, 255, 255, 0.11); }
.calc button.op { background-color: rgba(99, 102, 241, 0.35); }
.calc button.eq { background-image: linear-gradient(140deg, #a5b4fc, #6366f1); color: #ffffff; }
"#;

/// Что набрано: текущее число строкой, прежнее и действие между ними.
#[derive(Default)]
struct Счёт {
    текущее: String,
    прежнее: Option<f64>,
    действие: Option<&'static str>,
    свежее: bool,
}

/// Число для показа: без лишних нулей, огромное и крошечное — степенью.
pub fn покажи(ч: f64) -> String {
    if !ч.is_finite() {
        return "Ошибка".into();
    }
    if ч != 0.0 && (ч.abs() >= 1e12 || ч.abs() < 1e-9) {
        return format!("{ч:.6e}");
    }
    let с = format!("{:.10}", ч);
    let с = с.trim_end_matches('0').trim_end_matches('.').to_string();
    if с == "-0" {
        "0".into()
    } else {
        с
    }
}

impl Счёт {
    fn новый() -> Счёт {
        Счёт { текущее: "0".into(), свежее: true, ..Default::default() }
    }

    fn число(&self) -> f64 {
        self.текущее.parse().unwrap_or(0.0)
    }

    fn посчитай(&self) -> String {
        let (а, б) = (self.прежнее.unwrap_or(0.0), self.число());
        покажи(match self.действие {
            Some("+") => а + б,
            Some("−") => а - б,
            Some("×") => а * б,
            Some("÷") => а / б,
            _ => б,
        })
    }

    pub fn нажми(&mut self, к: &str) {
        match к {
            ц if ц.len() == 1 && ц.chars().all(|с| с.is_ascii_digit()) => {
                if self.свежее || self.текущее == "0" {
                    self.текущее = ц.to_string();
                } else {
                    self.текущее.push_str(ц);
                }
                self.свежее = false;
            }
            "," => {
                if self.свежее {
                    self.текущее = "0.".into();
                    self.свежее = false;
                } else if !self.текущее.contains('.') {
                    self.текущее.push('.');
                }
            }
            "+" | "−" | "×" | "÷" => {
                if self.действие.is_some() && !self.свежее {
                    self.текущее = self.посчитай();
                }
                self.прежнее = Some(self.число());
                self.действие = Some(match к {
                    "+" => "+",
                    "−" => "−",
                    "×" => "×",
                    _ => "÷",
                });
                self.свежее = true;
            }
            "=" => {
                if self.действие.is_some() {
                    self.текущее = self.посчитай();
                    self.прежнее = None;
                    self.действие = None;
                    self.свежее = true;
                }
            }
            "C" => *self = Счёт::новый(),
            "⌫" => {
                if self.свежее {
                    return;
                }
                self.текущее.pop();
                if self.текущее.is_empty() || self.текущее == "-" {
                    self.текущее = "0".into();
                }
            }
            "±" => self.текущее = покажи(-self.число()),
            "%" => self.текущее = покажи(self.число() / 100.0),
            "√" => {
                self.текущее = покажи(self.число().sqrt());
                self.свежее = true;
            }
            "x²" => {
                self.текущее = покажи(self.число().powi(2));
                self.свежее = true;
            }
            "1/x" => {
                self.текущее = покажи(1.0 / self.число());
                self.свежее = true;
            }
            "π" => {
                self.текущее = покажи(std::f64::consts::PI);
                self.свежее = true;
            }
            _ => {}
        }
        if self.текущее == "Ошибка" {
            // После ошибки начинаем заново со следующей кнопки
            self.прежнее = None;
            self.действие = None;
            self.свежее = true;
        }
    }

    /// Что на экране: десятичная запятая — по-русски.
    fn экран(&self) -> (String, String) {
        let запятая = |с: &str| с.replace('.', ",");
        let верх = match (self.прежнее, self.действие) {
            (Some(п), Some(д)) => format!("{} {д}", запятая(&покажи(п))),
            _ => String::new(),
        };
        (верх, запятая(&self.текущее))
    }
}

pub fn запусти() -> ! {
    подготовь("glower-calc", "Калькулятор");
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль калькулятора");
    gtk::StyleContext::add_provider_for_screen(&gtk::gdk::Screen::default().expect("экран"), &стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_title("Калькулятор");
    окно.set_default_size(340, 500);
    окно.set_icon_name(Some("accessories-calculator"));
    окно.style_context().add_class("glower-app");
    окно.style_context().add_class("calc");

    let колонка = gtk::Box::new(gtk::Orientation::Vertical, 0);
    окно.add(&колонка);
    let экран = gtk::Box::new(gtk::Orientation::Vertical, 2);
    экран.style_context().add_class("display");
    let верх = gtk::Label::new(None);
    верх.style_context().add_class("hist");
    верх.set_xalign(1.0);
    let значение = gtk::Label::new(Some("0"));
    значение.style_context().add_class("value");
    значение.set_xalign(1.0);
    значение.set_ellipsize(gtk::pango::EllipsizeMode::Start);
    экран.add(&верх);
    экран.add(&значение);
    колонка.pack_start(&экран, false, false, 0);

    let сетка = gtk::Grid::new();
    сетка.style_context().add_class("keys");
    сетка.set_row_spacing(8);
    сетка.set_column_spacing(8);
    сетка.set_row_homogeneous(true);
    сетка.set_column_homogeneous(true);
    сетка.set_vexpand(true);
    колонка.pack_start(&сетка, true, true, 0);

    let счёт = Rc::new(RefCell::new(Счёт::новый()));
    let обнови = {
        let (счёт, верх, значение) = (счёт.clone(), верх.clone(), значение.clone());
        Rc::new(move || {
            let (в, з) = счёт.borrow().экран();
            верх.set_text(&в);
            значение.set_text(&з);
        })
    };

    let кнопки: [[(&'static str, &str); 4]; 6] = [
        [("C", "fn"), ("±", "fn"), ("%", "fn"), ("⌫", "fn")],
        [("√", "fn"), ("x²", "fn"), ("1/x", "fn"), ("÷", "op")],
        [("7", ""), ("8", ""), ("9", ""), ("×", "op")],
        [("4", ""), ("5", ""), ("6", ""), ("−", "op")],
        [("1", ""), ("2", ""), ("3", ""), ("+", "op")],
        [("π", "fn"), ("0", ""), (",", ""), ("=", "eq")],
    ];
    for (ряд, строка) in кнопки.iter().enumerate() {
        for (место, (к, вид)) in строка.iter().enumerate() {
            let б = gtk::Button::with_label(к);
            if !вид.is_empty() {
                б.style_context().add_class(вид);
            }
            б.set_can_focus(false);
            let (счёт, обнови, к) = (счёт.clone(), обнови.clone(), *к);
            б.connect_clicked(move |_| {
                счёт.borrow_mut().нажми(к);
                обнови();
            });
            сетка.attach(&б, место as i32, ряд as i32, 1, 1);
        }
    }

    // С клавиатуры — как у страничного: цифры, действия, Enter, Esc, Backspace
    {
        let (счёт, обнови) = (счёт.clone(), обнови.clone());
        окно.connect_key_press_event(move |_, е| {
            use gtk::gdk::keys::constants as к;
            let ключ = е.keyval();
            // Ctrl+C — забрать результат: выделять его мышью незачем
            if е.state().contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                if ключ.to_lower() == к::c || е.hardware_keycode() == 54 {
                    let (_, значение) = счёт.borrow().экран();
                    gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD).set_text(&значение);
                    return glib::Propagation::Stop;
                }
                return glib::Propagation::Proceed;
            }
            let нажатие: Option<String> = if ключ == к::Return || ключ == к::KP_Enter || ключ == к::equal || ключ == к::KP_Equal {
                Some("=".into())
            } else if ключ == к::BackSpace {
                Some("⌫".into())
            } else if ключ == к::Escape || ключ == к::Delete {
                Some("C".into())
            } else {
                ключ.to_unicode().and_then(|с| match с {
                    '0'..='9' => Some(с.to_string()),
                    '+' => Some("+".into()),
                    '-' => Some("−".into()),
                    '*' => Some("×".into()),
                    '/' => Some("÷".into()),
                    '.' | ',' => Some(",".into()),
                    '%' => Some("%".into()),
                    _ => None,
                })
            };
            match нажатие {
                Some(н) => {
                    счёт.borrow_mut().нажми(&н);
                    обнови();
                    glib::Propagation::Stop
                }
                None => glib::Propagation::Proceed,
            }
        });
    }
    окно.connect_delete_event(|_, _| {
        gtk::main_quit();
        glib::Propagation::Proceed
    });
    окно.show_all();
    gtk::main();
    std::process::exit(0);
}

#[cfg(test)]
mod проверки {
    use super::{покажи, Счёт};

    fn набери(клавиши: &[&str]) -> String {
        let mut с = Счёт::новый();
        for к in клавиши {
            с.нажми(к);
        }
        с.экран().1
    }

    #[test]
    fn считает_как_страничный() {
        assert_eq!(набери(&["1", "2", "+", "3", "="]), "15");
        assert_eq!(набери(&["7", "÷", "2", "="]), "3,5");
        assert_eq!(набери(&["2", "×", "3", "+", "4", "="]), "10");
        assert_eq!(набери(&["1", "÷", "0", "="]), "Ошибка");
        assert_eq!(набери(&["0", ",", "1", "+", "0", ",", "2", "="]), "0,3");
        assert_eq!(набери(&["9", "√"]), "3");
        assert_eq!(набери(&["1", "2", "3", "⌫"]), "12");
        assert_eq!(набери(&["5", "±"]), "-5");
    }

    #[test]
    fn большие_и_малые_числа() {
        assert_eq!(покажи(1e15), "1.000000e15");
        assert_eq!(покажи(0.5), "0.5");
        assert_eq!(покажи(-0.0), "0");
    }
}
