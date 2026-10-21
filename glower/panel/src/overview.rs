//! Меню программ во весь экран — нативное, в той же программе, что панель.
//!
//! Прежде меню рисовала страница рабочего стола. Здесь — то же меню:
//! рабочие столы наверху, поиск, открытые окна системы и все программы
//! разом. Что лежит на столах и в окнах, знает только страница: мы
//! спрашиваем её по шине («обзор-дай») и ей же отдаём то, что сделал
//! человек («обзор-дело»). Список программ стол и так рассказывает всем
//! («программы»).

use crate::agent::скажи;
use crate::popup;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::rc::Rc;

struct Меню {
    окно: gtk::Window,
    поиск: gtk::Entry,
    столы: gtk::Box,
    окна: gtk::Box,
    сетка: gtk::FlowBox,
    листание: gtk::ScrolledWindow,
    программы: Vec<Value>,
    слепок_программ: String,
    данные: Value,
}

thread_local! {
    static МЕНЮ: RefCell<Option<Rc<RefCell<Меню>>>> = const { RefCell::new(None) };
    static СПИСОК: RefCell<Vec<Value>> = const { RefCell::new(Vec::new()) };
    static ДАННЫЕ: RefCell<Value> = const { RefCell::new(Value::Null) };
}

/// Стол рассказал о своих программах.
pub fn программы(список: &Value) {
    СПИСОК.with(|с| *с.borrow_mut() = список.as_array().cloned().unwrap_or_default());
    if let Some(м) = меню_если_есть() {
        if м.borrow().окно.is_visible() {
            наполни_сетку(&м);
        }
    }
}

/// Стол ответил, что у него на столах и в окнах.
pub fn данные(что: &Value) {
    ДАННЫЕ.with(|д| *д.borrow_mut() = что.clone());
    if let Some(м) = меню_если_есть() {
        м.borrow_mut().данные = что.clone();
        if м.borrow().окно.is_visible() {
            наполни_столы(&м);
        }
    }
}

fn меню_если_есть() -> Option<Rc<RefCell<Меню>>> {
    МЕНЮ.with(|м| м.borrow().clone())
}

pub fn открыто() -> bool {
    меню_если_есть().is_some_and(|м| м.borrow().окно.is_visible())
}

pub fn закрой() {
    if let Some(м) = меню_если_есть() {
        м.borrow().окно.hide();
    }
}

/// «Переключи» — от клавиши Win и кнопки «Программы»: второе нажатие
/// закрывает. true / false — открыть или закрыть явно.
pub fn переключи(что: &Value) {
    let открыть = match что {
        Value::Bool(б) => *б,
        _ => !открыто(),
    };
    if !открыть {
        закрой();
        return;
    }
    popup::закрой();
    let м = меню_если_есть().unwrap_or_else(создай);
    {
        let м_ = м.borrow();
        м_.поиск.set_text("");
        м_.листание.vadjustment().set_value(0.0);
    }
    м.borrow_mut().данные = ДАННЫЕ.with(|д| д.borrow().clone());
    наполни_сетку(&м);
    наполни_столы(&м);
    // Свежее о столах и окнах — у стола: ответ придёт следом и дорисует
    скажи("обзор-дай", json!(true));
    let окно = м.borrow().окно.clone();
    окно.show_all();
    м.borrow().поиск.grab_focus();
}

fn дело(что: &str, доводы: Value) {
    let mut тело = доводы;
    тело["что"] = json!(что);
    скажи("обзор-дело", тело);
}

// ------------------------------------------------------------- окно меню

