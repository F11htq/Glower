//! Уведомления: всплывают справа сверху и копятся в центре уведомлений.
//!
//! Принимаем их отовсюду: от своих частей системы (вызов `покажи`), с шины
//! («уведомление» — так говорят агент и маленькие помощники) и от любых
//! программ Linux — по стандарту freedesktop: панель держит на шине
//! сеанса имя org.freedesktop.Notifications. Так Telegram, браузер и
//! почта показывают свои уведомления здесь же, а не молча.
//!
//! «Не беспокоить» не теряет уведомлений: они не всплывают, но лежат в
//! центре.

use crate::core::настройка;
use crate::popup;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

const СКОЛЬКО_ПОМНИТЬ: usize = 60;
const ВИСИТ: u64 = 6;

#[derive(Clone)]
pub struct Уведомление {
    pub номер: u32,
    pub заголовок: String,
    pub текст: String,
    pub значок: String,
    pub откуда: String,
    pub когда: i64,
    /// Что сделать по кнопке: подпись и дело
    pub кнопка: Option<(String, Rc<dyn Fn()>)>,
}

thread_local! {
    static ИСТОРИЯ: RefCell<Vec<Уведомление>> = const { RefCell::new(Vec::new()) };
    static СТОПКА: RefCell<Option<gtk::Window>> = const { RefCell::new(None) };
    static КАРТОЧКИ: RefCell<Vec<(u32, gtk::Widget)>> = const { RefCell::new(Vec::new()) };
    static КОЛОКОЛ: RefCell<Option<gtk::Button>> = const { RefCell::new(None) };
    static НЕПРОЧИТАНО: Cell<u32> = const { Cell::new(0) };
}

static НОМЕР: AtomicU32 = AtomicU32::new(0);
static ШИНА: OnceLock<gio::DBusConnection> = OnceLock::new();

fn следующий_номер() -> u32 {
    НОМЕР.fetch_add(1, Ordering::SeqCst) + 1
}

/// Своё уведомление
pub fn покажи(заголовок: &str, текст: &str, значок: &str) {
    добавь(Уведомление {
        номер: следующий_номер(),
        заголовок: заголовок.into(),
        текст: текст.into(),
        значок: значок.into(),
        откуда: "GlowerOS".into(),
        когда: glib::real_time() / 1_000_000,
        кнопка: None,
    });
}

/// Уведомление с кнопкой («Открыть»)
pub fn покажи_с_делом(заголовок: &str, текст: &str, значок: &str, кнопка: &str, дело: Box<dyn Fn()>) {
    добавь(Уведомление {
        номер: следующий_номер(),
        заголовок: заголовок.into(),
        текст: текст.into(),
        значок: значок.into(),
        откуда: "GlowerOS".into(),
        когда: glib::real_time() / 1_000_000,
        кнопка: Some((кнопка.into(), Rc::from(дело))),
    });
}

/// Звук звонка: из темы звуков freedesktop, чем есть. Нечем — молча
pub fn звук() {
    let файл = "/usr/share/sounds/freedesktop/stereo/alarm-clock-elapsed.oga";
    std::thread::spawn(move || {
        for (прог, доводы) in [("canberra-gtk-play", vec!["-i", "alarm-clock-elapsed"]), ("pw-play", vec![файл]), ("paplay", vec![файл])] {
            if std::process::Command::new(прог).args(&доводы).stderr(std::process::Stdio::null()).status().is_ok_and(|с| с.success()) {
                return;
            }
        }
    });
}

/// С шины: { заголовок, текст, значок, откуда, номер, звук }
pub fn из_шины(что: &Value) {
    if что["звук"].as_bool() == Some(true) {
        звук();
    }
    let строка = |k: &str| что[k].as_str().map(String::from).unwrap_or_else(|| if что[k].is_null() { String::new() } else { что[k].to_string() });
    let номер = что["номер"].as_u64().map(|н| н as u32).filter(|н| *н > 0).unwrap_or_else(следующий_номер);
    добавь(Уведомление {
        номер,
        заголовок: строка("заголовок"),
        текст: строка("текст"),
        значок: строка("значок"),
        откуда: строка("откуда"),
        когда: glib::real_time() / 1_000_000,
        кнопка: None,
    });
}

