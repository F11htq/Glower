//! Экран блокировки — glower-lock (та же программа, что панель, другим
//! именем).
//!
//! Выглядит как экран входа: обои, часы, аватар и имя, поле пароля, другие
//! люди машины и кнопки питания. Прежний замок (swaylock) был чистым
//! цветным экраном без единого слова, и человек не понимал, что от него
//! ждут.
//!
//! Замок должен быть настоящим, а не картинкой поверх окон:
//!   — окно лежит над всем, во весь экран, и клавиатура только его;
//!   — оконному серверу мы говорим «запереть ввод» (протокол
//!     wlr-input-inhibitor): пока замок жив, labwc не выполняет своих
//!     сочетаний и не отдаёт ни клавиш, ни мыши никому другому;
//!   — пароль проверяет PAM, как у любого экрана блокировки;
//!   — замок — отдельный процесс: упадёт панель — экран останется запертым.

use crate::agent::позови;
use gtk::glib;
use gtk::glib::translate::ToGlibPtr;
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use serde_json::{json, Value};
use std::ffi::{c_char, c_int, c_void, CString};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::wl_registry;
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols_wlr::input_inhibitor::v1::client::{
    zwlr_input_inhibit_manager_v1::ZwlrInputInhibitManagerV1, zwlr_input_inhibitor_v1::ZwlrInputInhibitorV1,
};