fn создай() -> Rc<RefCell<Меню>> {
    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_decorated(false);
    окно.set_app_paintable(true);
    if let Some(в) = gtk::prelude::GtkWindowExt::screen(&окно).and_then(|э| э.rgba_visual()) {
        окно.set_visual(Some(&в));
    }
    окно.style_context().add_class("overview");
    окно.init_layer_shell();
    окно.set_layer(Layer::Top);
    окно.set_namespace("glower-overview");
    for край in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        окно.set_anchor(край, true);
    }
    // Ноль: меню ложится под панелью, а сама панель остаётся видна и
    // нажимаема — «Программы» закрывает меню тем же нажатием
    окно.set_exclusive_zone(0);
    окно.set_keyboard_mode(KeyboardMode::Exclusive);
    окно.connect_key_press_event(|_, с| {
        if с.keyval() == gtk::gdk::keys::constants::Escape {
            закрой();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });

    // Щелчок по пустому месту закрывает — как в GNOME
    let фон = gtk::EventBox::new();
    фон.set_visible_window(false);
    фон.connect_button_press_event(|_, с| {
        if с.button() == 1 {
            закрой();
        }
        glib::Propagation::Stop
    });

    let колонка = gtk::Box::new(gtk::Orientation::Vertical, 16);
    колонка.set_margin_top(22);
    колонка.set_margin_bottom(18);
    колонка.set_margin_start(40);
    колонка.set_margin_end(40);

    let столы = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    столы.set_halign(gtk::Align::Center);

    let поиск = gtk::Entry::new();
    поиск.set_placeholder_text(Some("Поиск программ"));
    поиск.set_width_chars(36);
    поиск.set_halign(gtk::Align::Center);
    поиск.style_context().add_class("search");

    let окна = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    окна.set_halign(gtk::Align::Center);

    let сетка = gtk::FlowBox::new();
    сетка.set_selection_mode(gtk::SelectionMode::None);
    сетка.set_homogeneous(true);
    сетка.set_max_children_per_line(9);
    сетка.set_min_children_per_line(3);
    сетка.set_row_spacing(6);
    сетка.set_column_spacing(6);
    сетка.set_valign(gtk::Align::Start);
    сетка.set_halign(gtk::Align::Center);

    let листание = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    листание.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    листание.set_vexpand(true);
    листание.add(&сетка);

    колонка.pack_start(&столы, false, false, 0);
    колонка.pack_start(&поиск, false, false, 0);
    колонка.pack_start(&окна, false, false, 0);
    колонка.pack_start(&листание, true, true, 0);
    фон.add(&колонка);
    окно.add(&фон);

    // Поиск: фильтр по имени, Ввод открывает первое найденное
    let строка = Rc::new(RefCell::new(String::new()));
    {
        let строка = строка.clone();
        сетка.set_filter_func(Some(Box::new(move |ребёнок| {
            let что = строка.borrow();
            что.is_empty() || ребёнок.widget_name().as_str().contains(что.as_str())
        })));
    }
    {
        let (строка, сетка) = (строка.clone(), сетка.clone());
        поиск.connect_changed(move |п| {
            *строка.borrow_mut() = п.text().trim().to_lowercase();
            сетка.invalidate_filter();
        });
    }
    {
        let сетка = сетка.clone();
        поиск.connect_activate(move |_| {
            let первый = сетка
                .children()
                .into_iter()
                .filter_map(|р| р.downcast::<gtk::FlowBoxChild>().ok())
                // Отсеянного поиском GTK не прячет, а лишь снимает с показа
                .find(|р| р.is_child_visible());
            if let Some(р) = первый {
                if let Some(б) = р.child().and_then(|б| б.downcast::<gtk::Button>().ok()) {
                    б.clicked();
                }
            }
        });
    }

    let м = Rc::new(RefCell::new(Меню {
        окно,
        поиск,
        столы,
        окна,
        сетка,
        листание,
        программы: Vec::new(),
        слепок_программ: String::new(),
        данные: Value::Null,
    }));
    МЕНЮ.with(|я| *я.borrow_mut() = Some(м.clone()));
    м
}


// ------------------------------------------------------------- значки