pub fn закрой_номер(номер: u32) {
    убери_карточку(номер);
}

fn тихо() -> bool {
    настройка("dnd").as_bool() == Some(true) || настройка("notif").as_bool() == Some(false)
}

fn добавь(у: Уведомление) {
    // Тот же номер — замена прежнего (так программы обновляют своё)
    ИСТОРИЯ.with(|и| {
        let mut и = и.borrow_mut();
        и.retain(|х| х.номер != у.номер);
        и.insert(0, у.clone());
        и.truncate(СКОЛЬКО_ПОМНИТЬ);
    });
    убери_карточку(у.номер);
    НЕПРОЧИТАНО.with(|н| н.set(н.get() + 1));
    обнови_колокол();
    if тихо() {
        return;
    }
    всплыви(&у);
}

/* ---------- всплывающие ---------- */

fn стопка() -> gtk::Window {
    if let Some(о) = СТОПКА.with(|с| с.borrow().clone()) {
        return о;
    }
    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_decorated(false);
    окно.set_app_paintable(true);
    if let Some(в) = gtk::prelude::GtkWindowExt::screen(&окно).and_then(|э| э.rgba_visual()) {
        окно.set_visual(Some(&в));
    }
    окно.connect_draw(|_, к| {
        к.set_operator(gtk::cairo::Operator::Source);
        к.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        let _ = к.paint();
        glib::Propagation::Proceed
    });
    окно.init_layer_shell();
    окно.set_layer(Layer::Overlay);
    окно.set_namespace("glower-notifications");
    окно.set_anchor(Edge::Top, true);
    окно.set_anchor(Edge::Right, true);
    окно.set_layer_shell_margin(Edge::Top, 40);
    окно.set_layer_shell_margin(Edge::Right, 10);
    окно.set_keyboard_mode(KeyboardMode::None);
    окно.style_context().add_class("toasts");
    let колонка = gtk::Box::new(gtk::Orientation::Vertical, 8);
    окно.add(&колонка);
    СТОПКА.with(|с| *с.borrow_mut() = Some(окно.clone()));
    окно
}

fn значок_уведомления(з: &str) -> gtk::Widget {
    // Путь к картинке, имя значка темы или эмодзи
    if з.starts_with('/') || з.starts_with("file://") {
        let путь = з.trim_start_matches("file://");
        if let Ok(к) = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(путь, 32, 32, true) {
            return gtk::Image::from_pixbuf(Some(&к)).upcast();
        }
    }
    if !з.is_empty() && з.chars().all(|с| с.is_ascii_alphanumeric() || "-_.".contains(с)) {
        if gtk::IconTheme::default().is_some_and(|т| т.has_icon(з)) {
            let и = gtk::Image::from_icon_name(Some(з), gtk::IconSize::Dnd);
            и.set_pixel_size(32);
            return и.upcast();
        }
    }
    let л = gtk::Label::new(Some(if з.is_empty() || з.is_ascii() { "🔔" } else { з }));
    л.style_context().add_class("toast-emoji");
    л.upcast()
}