/// Тот же вид, что у экрана входа (css/extra.css, «.вход…»): замок — это
/// тот же экран, что встречает при включении, только с запертым сеансом.
const СТИЛЬ: &str = r#"
window.lock {
  background-image:
    linear-gradient(180deg, rgba(6,8,20,.34) 0%, rgba(6,8,20,.06) 34%, rgba(6,8,20,.46) 100%),
    radial-gradient(66% 54% at 14% 86%, rgba(255,146,72,.92) 0%, rgba(255,88,124,.46) 42%, rgba(255,88,124,0) 74%),
    radial-gradient(56% 50% at 86% 18%, rgba(64,232,214,.72) 0%, rgba(52,138,255,.40) 46%, rgba(52,138,255,0) 78%),
    radial-gradient(74% 68% at 66% 94%, rgba(176,84,255,.72) 0%, rgba(176,84,255,0) 70%),
    radial-gradient(86% 80% at 26% 8%, rgba(126,96,255,.60) 0%, rgba(126,96,255,0) 68%),
    linear-gradient(158deg, #1b2554 0%, #241a48 46%, #0d1026 100%);
}
window.lock, window.lock * {
  color: #eef1ff;
  font-family: sans-serif;
  font-size: 15px;
  text-shadow: none;
  -gtk-icon-shadow: none;
}
.lock .time { font-size: 64px; font-weight: 200; letter-spacing: 2px; }
.lock .date { opacity: 0.62; margin-top: 4px; }
.lock .avatar {
  font-size: 38px;
  font-weight: 500;
  color: #ffffff;
  min-width: 96px;
  min-height: 96px;
  border-radius: 48px;
  background-image: linear-gradient(140deg, #6d5cff, #c04bff);
}
.lock .name { font-size: 20px; font-weight: 500; }
.lock entry {
  background-color: rgba(255, 255, 255, 0.07);
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 11px;
  padding: 11px 14px;
  min-width: 230px;
  min-height: 0;
  box-shadow: none;
  color: #ffffff;
  caret-color: #ffffff;
}
.lock entry:focus { border-color: rgba(160, 140, 255, 0.75); background-color: rgba(255, 255, 255, 0.10); }
.lock button {
  background: none;
  border: none;
  box-shadow: none;
  min-height: 0;
  min-width: 0;
}
.lock button.go {
  background-image: linear-gradient(140deg, #6d5cff, #a24bff);
  border-radius: 11px;
  min-width: 44px;
  min-height: 44px;
  padding: 0;
}
.lock button.go label { font-size: 19px; }
.lock .error { min-height: 20px; color: #ffb3c1; opacity: 0.85; }
.lock .error, .lock .error * { font-size: 13px; }
.lock button.person {
  padding: 7px 13px;
  border-radius: 9px;
  border: 1px solid rgba(255, 255, 255, 0.16);
  background-color: rgba(255, 255, 255, 0.06);
}
.lock button.person label { font-size: 13.5px; color: #e8ecff; }
.lock button.person:hover { background-color: rgba(255, 255, 255, 0.12); }
.lock button.person.on { background-color: rgba(140, 120, 255, 0.28); border-color: rgba(160, 140, 255, 0.6); }
.lock button.power {
  min-width: 42px;
  min-height: 42px;
  padding: 0;
  border-radius: 12px;
  border: 1px solid rgba(255, 255, 255, 0.18);
  background-color: rgba(255, 255, 255, 0.08);
}
.lock button.power:hover { background-color: rgba(255, 255, 255, 0.18); }
.lock button.main {
  background-image: linear-gradient(140deg, #6d5cff, #a24bff);
  border-radius: 11px;
  padding: 9px 16px;
}
.lock button.plain {
  background-color: rgba(255, 255, 255, 0.08);
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 11px;
  padding: 9px 16px;
}
"#;

// ------------------------------------------------------------- запереть ввод

struct Тихо;
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Тихо {
    fn event(_: &mut Self, _: &wl_registry::WlRegistry, _: wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<ZwlrInputInhibitManagerV1, ()> for Тихо {
    fn event(_: &mut Self, _: &ZwlrInputInhibitManagerV1, _: <ZwlrInputInhibitManagerV1 as wayland_client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<ZwlrInputInhibitorV1, ()> for Тихо {
    fn event(_: &mut Self, _: &ZwlrInputInhibitorV1, _: <ZwlrInputInhibitorV1 as wayland_client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

extern "C" {
    fn gdk_wayland_display_get_wl_display(display: *mut gtk::gdk::ffi::GdkDisplay) -> *mut c_void;
}

/// Просим оконный сервер отдавать ввод только нам. Держим до выхода из
/// программы: выход и есть «отпереть».
fn запри_ввод() -> Result<(Connection, ZwlrInputInhibitorV1), String> {
    let экран = gtk::gdk::Display::default().ok_or("нет экрана")?;
    let указатель = unsafe { gdk_wayland_display_get_wl_display(экран.to_glib_none().0) };
    if указатель.is_null() {
        return Err("экран не Wayland".into());
    }
    // То же соединение, что у GTK: свой отдельный разговор с оконным
    // сервером не годится — запирать надо ввод именно для наших окон
    let связь = unsafe { wayland_backend::client::Backend::from_foreign_display(указатель as *mut _) };
    let связь = Connection::from_backend(связь);
    let (всё, mut очередь) = registry_queue_init::<Тихо>(&связь).map_err(|е| е.to_string())?;
    let ру = очередь.handle();
    let менеджер: ZwlrInputInhibitManagerV1 =
        всё.bind(&ру, 1..=1, ()).map_err(|_| "оконный сервер не умеет запирать ввод".to_string())?;
    let запор = менеджер.get_inhibitor(&ру, ());
    очередь.roundtrip(&mut Тихо).map_err(|е| е.to_string())?;
    Ok((связь, запор))
}

// ------------------------------------------------------------- пароль: PAM

#[repr(C)]
struct PamMessage {
    msg_style: c_int,
    msg: *const c_char,
}
#[repr(C)]
struct PamResponse {
    resp: *mut c_char,
    resp_retcode: c_int,
}
#[repr(C)]
struct PamConv {
    conv: extern "C" fn(c_int, *mut *const PamMessage, *mut *mut PamResponse, *mut c_void) -> c_int,
    appdata_ptr: *mut c_void,
}

#[link(name = "pam")]
extern "C" {
    fn pam_start(service: *const c_char, user: *const c_char, conv: *const PamConv, pamh: *mut *mut c_void) -> c_int;
    fn pam_authenticate(pamh: *mut c_void, flags: c_int) -> c_int;
    fn pam_end(pamh: *mut c_void, status: c_int) -> c_int;
}
extern "C" {
    fn calloc(n: usize, size: usize) -> *mut c_void;
    fn strdup(s: *const c_char) -> *mut c_char;
}

const PAM_SUCCESS: c_int = 0;
const PAM_CONV_ERR: c_int = 19;
const PAM_BUF_ERR: c_int = 5;
const PAM_PROMPT_ECHO_OFF: c_int = 1;
const PAM_PROMPT_ECHO_ON: c_int = 2;

/// PAM спрашивает — мы отвечаем паролем на каждый вопрос «введите».
/// Ответы выделяем через calloc и strdup: освобождает их сам PAM.
extern "C" fn разговор(n: c_int, вопросы: *mut *const PamMessage, ответы: *mut *mut PamResponse, пароль: *mut c_void) -> c_int {
    if n <= 0 || вопросы.is_null() || ответы.is_null() {
        return PAM_CONV_ERR;
    }
    unsafe {
        let список = calloc(n as usize, std::mem::size_of::<PamResponse>()) as *mut PamResponse;
        if список.is_null() {
            return PAM_BUF_ERR;
        }
        for i in 0..n as usize {
            let в = *вопросы.add(i);
            if в.is_null() {
                continue;
            }
            let вид = (*в).msg_style;
            if вид == PAM_PROMPT_ECHO_OFF || вид == PAM_PROMPT_ECHO_ON {
                (*список.add(i)).resp = strdup(пароль as *const c_char);
            }
        }
        *ответы = список;
    }
    PAM_SUCCESS
}

/// Верен ли пароль человека. Своя служба PAM — glower-lock; если её файла
/// нет (машина не обновилась целиком), берём «login»: со службой, которой
/// нет, PAM не пускает никого, и человек оказался бы заперт навсегда.
fn пароль_верен(кто: &str, пароль: &str) -> bool {
    let служба = if std::path::Path::new("/etc/pam.d/glower-lock").exists() { "glower-lock" } else { "login" };
    let (Ok(служба), Ok(кто), Ok(пароль)) = (CString::new(служба), CString::new(кто), CString::new(пароль)) else {
        return false;
    };
    let беседа = PamConv { conv: разговор, appdata_ptr: пароль.as_ptr() as *mut c_void };
    let mut ручка: *mut c_void = std::ptr::null_mut();
    unsafe {
        if pam_start(служба.as_ptr(), кто.as_ptr(), &беседа, &mut ручка) != PAM_SUCCESS {
            return false;
        }
        let итог = pam_authenticate(ручка, 0);
        pam_end(ручка, итог);
        итог == PAM_SUCCESS
    }
}

// ------------------------------------------------------------- окно

fn кто_я() -> String {
    std::env::var("USER").ok().filter(|у| !у.is_empty()).unwrap_or_else(|| {
        // Без переменной — по своему номеру в системе
        let номер = unsafe { getuid() };
        let mut имя = String::new();
        if let Ok(строки) = std::fs::read_to_string("/etc/passwd") {
            for с in строки.lines() {
                let части: Vec<&str> = с.split(':').collect();
                if части.len() > 2 && части[2] == номер.to_string() {
                    имя = части[0].to_string();
                    break;
                }
            }
        }
        имя
    })
}
extern "C" {
    fn getuid() -> u32;
}

/// Полное имя человека из /etc/passwd («Андрей Малышенко,,,» → «Андрей Малышенко»).
fn полное_имя(логин: &str) -> String {
    std::fs::read_to_string("/etc/passwd")
        .ok()
        .and_then(|с| {
            с.lines().find_map(|строка| {
                let ч: Vec<&str> = строка.split(':').collect();
                (ч.len() > 4 && ч[0] == логин).then(|| ч[4].split(',').next().unwrap_or("").trim().to_string())
            })
        })
        .filter(|и| !и.is_empty())
        .unwrap_or_else(|| логин.to_string())
}

/// Люди машины — по тем же правилам, что у агента экрана входа
/// (agent/login.mjs): обычные учётные записи, в которые можно войти.
fn люди_системы() -> Vec<Value> {
    let mut список: Vec<(u32, Value)> = std::fs::read_to_string("/etc/passwd")
        .unwrap_or_default()
        .lines()
        .filter_map(|строка| {
            let ч: Vec<&str> = строка.split(':').collect();
            if ч.len() < 7 {
                return None;
            }
            let номер: u32 = ч[2].parse().ok()?;
            if !(1000..65000).contains(&номер) || ч[6].contains("nologin") || ч[6].ends_with("/false") {
                return None;
            }
            let полное = ч[4].split(',').next().unwrap_or("").trim();
            Some((номер, json!({ "имя": ч[0], "полное": if полное.is_empty() { ч[0] } else { полное } })))
        })
        .collect();
    список.sort_by_key(|(н, _)| *н);
    список.into_iter().map(|(_, ч)| ч).collect()
}

const МЕСЯЦЫ: [&str; 12] = [
    "января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа", "сентября",
    "октября", "ноября", "декабря",
];
const ДНИ: [&str; 7] = ["понедельник", "вторник", "среда", "четверг", "пятница", "суббота", "воскресенье"];

fn с_большой(с: &str) -> String {
    let mut б = с.chars();
    б.next().map(|п| п.to_uppercase().collect::<String>() + б.as_str()).unwrap_or_default()
}

fn часы() -> (String, String) {
    let т = glib::DateTime::now_local().expect("часы машины");
    (
        format!("{:02}:{:02}", т.hour(), т.minute()),
        format!(
            "{}, {} {}",
            с_большой(ДНИ[(т.day_of_week() as usize).clamp(1, 7) - 1]),
            т.day_of_month(),
            МЕСЯЦЫ[(т.month() as usize).clamp(1, 12) - 1]
        ),
    )
}

/// Знак кнопки питания — линиями, как SVG экрана входа: шрифтовые «⏻» и
/// «⟳» на разных машинах выглядят по-разному, а где-то их нет вовсе.
fn знак_питания(перезагрузка: bool) -> gtk::DrawingArea {
    let х = gtk::DrawingArea::new();
    х.set_size_request(20, 20);
    х.set_halign(gtk::Align::Center);
    х.set_valign(gtk::Align::Center);
    х.connect_draw(move |в, к| {
        let м = в.allocated_width().min(в.allocated_height()) as f64 / 24.0;
        к.scale(м, м);
        к.set_source_rgb(0.933, 0.945, 1.0);
        к.set_line_width(1.9);
        к.set_line_cap(gtk::cairo::LineCap::Round);
        к.set_line_join(gtk::cairo::LineJoin::Round);
        use std::f64::consts::PI;
        if перезагрузка {
            // «M20 12a8 8 0 1 1-2.34-5.66» — почти полный круг, «M20 4v4.4h-4.4» — стрелка
            к.arc(12.0, 12.0, 8.0, 0.0, 2.0 * PI - PI / 4.0);
            let _ = к.stroke();
            к.move_to(20.0, 4.0);
            к.line_to(20.0, 8.4);
            к.line_to(15.6, 8.4);
            let _ = к.stroke();
        } else {
            // «M12 3.6v8» и дуга «M7.1 6.6a7.4 7.4 0 1 0 9.8 0»
            к.move_to(12.0, 3.6);
            к.line_to(12.0, 11.6);
            let _ = к.stroke();
            к.arc_negative(12.0, 12.15, 7.4, 228.6_f64.to_radians(), (-48.6_f64).to_radians());
            let _ = к.stroke();
        }
        glib::Propagation::Proceed
    });
    х
}

/// Окно на одном мониторе. На главном — всё, на остальных — только фон
/// и часы: поле пароля одно, иначе непонятно, куда печатать.
/// Чем служит экран: замок поверх запертого сеанса или вход в систему.
#[derive(Clone, Copy, PartialEq)]
pub enum Режим {
    Замок,
    Вход,
}

/// Фон — обои стола, притемнённые сверху и снизу, чтобы читались часы и
/// поле пароля. Экрану входа чужие настройки не прочесть — у него обои
/// системы по умолчанию
fn фон_обоями(о: &gtk::Window, вход: bool) {
    let id = if вход {
        "glower".to_string()
    } else {
        std::fs::read_to_string(gtk::glib::user_config_dir().join("glower").join("settings.json"))
            .ok()
            .and_then(|т| serde_json::from_str::<serde_json::Value>(&т).ok())
            .and_then(|н| н["wallpaper"].as_str().map(String::from))
            .unwrap_or_else(|| "glower".into())
    };
    let обои = crate::desktop::цвета_для_gtk(&crate::core::обои_по_имени(&id));
    let css = format!(
        "window.lock {{ background-image: linear-gradient(180deg, rgba(6,8,20,.42) 0%, rgba(6,8,20,.12) 34%, rgba(6,8,20,.58) 100%), {обои}; background-size: cover; background-position: center; }}"
    );
    let с = gtk::CssProvider::new();
    if с.load_from_data(css.as_bytes()).is_ok() {
        о.style_context().add_provider(&с, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2);
    }
}

fn окно(монитор: &gtk::gdk::Monitor, главное: bool, режим: Режим) -> (gtk::Window, Option<gtk::Entry>) {
    let о = gtk::Window::new(gtk::WindowType::Toplevel);
    о.set_decorated(false);
    о.style_context().add_class("lock");
    фон_обоями(&о, режим == Режим::Вход);
    о.init_layer_shell();
    о.set_monitor(монитор);
    о.set_layer(Layer::Overlay);
    о.set_namespace("glower-lock");
    for край in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        о.set_anchor(край, true);
    }
    о.set_exclusive_zone(-1);
    о.set_keyboard_mode(if главное { KeyboardMode::Exclusive } else { KeyboardMode::None });

    // Как на экране входа: часы сверху, на девятой доле высоты; посередине
    // человек и пароль; в углу питание
    let слой = gtk::Overlay::new();
    о.add(&слой);
    let середина = gtk::Box::new(gtk::Orientation::Vertical, 14);
    середина.set_valign(gtk::Align::Center);
    середина.set_halign(gtk::Align::Center);
    середина.set_size_request(320, -1);
    слой.add(&середина);

    let часы_ = gtk::Box::new(gtk::Orientation::Vertical, 0);
    часы_.set_valign(gtk::Align::Start);
    часы_.set_halign(gtk::Align::Center);
    часы_.set_margin_top((монитор.geometry().height() as f64 * 0.09) as i32);
    let (время, дата) = часы();
    let в = gtk::Label::new(Some(&время));
    в.style_context().add_class("time");
    let д = gtk::Label::new(Some(&дата));
    д.style_context().add_class("date");
    часы_.add(&в);
    часы_.add(&д);
    слой.add_overlay(&часы_);
    {
        let (в, д) = (в.clone(), д.clone());
        glib::timeout_add_local(Duration::from_secs(5), move || {
            let (время, дата) = часы();
            в.set_text(&время);
            д.set_text(&дата);
            glib::ControlFlow::Continue
        });
    }
    if !главное {
        return (о, None);
    }

    // Кто входит или чей сеанс заперт. На замке — тот, кто работал; на
    // входе — первый человек машины, пока не выбрали другого
    let выбран: Rc<RefCell<(String, String)>> = Rc::new(RefCell::new(match режим {
        Режим::Замок => {
            let логин = кто_я();
            let имя = полное_имя(&логин);
            (логин, имя)
        }
        Режим::Вход => (String::new(), "…".to_string()),
    }));
    let аватар = gtk::Label::new(None);
    аватар.style_context().add_class("avatar");
    аватар.set_halign(gtk::Align::Center);
    середина.add(&аватар);
    let подпись = gtk::Label::new(None);
    подпись.style_context().add_class("name");
    середина.add(&подпись);
    let покажи_человека = {
        let (аватар, подпись, выбран) = (аватар.clone(), подпись.clone(), выбран.clone());
        Rc::new(move || {
            let имя = выбран.borrow().1.clone();
            аватар.set_text(&имя.chars().next().filter(|б| *б != '…').map(|б| б.to_uppercase().to_string()).unwrap_or("?".into()));
            подпись.set_text(&имя);
        })
    };
    покажи_человека();

    let строка = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    строка.set_halign(gtk::Align::Center);
    let поле = gtk::Entry::new();
    поле.set_visibility(false);
    поле.set_placeholder_text(Some("Пароль"));
    поле.set_input_purpose(gtk::InputPurpose::Password);
    let вперёд = crate::apps::значок_кнопка("→");
    вперёд.style_context().add_class("go");
    вперёд.set_tooltip_text(Some("Войти"));
    строка.add(&поле);
    строка.add(&вперёд);
    середина.add(&строка);
    let беда = gtk::Label::new(None);
    беда.style_context().add_class("error");
    беда.set_line_wrap(true);
    беда.set_max_width_chars(40);
    беда.set_justify(gtk::Justification::Center);
    середина.add(&беда);

    // Проверка пароля. На замке — сами, через PAM, в рабочем потоке: при
    // неверном пароле PAM нарочно медлит пару секунд. На входе — через
    // агента: пароль он отдаёт службе входа (greetd), а та и заводит сеанс
    let проверь = {
        let (поле, беда, выбран) = (поле.clone(), беда.clone(), выбран.clone());
        Rc::new(move || {
            let пароль = поле.text().to_string();
            let логин = выбран.borrow().0.clone();
            if пароль.is_empty() || логин.is_empty() || !поле.is_sensitive() {
                return;
            }
            поле.set_sensitive(false);
            беда.set_text("Проверяю…");
            let (поле, беда) = (поле.clone(), беда.clone());
            match режим {
                Режим::Замок => {
                    let (куда, откуда) = async_channel::bounded(1);
                    std::thread::spawn(move || {
                        let _ = куда.send_blocking(пароль_верен(&логин, &пароль));
                    });
                    glib::MainContext::default().spawn_local(async move {
                        if let Ok(true) = откуда.recv().await {
                            // Отпереть — значит просто выйти: окна уходят, запор
                            // ввода снимается вместе с соединением
                            std::process::exit(0);
                        }
                        беда.set_text("Пароль не подошёл");
                        поле.set_text("");
                        поле.set_sensitive(true);
                        поле.grab_focus();
                    });
                }
                Режим::Вход => {
                    позови("login.enter", json!({ "имя": логин, "пароль": пароль }), move |о| {
                        match о {
                            // Сеанс запущен — служба входа сейчас закроет нас сама
                            Ok(о) if о["ok"].as_bool() == Some(true) => беда.set_text("Входим…"),
                            Ok(о) => {
                                беда.set_text(о["почему"].as_str().unwrap_or("Войти не удалось"));
                                поле.set_text("");
                                поле.set_sensitive(true);
                                поле.grab_focus();
                            }
                            Err(е) => {
                                беда.set_text(&е);
                                поле.set_sensitive(true);
                                поле.grab_focus();
                            }
                        }
                    });
                }
            }
        })
    };
    {
        let п = проверь.clone();
        поле.connect_activate(move |_| п());
    }
    {
        let п = проверь.clone();
        вперёд.connect_clicked(move |_| п());
    }
    о.connect_key_press_event({
        let поле = поле.clone();
        move |_, с| {
            if с.keyval() == gtk::gdk::keys::constants::Escape {
                поле.set_text("");
            }
            glib::Propagation::Proceed
        }
    });

    // Вопрос «точно?» — на месте поля пароля, без отдельных окон
    let спроси = {
        let (середина, строка, беда, поле) = (середина.clone(), строка.clone(), беда.clone(), поле.clone());
        Rc::new(move |вопрос: String, кнопка: &'static str, дело: Rc<dyn Fn()>| {
            let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            ряд.set_halign(gtk::Align::Center);
            беда.set_text(&вопрос);
            let нет = gtk::Button::with_label("Отмена");
            нет.style_context().add_class("plain");
            let да = gtk::Button::with_label(кнопка);
            да.style_context().add_class("main");
            ряд.add(&нет);
            ряд.add(&да);
            строка.hide();
            середина.add(&ряд);
            середина.reorder_child(&ряд, 2);
            ряд.show_all();
            {
                let (ряд, строка, беда, поле) = (ряд.clone(), строка.clone(), беда.clone(), поле.clone());
                нет.connect_clicked(move |_| {
                    unsafe { ряд.destroy() };
                    беда.set_text("");
                    строка.show();
                    поле.grab_focus();
                });
            }
            да.connect_clicked(move |_| дело());
        })
    };

    // Люди этой машины — плашками, если их больше одного. На входе плашка
    // выбирает, кто входит. На замке войти другим, не закрыв сеанс, служба
    // входа не умеет — честно говорим, что сеанс завершится
    let люди = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    люди.set_halign(gtk::Align::Center);
    люди.set_margin_top(6);
    середина.add(&люди);
    {
        let (люди, спроси, выбран, покажи, поле, беда) =
            (люди.clone(), спроси.clone(), выбран.clone(), покажи_человека.clone(), поле.clone(), беда.clone());
        let ответ = move |о: Result<Value, String>| {
            let о = о.unwrap_or(Value::Null);
            let список: Vec<Value> = о["list"].as_array().cloned().unwrap_or_default();
            if режим == Режим::Вход {
                if о["готов"].as_bool() == Some(false) {
                    беда.set_text("Служба входа не отвечает — сеанс не запустится");
                }
                match список.first() {
                    Some(ч) => {
                        let ник = ч["имя"].as_str().unwrap_or("").to_string();
                        let полное = ч["полное"].as_str().filter(|п| !п.is_empty()).unwrap_or(&ник).to_string();
                        *выбран.borrow_mut() = (ник, полное);
                    }
                    None => *выбран.borrow_mut() = (String::new(), "Никого нет".into()),
                }
                покажи();
                поле.grab_focus();
            }
            if список.len() < 2 {
                return; // одному человеку выбирать не из кого
            }
            let кнопки: Rc<RefCell<Vec<(String, gtk::Button)>>> = Rc::new(RefCell::new(Vec::new()));
            for ч in список {
                let ник = ч["имя"].as_str().unwrap_or("").to_string();
                if ник.is_empty() {
                    continue;
                }
                let полное = ч["полное"].as_str().filter(|п| !п.is_empty()).unwrap_or(&ник).to_string();
                let б = gtk::Button::with_label(&полное);
                б.style_context().add_class("person");
                if ник == выбран.borrow().0 {
                    б.style_context().add_class("on");
                }
                match режим {
                    Режим::Вход => {
                        let (выбран, покажи, поле, беда, кнопки) =
                            (выбран.clone(), покажи.clone(), поле.clone(), беда.clone(), кнопки.clone());
                        let ник_ = ник.clone();
                        б.connect_clicked(move |_| {
                            *выбран.borrow_mut() = (ник_.clone(), полное.clone());
                            for (н, к) in кнопки.borrow().iter() {
                                if н == &ник_ {
                                    к.style_context().add_class("on");
                                } else {
                                    к.style_context().remove_class("on");
                                }
                            }
                            покажи();
                            поле.set_text("");
                            беда.set_text("");
                            поле.grab_focus();
                        });
                    }
                    Режим::Замок if ник != выбран.borrow().0 => {
                        let спроси = спроси.clone();
                        б.connect_clicked(move |_| {
                            спроси(
                                format!("Чтобы войти как {полное}, ваш сеанс завершится и программы закроются"),
                                "Выйти",
                                Rc::new(|| позови("sys.power", json!({ "action": "logout" }), |_| {})),
                            )
                        });
                    }
                    Режим::Замок => {}
                }
                кнопки.borrow_mut().push((ник, б.clone()));
                люди.add(&б);
            }
            люди.show_all();
        };
        match режим {
            // Вход спрашивает агента экрана входа: он же знает, готова ли
            // служба входа
            Режим::Вход => позови("login.users", json!({}), ответ),
            // Замок не зависит ни от кого: людей читаем сами
            Режим::Замок => ответ(Ok(json!({ "list": люди_системы() }))),
        }
    }

    let питание = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    питание.set_halign(gtk::Align::End);
    питание.set_valign(gtk::Align::End);
    питание.set_margin_end(28);
    питание.set_margin_bottom(26);
    for (перезагрузка, что, имя, вопрос) in [
        (true, "reboot", "Перезагрузить", "Перезагрузить? Программы закроются, несохранённое пропадёт"),
        (false, "poweroff", "Выключить", "Выключить? Программы закроются, несохранённое пропадёт"),
    ] {
        let б = gtk::Button::new();
        б.add(&знак_питания(перезагрузка));
        б.style_context().add_class("power");
        б.set_tooltip_text(Some(имя));
        let спроси = спроси.clone();
        б.connect_clicked(move |_| {
            let сделай: Rc<dyn Fn()> = Rc::new(move || позови("sys.power", json!({ "action": что }), |_| {}));
            match режим {
                // На входе программ ещё нет — терять нечего
                Режим::Вход => сделай(),
                Режим::Замок => спроси(вопрос.to_string(), имя, сделай),
            }
        });
        питание.add(&б);
    }
    слой.add_overlay(&питание);

    (о, Some(поле))
}

pub fn запусти(режим: Режим) -> ! {
    if gtk::init().is_err() {
        eprintln!("glower-lock: нет экрана");
        std::process::exit(1);
    }
    crate::i18n::подключи();
    if !gtk_layer_shell::is_supported() {
        eprintln!("glower-lock: оконный сервер не умеет слоёв");
        std::process::exit(3);
    }
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль замка");
    gtk::StyleContext::add_provider_for_screen(&gtk::gdk::Screen::default().expect("экран"), &стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);

    let экран = gtk::gdk::Display::default().expect("экран");
    let главный = экран.primary_monitor().or_else(|| экран.monitor(0));
    let mut поле_пароля = None;
    for i in 0..экран.n_monitors() {
        let Some(м) = экран.monitor(i) else { continue };
        let главное = главный.as_ref().map(|г| г == &м).unwrap_or(i == 0);
        let (о, поле) = окно(&м, главное, режим);
        о.show_all();
        if поле.is_some() {
            поле_пароля = поле;
        }
    }
    if let Some(п) = &поле_пароля {
        п.grab_focus();
    }

    // Окна на экране — теперь запираем ввод. Не вышло — всё равно стоим:
    // окно над всем и клавиатура наша, а это лучше, чем не запереть вовсе.
    // На входе запирать нечего: кроме этого экрана, в сеансе входа никого
    let запор = if режим == Режим::Вход { None } else { match запри_ввод() {
        Ok(з) => Some(з),
        Err(е) => {
            eprintln!("glower-lock: ввод запереть не вышло ({е}) — замок держится одним окном");
            None
        }
    } };
    gtk::main();
    drop(запор);
    std::process::exit(0);
}