/// Значок нашей программы: знак на цветной плашке — тот же, что у
/// страницы. Фон приходит готовой строкой CSS, её и отдаём GTK.
fn наш_значок(знак: &str, фон: &str) -> gtk::Widget {
    let з = gtk::Label::new(Some(if знак.is_empty() { "▢" } else { знак }));
    з.style_context().add_class("glyph");
    let стиль = gtk::CssProvider::new();
    let css = if фон.trim_start().starts_with("linear-gradient") || фон.trim_start().starts_with("radial-gradient") {
        format!("label {{ background-image: {фон}; }}")
    } else if !фон.is_empty() {
        format!("label {{ background-color: {фон}; }}")
    } else {
        String::new()
    };
    if !css.is_empty() && стиль.load_from_data(css.as_bytes()).is_ok() {
        з.style_context().add_provider(&стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
    }
    з.upcast()
}

/// Значок программы машины — её собственный, из темы значков системы.
fn значок_машины(id: &str, знак: &str) -> gtk::Widget {
    if let Some(ярлык) = gio::DesktopAppInfo::new(id) {
        if let Some(з) = ярлык.icon() {
            let к = gtk::Image::from_gicon(&з, gtk::IconSize::Dialog);
            к.set_pixel_size(52);
            return к.upcast();
        }
    }
    наш_значок(if знак.is_empty() { "🐧" } else { знак }, "rgba(255,255,255,0.12)")
}

// ------------------------------------------------------------- перетаскивание
//
// Программу из сетки можно бросить на рабочий стол наверху — она там и
// откроется; на «+» — заведётся новый стол, и откроется на нём. Открытое
// окно переезжает на другой стол так же. Так это устроено в GNOME, и так
// было в страничном меню: открыть программу не там, где стоишь, — одно
// движение вместо трёх (уйти на стол, открыть, вернуться).

const ГРУЗ: &str = "application/x-glower-drag";

fn цель() -> gtk::TargetEntry {
    // Только внутри своей программы: чужим окнам наш груз ни к чему
    gtk::TargetEntry::new(ГРУЗ, gtk::TargetFlags::SAME_APP, 0)
}

/// Сделать виджет источником груза. `картинка` — что тянется за курсором:
/// значок программы, а не безликий значок GTK.
fn тащи(кто: &gtk::Button, груз: Value, картинка: &gtk::Widget) {
    кто.drag_source_set(gtk::gdk::ModifierType::BUTTON1_MASK, &[цель()], gtk::gdk::DragAction::MOVE);
    let картинка = картинка.clone();
    кто.connect_drag_begin(move |_, ход| {
        let (ш, в) = (картинка.allocated_width(), картинка.allocated_height());
        if ш > 1 && в > 1 {
            if let Ok(холст) = gtk::cairo::ImageSurface::create(gtk::cairo::Format::ARgb32, ш, в) {
                if let Ok(кисть) = gtk::cairo::Context::new(&холст) {
                    картинка.draw(&кисть);
                }
                холст.set_device_offset(-(ш as f64) / 2.0, -(в as f64) / 2.0);
                ход.drag_set_icon_surface(&холст);
            }
        }
    });
    let строка = груз.to_string();
    кто.connect_drag_data_get(move |_, _, данные, _, _| {
        данные.set(&данные.target(), 8, строка.as_bytes());
    });
}

/// Сделать виджет местом, куда бросают. `дело` получает груз.
fn прими<F: Fn(Value) + 'static>(куда: &gtk::Button, дело_: F) {
    куда.drag_dest_set(gtk::DestDefaults::ALL, &[цель()], gtk::gdk::DragAction::MOVE);
    // Стол под курсором подсвечиваем — видно, куда упадёт
    куда.connect_drag_motion(|к, _, _, _, _| {
        к.style_context().add_class("drop");
        false
    });
    куда.connect_drag_leave(|к, _, _| к.style_context().remove_class("drop"));
    куда.connect_drag_data_received(move |к, _, _, _, данные, _, _| {
        к.style_context().remove_class("drop");
        if let Ok(груз) = serde_json::from_slice::<Value>(&данные.data()) {
            дело_(груз);
        }
    });
}