fn карточка(у: &Уведомление, в_центре: bool) -> gtk::Widget {
    let к = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    к.style_context().add_class(if в_центре { "note-row" } else { "toast" });
    let з = значок_уведомления(&у.значок);
    з.set_valign(gtk::Align::Start);
    к.add(&з);
    let т = gtk::Box::new(gtk::Orientation::Vertical, 2);
    т.set_hexpand(true);
    let верх = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let заг = gtk::Label::new(Some(if у.заголовок.is_empty() { &у.откуда } else { &у.заголовок }));
    заг.set_xalign(0.0);
    заг.set_hexpand(true);
    заг.set_ellipsize(gtk::pango::EllipsizeMode::End);
    заг.style_context().add_class("toast-title");
    верх.add(&заг);
    if в_центре {
        let когда = glib::DateTime::from_unix_local(у.когда).and_then(|д| д.format("%H:%M")).map(|т| т.to_string()).unwrap_or_default();
        let в = gtk::Label::new(Some(&когда));
        в.style_context().add_class("toast-time");
        верх.add(&в);
    }
    т.add(&верх);
    if !у.текст.is_empty() {
        // Программы шлют простую разметку: <b>, <i>, ссылки — показываем текстом
        let текст = у.текст.replace("<b>", "").replace("</b>", "").replace("<i>", "").replace("</i>", "");
        let л = gtk::Label::new(Some(&текст));
        л.set_xalign(0.0);
        л.set_line_wrap(true);
        л.set_line_wrap_mode(gtk::pango::WrapMode::WordChar);
        л.set_max_width_chars(40);
        л.set_lines(if в_центре { 3 } else { 4 });
        л.set_ellipsize(gtk::pango::EllipsizeMode::End);
        л.style_context().add_class("toast-text");
        т.add(&л);
    }
    if let Some((подпись, дело)) = &у.кнопка {
        let б = gtk::Button::with_label(подпись);
        б.style_context().add_class("toast-action");
        б.set_halign(gtk::Align::Start);
        let (дело, номер) = (дело.clone(), у.номер);
        б.connect_clicked(move |_| {
            дело();
            убери_карточку(номер);
            popup::закрой();
        });
        т.add(&б);
    }
    к.add(&т);
    if !в_центре {
        let крест = crate::apps::значок_кнопка("×");
        крест.style_context().add_class("toast-close");
        крест.set_valign(gtk::Align::Start);
        let номер = у.номер;
        крест.connect_clicked(move |_| убери_карточку(номер));
        к.add(&крест);
    }
    // Щелчок по уведомлению программы — ей сигнал, как велит стандарт
    let коробка = gtk::EventBox::new();
    коробка.add(&к);
    let номер = у.номер;
    let наше = у.откуда == "GlowerOS";
    коробка.connect_button_release_event(move |_, е| {
        if е.button() == 1 && !наше {
            if let Some(ш) = ШИНА.get() {
                let _ = ш.emit_signal(None, "/org/freedesktop/Notifications", "org.freedesktop.Notifications", "ActionInvoked", Some(&(номер, "default").to_variant()));
            }
            убери_карточку(номер);
        }
        glib::Propagation::Proceed
    });
    коробка.upcast()
}

fn всплыви(у: &Уведомление) {
    let окно = стопка();
    let Some(колонка) = окно.child().and_then(|к| к.downcast::<gtk::Box>().ok()) else { return };
    let к = карточка(у, false);
    let обёртка = gtk::Revealer::new();
    обёртка.set_transition_type(gtk::RevealerTransitionType::SlideDown);
    обёртка.set_transition_duration(180);
    обёртка.add(&к);
    колонка.pack_start(&обёртка, false, false, 0);
    // Больше трёх разом не держим: старые уходят
    КАРТОЧКИ.with(|с| {
        let mut с = с.borrow_mut();
        с.push((у.номер, обёртка.clone().upcast()));
        while с.len() > 3 {
            let (_, старая) = с.remove(0);
            колонка.remove(&старая);
        }
    });
    // Пустую стопку слой не покажет: нулевой высоты поверхность сервер не
    // примет. Поэтому первая карточка встаёт сразу, следующие — выезжают
    if !окно.is_visible() {
        обёртка.set_reveal_child(true);
        окно.show_all();
    } else {
        окно.show_all();
        обёртка.set_reveal_child(true);
    }
    let номер = у.номер;
    // Висит, пока на него смотрят: под курсором не исчезает
    let наведено = Rc::new(Cell::new(false));
    {
        let н = наведено.clone();
        к.connect_enter_notify_event(move |_, _| {
            н.set(true);
            glib::Propagation::Proceed
        });
        let н = наведено.clone();
        к.connect_leave_notify_event(move |_, _| {
            н.set(false);
            glib::Propagation::Proceed
        });
    }
    let прошло = Rc::new(Cell::new(0u64));
    glib::timeout_add_local(Duration::from_secs(1), move || {
        if наведено.get() {
            return glib::ControlFlow::Continue;
        }
        прошло.set(прошло.get() + 1);
        if прошло.get() < ВИСИТ {
            return glib::ControlFlow::Continue;
        }
        убери_карточку(номер);
        glib::ControlFlow::Break
    });
}