/// Что сделать, когда груз упал на стол `n` (None — на «+», новый стол).
fn брошено(груз: Value, n: Option<u64>) {
    if let Some(окно) = груз["окно"].as_str() {
        // Окно переезжает, меню остаётся: человек видит, что оно уехало
        match n {
            Some(n) => дело("окно-на-стол", json!({ "id": окно, "n": n })),
            None => дело("окно-на-новый-стол", json!({ "id": окно })),
        }
        return;
    }
    let (id, вид, имя) = (груз["id"].clone(), груз["вид"].clone(), груз["имя"].clone());
    закрой();
    match n {
        Some(n) => дело("на-стол", json!({ "id": id, "вид": вид, "имя": имя, "n": n })),
        None => дело("на-новый-стол", json!({ "id": id, "вид": вид, "имя": имя })),
    }
}

// ------------------------------------------------------------- сетка

fn наполни_сетку(м: &Rc<RefCell<Меню>>) {
    let список = СПИСОК.with(|с| с.borrow().clone());
    let слепок = serde_json::to_string(&список).unwrap_or_default();
    if м.borrow().слепок_программ == слепок {
        м.borrow().сетка.invalidate_filter();
        return;
    }
    let сетка = м.borrow().сетка.clone();
    for р in сетка.children() {
        сетка.remove(&р);
    }
    for п in &список {
        let id = п["id"].as_str().unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let имя = п["имя"].as_str().unwrap_or(&id).to_string();
        let вид = п["вид"].as_str().unwrap_or("приложение").to_string();
        let знак = п["знак"].as_str().unwrap_or("");
        let фон = п["фон"].as_str().unwrap_or("");

        let б = gtk::Button::new();
        б.style_context().add_class("app");
        let ящик = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let значок = if вид == "машина" { значок_машины(&id, знак) } else { наш_значок(знак, фон) };
        значок.set_halign(gtk::Align::Center);
        ящик.add(&значок);
        let подпись = gtk::Label::new(Some(&имя));
        подпись.style_context().add_class("app-name");
        подпись.set_max_width_chars(12);
        подпись.set_ellipsize(gtk::pango::EllipsizeMode::End);
        подпись.set_justify(gtk::Justification::Center);
        ящик.add(&подпись);
        б.add(&ящик);
        б.set_tooltip_text(Some(&имя));
        тащи(&б, json!({ "id": id, "вид": вид, "имя": имя }), &значок);

        {
            let (id, вид) = (id.clone(), вид.clone());
            б.connect_clicked(move |_| {
                закрой();
                дело("запусти", json!({ "id": id, "вид": вид }));
            });
        }
        {
            let (id, вид, имя) = (id.clone(), вид.clone(), имя.clone());
            б.connect_button_press_event(move |к, с| {
                if с.button() == 3 {
                    меню_программы(к, &id, &вид, &имя, с);
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            });
        }

        let ребёнок = gtk::FlowBoxChild::new();
        ребёнок.set_widget_name(&имя.to_lowercase());
        ребёнок.add(&б);
        сетка.add(&ребёнок);
    }
    if список.is_empty() {
        let п = gtk::Label::new(Some("Список программ ещё не пришёл"));
        п.style_context().add_class("empty");
        сетка.add(&п);
    }
    let mut м_ = м.borrow_mut();
    м_.программы = список;
    м_.слепок_программ = слепок;
    сетка.show_all();
}

/// Меню программы по правой кнопке — те же пункты, что у страничного.
fn меню_программы(кнопка: &gtk::Button, id: &str, вид: &str, имя: &str, событие: &gtk::gdk::EventButton) {
    let машина = вид == "машина";
    let данные = ДАННЫЕ.with(|д| д.borrow().clone());
    let есть = |поле: &str| данные[поле].as_array().is_some_and(|с| с.iter().any(|x| x.as_str() == Some(id)));

    let меню = gtk::Menu::new();
    let пункт = |текст: &str, что: &'static str, закрыть: bool| {
        let п = gtk::MenuItem::with_label(текст);
        let доводы = json!({ "id": id, "вид": вид, "имя": имя });
        п.connect_activate(move |_| {
            if закрыть {
                закрой();
            }
            дело(что, доводы.clone());
        });
        меню.append(&п);
    };
    пункт("Открыть", "запусти", true);
    if !машина || данные["столы"].as_u64().unwrap_or(1) > 1 {
        пункт("Открыть на новом рабочем столе", "на-новый-стол", true);
    }
    меню.append(&gtk::SeparatorMenuItem::new());
    if машина {
        пункт(if есть("на-столе") { "Убрать с рабочего стола" } else { "Вынести на рабочий стол" }, "вынеси", false);
    } else {
        пункт("Вынести на рабочий стол", "вынеси", false);
        пункт(if есть("в-доке") { "Убрать из дока" } else { "Закрепить в доке" }, "док", false);
    }
    if машина {
        пункт("Найти в Магазине", "магазин", true);
        меню.append(&gtk::SeparatorMenuItem::new());
        пункт("Удалить", "удали", true);
    }
    меню.show_all();
    let событие: &gtk::gdk::Event = событие;
    меню.popup_at_pointer(Some(событие));
    let _ = кнопка;
}