fn убери_карточку(номер: u32) {
    let была = КАРТОЧКИ.with(|с| {
        let mut с = с.borrow_mut();
        с.iter().position(|(н, _)| *н == номер).map(|i| с.remove(i).1)
    });
    let Some(виджет) = была else { return };
    if let Some(р) = виджет.downcast_ref::<gtk::Revealer>() {
        р.set_reveal_child(false);
    }
    glib::timeout_add_local_once(Duration::from_millis(200), move || {
        if let Some(р) = виджет.parent() {
            if let Some(к) = р.downcast_ref::<gtk::Box>() {
                к.remove(&виджет);
            }
        }
        let пусто = КАРТОЧКИ.with(|с| с.borrow().is_empty());
        if пусто {
            if let Some(о) = СТОПКА.with(|с| с.borrow().clone()) {
                о.hide();
            }
        }
    });
}

/* ---------- центр уведомлений ---------- */

fn обнови_колокол() {
    let н = НЕПРОЧИТАНО.with(|н| н.get());
    КОЛОКОЛ.with(|к| {
        if let Some(к) = к.borrow().as_ref() {
            let знак = if тихо() { "🔕" } else { "🔔" };
            crate::apps::знак_кнопки(к, &if н > 0 { format!("{знак} {н}") } else { знак.to_string() });
            if н > 0 {
                к.style_context().add_class("unread");
            } else {
                к.style_context().remove_class("unread");
            }
        }
    });
}

/// Кнопка в полосе: колокольчик и число непрочитанных
pub fn колокол() -> gtk::Button {
    let б = crate::apps::значок_кнопка("🔔");
    б.set_tooltip_text(Some("Уведомления"));
    б.style_context().add_class("bell");
    б.connect_clicked(|б| центр(б));
    КОЛОКОЛ.with(|к| *к.borrow_mut() = Some(б.clone()));
    // «Не беспокоить» могли включить в Параметрах — колокол следит
    glib::timeout_add_local(Duration::from_secs(2), || {
        обнови_колокол();
        glib::ControlFlow::Continue
    });
    б
}

pub fn центр(кнопка: &gtk::Button) {
    НЕПРОЧИТАНО.with(|н| н.set(0));
    обнови_колокол();
    popup::открой(кнопка, "уведомления", popup::Где::Справа, 360, |меню| наполни_центр(меню));
}

fn наполни_центр(меню: &gtk::Box) {
    for р in меню.children() {
        меню.remove(&р);
    }
    let шапка = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let з = gtk::Label::new(Some("Уведомления"));
    з.style_context().add_class("title");
    з.set_xalign(0.0);
    з.set_hexpand(true);
    шапка.add(&з);
    let пусто = ИСТОРИЯ.with(|и| и.borrow().is_empty());
    if !пусто {
        let очистить = gtk::Button::with_label("Очистить");
        очистить.style_context().add_class("action");
        let м = меню.clone();
        очистить.connect_clicked(move |_| {
            ИСТОРИЯ.with(|и| и.borrow_mut().clear());
            наполни_центр(&м);
            м.show_all();
        });
        шапка.add(&очистить);
    }
    меню.add(&шапка);

    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let п = gtk::Label::new(Some("Не беспокоить"));
    п.set_xalign(0.0);
    п.set_hexpand(true);
    ряд.add(&п);
    let в = gtk::Switch::new();
    в.set_active(настройка("dnd").as_bool() == Some(true));
    в.connect_active_notify(|в| {
        crate::core::задай("dnd", serde_json::json!(в.is_active()));
        обнови_колокол();
    });
    ряд.add(&в);
    меню.add(&ряд);

    let список = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let история = ИСТОРИЯ.with(|и| и.borrow().clone());
    if история.is_empty() {
        let л = gtk::Label::new(Some("Новых уведомлений нет"));
        л.style_context().add_class("small");
        л.set_margin_top(18);
        л.set_margin_bottom(18);
        список.add(&л);
    }
    for у in история.iter() {
        список.add(&карточка(у, true));
    }
    let прокрутка = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    прокрутка.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    прокрутка.set_max_content_height(460);
    прокрутка.set_propagate_natural_height(true);
    прокрутка.add(&список);
    меню.add(&прокрутка);
}

/* ---------- служба уведомлений для программ ---------- */

const ОПИСАНИЕ: &str = r#"<node>
  <interface name="org.freedesktop.Notifications">
    <method name="Notify">
      <arg type="s" direction="in"/><arg type="u" direction="in"/><arg type="s" direction="in"/>
      <arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="as" direction="in"/>
      <arg type="a{sv}" direction="in"/><arg type="i" direction="in"/><arg type="u" direction="out"/>
    </method>
    <method name="CloseNotification"><arg type="u" direction="in"/></method>
    <method name="GetCapabilities"><arg type="as" direction="out"/></method>
    <method name="GetServerInformation">
      <arg type="s" direction="out"/><arg type="s" direction="out"/><arg type="s" direction="out"/><arg type="s" direction="out"/>
    </method>
    <signal name="NotificationClosed"><arg type="u"/><arg type="u"/></signal>
    <signal name="ActionInvoked"><arg type="u"/><arg type="s"/></signal>
  </interface>
</node>"#;

enum Весть {
    Новое { номер: u32, откуда: String, значок: String, заголовок: String, текст: String },
    Закрыть(u32),
}

pub fn служба() {
    let (куда, откуда) = async_channel::unbounded::<Весть>();
    glib::MainContext::default().spawn_local(async move {
        while let Ok(в) = откуда.recv().await {
            match в {
                Весть::Новое { номер, откуда, значок, заголовок, текст } => добавь(Уведомление {
                    номер,
                    заголовок,
                    текст,
                    значок,
                    откуда,
                    когда: glib::real_time() / 1_000_000,
                    кнопка: None,
                }),
                Весть::Закрыть(н) => убери_карточку(н),
            }
        }
    });
    let куда = std::sync::Arc::new(куда);
    gio::bus_own_name(
        gio::BusType::Session,
        "org.freedesktop.Notifications",
        gio::BusNameOwnerFlags::REPLACE | gio::BusNameOwnerFlags::ALLOW_REPLACEMENT,
        move |шина, _| {
            let Ok(узел) = gio::DBusNodeInfo::for_xml(ОПИСАНИЕ) else { return };
            let Some(описание) = узел.lookup_interface("org.freedesktop.Notifications") else { return };
            let куда = куда.clone();
            let _ = ШИНА.set(шина.clone());
            let _ = шина.register_object(
                "/org/freedesktop/Notifications",
                &описание,
                move |_, _, _, _, метод, доводы, ответ| match метод {
                    "Notify" => {
                        let откуда = доводы.child_value(0).str().unwrap_or("").to_string();
                        let замена = доводы.child_value(1).get::<u32>().unwrap_or(0);
                        let значок = доводы.child_value(2).str().unwrap_or("").to_string();
                        let заголовок = доводы.child_value(3).str().unwrap_or("").to_string();
                        let текст = доводы.child_value(4).str().unwrap_or("").to_string();
                        let номер = if замена > 0 { замена } else { следующий_номер() };
                        let _ = куда.send_blocking(Весть::Новое { номер, откуда, значок, заголовок, текст });
                        ответ.return_value(Some(&(номер,).to_variant()));
                    }
                    "CloseNotification" => {
                        let н = доводы.child_value(0).get::<u32>().unwrap_or(0);
                        let _ = куда.send_blocking(Весть::Закрыть(н));
                        ответ.return_value(None);
                    }
                    "GetCapabilities" => ответ.return_value(Some(&(vec!["body".to_string(), "actions".to_string(), "persistence".to_string()],).to_variant())),
                    "GetServerInformation" => ответ.return_value(Some(&("GlowerOS", "GlowerOS", "1.0", "1.2").to_variant())),
                    _ => ответ.return_value(None),
                },
                |_, _, _, _, _| glib::Variant::from(""),
                |_, _, _, _, _, _| false,
            );
        },
        |_, _| eprintln!("glower-panel: принимаю уведомления программ"),
        |_, _| eprintln!("glower-panel: уведомления программ принимает кто-то другой"),
    );
}