// ------------------------------------------------------------- столы и окна

fn наполни_столы(м: &Rc<RefCell<Меню>>) {
    let (столы, окна, д) = {
        let м_ = м.borrow();
        (м_.столы.clone(), м_.окна.clone(), м_.данные.clone())
    };
    for р in столы.children() {
        столы.remove(&р);
    }
    for р in окна.children() {
        окна.remove(&р);
    }
    let сколько = д["столы"].as_u64().unwrap_or(1).max(1);
    let текущий = д["текущий"].as_u64().unwrap_or(0);
    let на_столах: Vec<u64> = д["на-столах"]
        .as_array()
        .map(|с| с.iter().map(|x| x.as_u64().unwrap_or(0)).collect())
        .unwrap_or_default();

    for i in 0..сколько {
        let окон = на_столах.get(i as usize).copied().unwrap_or(0);
        let стол = gtk::Overlay::new();
        let б = gtk::Button::with_label(&if окон > 0 { format!("{} · {}", i + 1, окон) } else { format!("{}", i + 1) });
        б.style_context().add_class("desk");
        if i == текущий {
            б.style_context().add_class("on");
        }
        б.connect_clicked(move |_| дело("стол", json!({ "n": i })));
        прими(&б, move |груз| брошено(груз, Some(i)));
        стол.add(&б);
        // Пустой стол можно убрать: иначе заведённые столы только копятся
        if окон == 0 && сколько > 1 {
            let крестик = gtk::Button::with_label("×");
            крестик.style_context().add_class("desk-x");
            крестик.set_halign(gtk::Align::End);
            крестик.set_valign(gtk::Align::Start);
            крестик.set_tooltip_text(Some("Убрать этот рабочий стол"));
            крестик.connect_clicked(move |_| дело("стол-убери", json!({ "n": i })));
            стол.add_overlay(&крестик);
        }
        столы.add(&стол);
    }
    if д["можно-ещё"].as_bool() != Some(false) && сколько < 8 {
        let плюс = gtk::Button::with_label("+");
        плюс.style_context().add_class("desk");
        плюс.style_context().add_class("desk-plus");
        плюс.set_tooltip_text(Some("Новый рабочий стол"));
        плюс.connect_clicked(|_| дело("стол-добавь", json!({})));
        прими(&плюс, |груз| брошено(груз, None));
        столы.add(&плюс);
    }

    for о in д["окна"].as_array().cloned().unwrap_or_default() {
        let id = о["id"].as_str().unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let б = gtk::Button::new();
        б.style_context().add_class("win");
        let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let з = наш_значок(о["знак"].as_str().unwrap_or(""), о["фон"].as_str().unwrap_or(""));
        з.style_context().add_class("small");
        ряд.add(&з);
        let имя = gtk::Label::new(о["имя"].as_str());
        имя.set_max_width_chars(22);
        имя.set_ellipsize(gtk::pango::EllipsizeMode::End);
        ряд.add(&имя);
        б.add(&ряд);
        тащи(&б, json!({ "окно": id }), &з);
        б.connect_clicked(move |_| {
            закрой();
            дело("окно", json!({ "id": id }));
        });
        окна.add(&б);
    }
    столы.show_all();
    окна.show_all();
}
