//! Установка GlowerOS на диск — glower-installer.
//!
//! Живая система всё забывает при перезагрузке; мастер переносит её на
//! настоящий диск: выбор диска, учётная запись машины, подтверждение, ход
//! работы. Всё опасное делает агент (install.*), мастер лишь показывает
//! его отчёт и не даёт выбрать носитель, с которого система загружена.
//!
//! В установочной среде (glower.install=1 у ядра) мастер встаёт во весь
//! экран сам; glower.repair=1 — восстановление: файлы системы заново
//! поверх установленной, личное не трогается.
//!
//! Вид — как у установщиков Windows 11 и macOS: во весь экран — карточка
//! посреди фирменных обоев, слева полоса шагов, справа сам шаг. Пока
//! пишется диск — крупные проценты и сменяющиеся карточки о том, что
//! умеет система: двадцать минут смотреть на полоску скучно.
//!
//! GLOWER_INSTALLER_DEMO=1 — показ без агента: поддельные диски и
//! установка, которая только делает вид. Для снимков и проверки вида; на
//! диск в нём не пишется ничего.

use crate::agent::{зов, позови};
use crate::apps;
use gtk::glib;
use gtk::prelude::*;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

const СТИЛЬ: &str = r#"
/* Во весь экран — фирменные обои, на них карточка */
window.installer.full {
  background-color: #0b0d1a;
  background-image:
    radial-gradient(circle at 18% 85%, rgba(255, 140, 60, 0.55), transparent 38%),
    radial-gradient(circle at 85% 18%, rgba(40, 200, 200, 0.40), transparent 40%),
    radial-gradient(circle at 70% 90%, rgba(170, 70, 230, 0.55), transparent 45%),
    linear-gradient(160deg, #3b2a8f 0%, #2a2160 40%, #4a1f5c 75%, #22113a 100%);
}
.installer.full .inst-frame {
  border-radius: 22px;
  background-color: @glower_bg;
  box-shadow: 0 24px 70px rgba(0, 0, 0, 0.55), 0 0 0 1px rgba(255, 255, 255, 0.10);
}
.installer.full .side { border-radius: 22px 0 0 22px; }
window.installer.csd:not(.maximized):not(.full) .side { border-radius: 0 0 0 10px; }
.installer .side {
  background-image: linear-gradient(170deg, alpha(@glower_accent, 0.16), alpha(@glower_accent2, 0.06) 60%, transparent);
  background-color: alpha(@glower_fg, 0.025);
  border-right: 1px solid alpha(@glower_fg, 0.06);
  padding: 26px 16px 18px 18px;
}
.installer button.try { background-color: transparent; border-radius: 12px; padding: 8px 10px; box-shadow: inset 0 0 0 1px alpha(@glower_fg, 0.10); }
.installer button.try:hover { background-color: alpha(@glower_fg, 0.08); }
.installer .brand { font-size: 20px; font-weight: 600; }
.installer .ver { font-size: 11px; opacity: 0.5; }
.installer .step { padding: 7px 8px; border-radius: 10px; }
.installer .step label.num {
  min-width: 26px; min-height: 26px; border-radius: 13px;
  font-size: 12px; font-weight: 700;
  background-color: alpha(@glower_fg, 0.08);
  color: alpha(@glower_fg, 0.6);
}
.installer .step image.num { min-width: 26px; min-height: 26px; border-radius: 13px; background-color: alpha(@glower_accent, 0.25); color: @glower_accent; }
.installer .step .name { opacity: 0.55; }
.installer .step.on { background-color: alpha(@glower_fg, 0.07); }
.installer .step.on label.num { background-image: linear-gradient(135deg, @glower_accent, @glower_accent2); color: #ffffff; }
.installer .step.on .name { opacity: 1; font-weight: 600; }
.installer .step.done .name { opacity: 0.85; }
.installer .step.bad label.num { background-image: none; background-color: #e5484d; color: #ffffff; }
.installer .body { padding: 34px 40px 10px 40px; }
.installer .foot { padding: 14px 22px; border-top: 1px solid alpha(@glower_fg, 0.07); }
.installer .foot button { padding: 8px 18px; border-radius: 10px; }
.installer .foot button.accent { padding: 8px 26px; font-weight: 600; }
.installer .h1 { font-size: 30px; font-weight: 300; }
.installer .h2 { font-size: 22px; font-weight: 300; }
.installer .lead { opacity: 0.8; font-size: 14px; }
.installer .note { background-color: alpha(@glower_accent, 0.12); border-radius: 12px; padding: 12px 14px; }
.installer .warn { background-color: rgba(255, 90, 90, 0.13); border-radius: 12px; padding: 12px 14px; color: #ffb4b4; }
.installer .card {
  background-color: alpha(@glower_fg, 0.045);
  border-radius: 14px;
  padding: 14px;
  box-shadow: inset 0 0 0 1px alpha(@glower_fg, 0.06);
}
.installer .feat-name { font-weight: 600; }
.installer .circle { min-width: 40px; min-height: 40px; border-radius: 20px; color: #ffffff; }
.installer .circle.c1 { background-image: linear-gradient(135deg, #f59e0b, #ef4444); }
.installer .circle.c2 { background-image: linear-gradient(135deg, #a855f7, #6366f1); }
.installer .circle.c3 { background-image: linear-gradient(135deg, #22c55e, #0ea5e9); }
.installer .circle.acc { background-image: linear-gradient(135deg, @glower_accent, @glower_accent2); }
.installer .circle.small { min-width: 32px; min-height: 32px; border-radius: 16px; }
.installer .circle.huge { min-width: 88px; min-height: 88px; border-radius: 44px; }
.installer .circle.ok { background-image: linear-gradient(135deg, #22c55e, #14b8a6); box-shadow: 0 10px 30px rgba(34, 197, 94, 0.35); }
.installer .circle.bad { background-image: linear-gradient(135deg, #ef4444, #f97316); box-shadow: 0 10px 30px rgba(239, 68, 68, 0.35); }
.installer button.disk {
  padding: 14px 16px; border-radius: 14px;
  background-color: alpha(@glower_fg, 0.045);
  box-shadow: inset 0 0 0 1px alpha(@glower_fg, 0.07);
}
.installer button.disk:hover { background-color: alpha(@glower_fg, 0.09); }
.installer button.disk.on { background-color: alpha(@glower_accent, 0.18); box-shadow: inset 0 0 0 2px @glower_accent; }
.installer button.disk .disk-ico { min-width: 48px; min-height: 48px; border-radius: 12px; background-color: alpha(@glower_fg, 0.08); }
.installer button.disk.on .disk-ico { background-image: linear-gradient(135deg, @glower_accent, @glower_accent2); color: #ffffff; }
.installer button.disk .disk-size { font-size: 18px; font-weight: 300; }
.installer button.disk .tick { color: @glower_accent; }
.installer button:disabled { opacity: 0.4; }
.installer .sum-k { opacity: 0.6; }
.installer .sum-row { padding: 9px 4px; border-bottom: 1px solid alpha(@glower_fg, 0.05); }
.installer .percent { font-size: 56px; font-weight: 200; }
.installer progressbar trough { min-height: 8px; border-radius: 4px; background-color: alpha(@glower_fg, 0.08); }
.installer progressbar progress { min-height: 8px; border-radius: 4px; background-image: linear-gradient(90deg, @glower_accent, @glower_accent2); }
.installer .slide { padding: 18px; border-radius: 16px; background-image: linear-gradient(135deg, alpha(@glower_accent, 0.16), alpha(@glower_accent2, 0.08)); box-shadow: inset 0 0 0 1px alpha(@glower_fg, 0.07); }
.installer .slide-title { font-size: 16px; font-weight: 600; }
.installer .dots label { font-size: 8px; color: alpha(@glower_fg, 0.25); }
.installer .dots label.on { color: @glower_accent; }
.installer textview, .installer textview text { background-color: #0f1117; font-family: monospace; font-size: 11px; }
"#;

#[derive(Default)]
struct Состояние {
    шаг: usize,
    диск: Option<String>,
    диски: Vec<Value>,
    пароль: String,
    имя_машины: String,
    шифр: bool,
    пароль_диска: String,
    можно: Option<Value>,
    чиним: bool,
}

struct Мастер {
    окно: gtk::Window,
    тело: gtk::Box,
    шаги: gtk::Box,
    назад: gtk::Button,
    далее: gtk::Button,
    с: Состояние,
}

thread_local! {
    static М: RefCell<Option<Rc<RefCell<Мастер>>>> = const { RefCell::new(None) };
    static НАЧАЛО_ПОКАЗА: RefCell<Option<Instant>> = const { RefCell::new(None) };
}

fn с_мастером<R>(дело: impl FnOnce(&mut Мастер) -> R) -> Option<R> {
    let м = М.with(|м| м.borrow().clone())?;
    let mut м = м.borrow_mut();
    Some(дело(&mut м))
}

fn в_ядре(флаг: &str) -> bool {
    std::fs::read_to_string("/proc/cmdline").map(|с| с.split_whitespace().any(|ч| ч == флаг)).unwrap_or(false)
}

fn размер(байт: f64) -> String {
    let г = байт / 1e9;
    if г >= 1000.0 {
        format!("{:.1} ТБ", г / 1000.0)
    } else {
        format!("{} ГБ", г.round())
    }
}

// ------------------------------------------------------------- показ

fn показ() -> bool {
    std::env::var_os("GLOWER_INSTALLER_DEMO").is_some()
}

/// Что ответил бы агент в показе
fn ответ_показа(метод: &str) -> Result<Value, String> {
    match метод {
        "install.can" => Ok(json!({ "allowed": true, "reason": null })),
        "install.disks" => Ok(json!({ "list": [
            { "dev": "/dev/nvme0n1", "model": "Samsung SSD 980 PRO", "size": 1.0e12, "kind": "твердотельный" },
            { "dev": "/dev/sda", "model": "WDC WD20EZBX", "size": 2.0e12, "kind": "диск" },
            { "dev": "/dev/sdb", "model": "SanDisk Ultra", "size": 32.0e9, "kind": "твердотельный", "removable": true, "live": true },
        ] })),
        "install.start" => {
            НАЧАЛО_ПОКАЗА.with(|н| *н.borrow_mut() = Some(Instant::now()));
            Ok(json!({}))
        }
        "install.state" => {
            let прошло = НАЧАЛО_ПОКАЗА.with(|н| н.borrow().map(|н| н.elapsed().as_secs_f64())).unwrap_or(0.0);
            let доля = (прошло / 45.0).min(1.0);
            let шаг = ["Размечаю диск", "Создаю файловые системы", "Копирую систему", "Копирую систему", "Ставлю загрузчик", "Последние штрихи"][((доля * 6.0) as usize).min(5)];
            Ok(json!({ "percent": доля * 100.0, "step": шаг, "log": format!("показ: {шаг}…"), "running": доля < 1.0, "ok": true }))
        }
        _ => Err("в показе такого нет".into()),
    }
}

fn спроси(метод: &'static str, доводы: Value, дело: impl FnOnce(Result<Value, String>) + 'static) {
    if показ() {
        let о = ответ_показа(метод);
        glib::timeout_add_local_once(Duration::from_millis(250), move || дело(о));
        return;
    }
    позови(метод, доводы, дело);
}

// ------------------------------------------------------------- окно

pub fn запусти(доводы: Vec<String>) -> ! {
    apps::подготовь("glower-installer", "Установка GlowerOS");
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль установщика");
    gtk::StyleContext::add_provider_for_screen(&gtk::gdk::Screen::default().expect("экран"), &стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);

    let во_весь_экран = доводы.iter().any(|д| д == "--install") || в_ядре("glower.install=1");
    let чиним = доводы.iter().any(|д| д == "--repair") || в_ядре("glower.repair=1");

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_title(if чиним { "Восстановление GlowerOS" } else { "Установка GlowerOS" });
    окно.set_icon_name(Some("drive-harddisk"));
    окно.set_default_size(900, 600);
    окно.style_context().add_class("glower-app");
    crate::apps::строка_заголовка(&окно);
    окно.style_context().add_class("installer");
    if во_весь_экран {
        окно.style_context().add_class("full");
        // Фон — обои системы по умолчанию, чуть притемнённые под карточкой
        let обои = crate::desktop::цвета_для_gtk(&crate::core::обои_по_имени("glower"));
        let с = gtk::CssProvider::new();
        let css = format!("window.installer.full {{ background-image: linear-gradient(rgba(5,8,16,.25), rgba(5,8,16,.25)), {обои}; background-size: cover; background-position: center; }}");
        if с.load_from_data(css.as_bytes()).is_ok() {
            окно.style_context().add_provider(&с, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2);
        }
    }
    окно.connect_destroy(|_| gtk::main_quit());
    // Пока идёт запись на диск, окно не закрыть: оборванная установка
    // оставляет диск размеченным наполовину
    окно.connect_delete_event(move |_, _| {
        if с_мастером(|м| м.с.шаг == 4) == Some(true) {
            return glib::Propagation::Stop;
        }
        // С флешки установщик — это весь сеанс: закрыть его значит уйти на
        // рабочий стол, и об этом лучше спросить, как при Esc
        if во_весь_экран && !чиним {
            glib::idle_add_local_once(попробуй);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });

    // Рамка: слева шаги, справа сам шаг и кнопки
    let рамка = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    рамка.style_context().add_class("inst-frame");

    let бок = gtk::Box::new(gtk::Orientation::Vertical, 4);
    бок.style_context().add_class("side");
    бок.set_size_request(220, -1);
    let знак = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    if let Some(л) = crate::setup::логотип(36) {
        знак.add(&gtk::Image::from_pixbuf(Some(&л)));
    }
    let имя = gtk::Box::new(gtk::Orientation::Vertical, 2);
    match crate::apps::картинка_системы("logo-word.png", 120, 24) {
        Some(н) => {
            let н = gtk::Image::from_pixbuf(Some(&н));
            н.set_halign(gtk::Align::Start);
            имя.add(&н);
        }
        None => имя.add(&метка("GlowerOS", "brand")),
    }
    имя.add(&метка(&format!("{} {}", crate::i18n::tr("Версия"), crate::core::версия()), "ver"));
    имя.set_valign(gtk::Align::Center);
    знак.add(&имя);
    знак.set_margin_bottom(22);
    бок.add(&знак);
    let шаги = gtk::Box::new(gtk::Orientation::Vertical, 4);
    бок.add(&шаги);
    if во_весь_экран && !чиним {
        // Попробовать, не устанавливая — кнопкой внизу полосы шагов (и Esc)
        let проба = gtk::Button::new();
        проба.style_context().add_class("try");
        let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let з = gtk::Image::from_icon_name(Some("media-playback-start-symbolic"), gtk::IconSize::Button);
        ряд.add(&з);
        let к = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let надпись = метка("Попробовать без установки", "");
        надпись.set_max_width_chars(18);
        к.add(&надпись);
        к.add(&метка("Esc", "ver"));
        ряд.add(&к);
        проба.add(&ряд);
        проба.set_valign(gtk::Align::End);
        проба.set_vexpand(true);
        проба.connect_clicked(|_| попробуй());
        бок.add(&проба);
    }

    let правая = gtk::Box::new(gtk::Orientation::Vertical, 0);
    правая.set_hexpand(true);
    let тело = gtk::Box::new(gtk::Orientation::Vertical, 16);
    тело.style_context().add_class("body");
    тело.set_vexpand(true);
    let низ = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    низ.style_context().add_class("foot");
    let назад = gtk::Button::with_label("Назад");
    let далее = gtk::Button::with_label("Далее");
    далее.style_context().add_class("accent");
    низ.pack_start(&назад, false, false, 0);
    низ.pack_end(&далее, false, false, 0);
    // Тело — в листании: как бы ни выросло содержимое, окно не вытянется
    // за экран и кнопки внизу останутся на месте
    let листание = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    листание.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    листание.set_vexpand(true);
    листание.add(&тело);
    правая.add(&листание);
    правая.add(&низ);

    рамка.add(&бок);
    рамка.add(&правая);
    if во_весь_экран {
        // Карточка посреди обоев — одного размера на любом экране
        рамка.set_halign(gtk::Align::Center);
        рамка.set_valign(gtk::Align::Center);
        рамка.set_size_request(960, 640);
    }
    окно.add(&рамка);

    назад.connect_clicked(|_| {
        с_мастером(|м| м.с.шаг = м.с.шаг.saturating_sub(1));
        нарисуй();
    });
    далее.connect_clicked(|_| вперёд());

    М.with(|я| {
        *я.borrow_mut() = Some(Rc::new(RefCell::new(Мастер {
            окно: окно.clone(),
            тело,
            шаги,
            назад,
            далее,
            с: Состояние { имя_машины: "GlowerOS".into(), чиним, ..Default::default() },
        })))
    });
    // Esc — попробовать систему, не устанавливая. Только с флешки и только
    // до начала записи на диск
    if во_весь_экран && !чиним {
        окно.connect_key_press_event(|_, е| {
            if е.keyval() == gtk::gdk::keys::constants::Escape && с_мастером(|м| м.с.шаг < 4) == Some(true) {
                glib::idle_add_local_once(попробуй);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
    }
    // Во весь экран — до первого показа: тогда окно сразу появляется во
    // весь экран, а не обычным окном, которое потом растягивается
    if во_весь_экран {
        окно.fullscreen();
    }
    окно.show_all();
    нарисуй();
    gtk::main();
    std::process::exit(0);
}

fn очисти() -> Option<gtk::Box> {
    let т = с_мастером(|м| м.тело.clone())?;
    for р in т.children() {
        т.remove(&р);
    }
    Some(т)
}

fn метка(текст: &str, класс: &str) -> gtk::Label {
    let л = gtk::Label::new(Some(текст));
    л.set_xalign(0.0);
    л.set_line_wrap(true);
    // Без предела ширины перенос слов просит у окна высоту с запасом,
    // и окно вырастает за экран вместе с кнопками
    л.set_max_width_chars(80);
    if !класс.is_empty() {
        л.style_context().add_class(класс);
    }
    л
}

fn кнопки(назад: bool, далее: Option<&str>, можно: bool) {
    с_мастером(|м| {
        // Нажимать нечего — нет и нижней строки
        if let Some(низ) = м.далее.parent() {
            низ.set_visible(назад || далее.is_some());
        }
        м.назад.set_visible(назад);
        м.далее.set_visible(далее.is_some());
        if let Some(т) = далее {
            м.далее.set_label(т);
        }
        м.далее.set_sensitive(можно);
    });
}

/// Значок в цветном кружке
fn кружок(значок: &str, размер: i32, классы: &[&str]) -> gtk::Image {
    let к = gtk::Image::from_icon_name(Some(значок), gtk::IconSize::Dialog);
    к.set_pixel_size(размер);
    к.style_context().add_class("circle");
    for класс in классы {
        к.style_context().add_class(класс);
    }
    к.set_valign(gtk::Align::Center);
    к.set_halign(gtk::Align::Start);
    к
}

/// Полоса шагов слева: пройденные — с галочкой, текущий — подсвечен
fn нарисуй_шаги(шаг: usize, чиним: bool) {
    let Some(шаги) = с_мастером(|м| м.шаги.clone()) else { return };
    for р in шаги.children() {
        шаги.remove(&р);
    }
    let имена = [
        "Приветствие",
        "Диск",
        "Учётная запись",
        "Проверка",
        if чиним { "Восстановление" } else { "Установка" },
        "Готово",
    ];
    // Беда (6) — на месте установки, красным
    let (текущий, беда) = if шаг == 6 { (4, true) } else { (шаг, false) };
    for (i, имя) in имена.iter().enumerate() {
        let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        ряд.style_context().add_class("step");
        let знак: gtk::Widget = if i < текущий {
            ряд.style_context().add_class("done");
            let к = gtk::Image::from_icon_name(Some("object-select-symbolic"), gtk::IconSize::Button);
            к.set_pixel_size(14);
            к.style_context().add_class("num");
            к.upcast()
        } else {
            let н = gtk::Label::new(Some(&(i + 1).to_string()));
            н.style_context().add_class("num");
            н.upcast()
        };
        if i == текущий {
            ряд.style_context().add_class("on");
            if беда {
                ряд.style_context().add_class("bad");
            }
        }
        ряд.add(&знак);
        let п = gtk::Label::new(Some(имя));
        п.style_context().add_class("name");
        п.set_xalign(0.0);
        ряд.add(&п);
        шаги.add(&ряд);
    }
    шаги.show_all();
}

/// Шаг появляется плавно, а не скачком
fn проявись(т: &gtk::Box) {
    т.set_opacity(0.0);
    let начало = Instant::now();
    т.add_tick_callback(move |т, _| {
        let доля = (начало.elapsed().as_secs_f64() / 0.22).min(1.0);
        т.set_opacity(1.0 - (1.0 - доля).powi(3));
        if доля >= 1.0 {
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}

fn нарисуй() {
    let Some(т) = очисти() else { return };
    let шаг = с_мастером(|м| м.с.шаг).unwrap_or(0);
    let чиним = с_мастером(|м| м.с.чиним).unwrap_or(false);
    нарисуй_шаги(шаг, чиним);
    проявись(&т);
    match шаг {
        0 => {
            if let Some(л) = crate::setup::логотип(76) {
                let к = gtk::Image::from_pixbuf(Some(&л));
                к.set_halign(gtk::Align::Start);
                т.add(&к);
            }
            т.add(&метка(if чиним { "Восстановление GlowerOS" } else { "Добро пожаловать в GlowerOS" }, "h1"));
            т.add(&метка(
                if чиним {
                    "Системные файлы будут положены заново поверх установленной системы. Личные файлы и папка пользователя останутся на месте."
                } else {
                    "Сейчас система работает из памяти и всё забывает при выключении. Установка перенесёт её на диск: настройки, файлы и учётная запись начнут сохраняться."
                },
                "lead",
            ));
            if !чиним {
                let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                ряд.set_homogeneous(true);
                for (значок, класс, имя, под) in [
                    ("system-run-symbolic", "c1", "Быстрая", "Включается за секунды"),
                    ("preferences-desktop-appearance-symbolic", "c2", "Красивая", "Темы, акценты и обои"),
                    ("security-high-symbolic", "c3", "Надёжная", "Шифрование всего диска"),
                ] {
                    let к = gtk::Box::new(gtk::Orientation::Vertical, 8);
                    к.style_context().add_class("card");
                    к.add(&кружок(значок, 20, &[класс]));
                    к.add(&метка(имя, "feat-name"));
                    к.add(&метка(под, "muted"));
                    ряд.add(&к);
                }
                т.add(&ряд);
            }
            кнопки(false, Some("Начать"), false);
            let место = т.clone();
            let ждём = метка("Проверяю, можно ли ставить…", "muted");
            т.add(&ждём);
            спроси("install.can", json!({}), move |о| {
                место.remove(&ждём);
                let о = о.unwrap_or_else(|е| json!({ "allowed": false, "reason": е }));
                let можно = о["allowed"].as_bool() == Some(true) && о["reason"].is_null();
                с_мастером(|м| м.с.можно = Some(о.clone()));
                if можно {
                    место.add(&метка(
                        if чиним { "Разметка диска не меняется. Заменяются только файлы самой системы." } else { "Диск, который вы выберете, будет очищен полностью. Всё, что на нём есть, исчезнет." },
                        "note",
                    ));
                } else {
                    место.add(&метка(&format!("Установка недоступна: {}", о["reason"].as_str().unwrap_or("причина неизвестна")), "warn"));
                }
                место.show_all();
                кнопки(false, Some("Начать"), можно);
            });
        }
        1 => {
            т.add(&метка("Куда установить", "h2"));
            т.add(&метка("Выберите диск. Носитель, с которого запущена система, выбрать нельзя.", "muted"));
            let список = gtk::Box::new(gtk::Orientation::Vertical, 10);
            т.add(&список);
            список.add(&метка("Ищу диски…", "muted"));
            кнопки(true, Some("Далее"), false);
            спроси("install.disks", json!({}), move |о| {
                for р in список.children() {
                    список.remove(&р);
                }
                match о {
                    Err(е) => список.add(&метка(&е, "warn")),
                    Ok(о) => {
                        let диски = о["list"].as_array().cloned().unwrap_or_default();
                        let выбран = с_мастером(|м| {
                            м.с.диски = диски.clone();
                            м.с.диск.clone()
                        })
                        .flatten();
                        if диски.is_empty() {
                            список.add(&метка("Дисков не найдено", "warn"));
                        }
                        for д in диски {
                            список.add(&плитка_диска(&д, выбран.as_deref()));
                        }
                        кнопки(true, Some("Далее"), выбран.is_some());
                    }
                }
                список.show_all();
            });
        }
        2 => {
            т.add(&метка("Учётная запись машины", "h2"));
            let (пароль, имя, шифр, пароль_диска) = с_мастером(|м| (м.с.пароль.clone(), м.с.имя_машины.clone(), м.с.шифр, м.с.пароль_диска.clone())).unwrap_or_default();
            let сетка = gtk::Grid::new();
            сетка.set_row_spacing(14);
            сетка.set_column_spacing(16);
            let п1 = gtk::Entry::new();
            п1.set_visibility(false);
            п1.set_placeholder_text(Some("Пароль (необязательно)"));
            п1.set_text(&пароль);
            п1.set_hexpand(true);
            п1.connect_changed(|п| {
                let т = п.text().to_string();
                с_мастером(|м| м.с.пароль = т);
            });
            let хн = gtk::Entry::new();
            хн.set_text(&имя);
            хн.set_placeholder_text(Some("Имя машины"));
            хн.connect_changed(|п| {
                let т = п.text().to_string();
                с_мастером(|м| м.с.имя_машины = т);
            });
            let строка = |r: i32, значок: &str, имя: &str, пояснение: &str, поле: &gtk::Widget| {
                let л = gtk::Box::new(gtk::Orientation::Horizontal, 12);
                л.add(&кружок(значок, 16, &["acc", "small"]));
                let к = gtk::Box::new(gtk::Orientation::Vertical, 2);
                к.add(&метка(имя, "feat-name"));
                let п = метка(пояснение, "muted");
                // Ширина с самого начала: узкая колонка с переносом просит
                // у окна огромную высоту, и кнопки уезжают за край экрана
                п.set_width_chars(40);
                п.set_max_width_chars(42);
                к.add(&п);
                л.add(&к);
                сетка.attach(&л, 0, r, 1, 1);
                поле.set_valign(gtk::Align::Center);
                сетка.attach(поле, 1, r, 1, 1);
            };
            строка(0, "dialog-password-symbolic", "Пароль системной записи", "Нужен для входа в консоль и для sudo. Можно оставить пустым, как в живой системе.", п1.upcast_ref());
            строка(1, "computer-symbolic", "Имя машины", "Так система будет представляться в сети", хн.upcast_ref());
            // Шифрование: пароль входа защищает от того, кто сядет за машину,
            // шифрование — от того, кто унесёт диск
            let флажок = gtk::Switch::new();
            флажок.set_active(шифр);
            флажок.set_halign(gtk::Align::Start);
            строка(2, "security-high-symbolic", "Зашифровать диск", "Система будет спрашивать пароль при включении. Без него файлы не прочитать даже вынув диск.", флажок.upcast_ref());
            let п2 = gtk::Entry::new();
            п2.set_visibility(false);
            п2.set_placeholder_text(Some("Пароль диска (не короче 8 знаков)"));
            п2.set_text(&пароль_диска);
            строка(3, "dialog-password-symbolic", "Пароль диска", "Отдельный от пароля записи. Забыть его нельзя: восстановить данные будет нечем.", п2.upcast_ref());
            let карточка = gtk::Box::new(gtk::Orientation::Vertical, 0);
            карточка.style_context().add_class("card");
            карточка.add(&сетка);
            т.add(&карточка);
            let беда = метка("", "warn");
            т.add(&беда);
            т.add(&метка("Имя пользователя и оформление вы зададите при первом запуске установленной системы.", "note"));
            let проверь = {
                let (беда, сетка) = (беда.clone(), сетка.clone());
                move || {
                    let (шифр, п) = с_мастером(|м| (м.с.шифр, м.с.пароль_диска.clone())).unwrap_or_default();
                    // Пароль диска набирают до загрузки системы, где раскладок
                    // ещё нет: русские буквы в нём — сундук без ключа
                    let не_латиница = п.chars().any(|c| !(' '..='~').contains(&c));
                    беда.set_text("Пароль диска можно набрать только латиницей: при включении машины раскладки ещё нет.");
                    беда.set_visible(шифр && не_латиница);
                    for r in [3] {
                        for к in 0..2 {
                            if let Some(в) = сетка.child_at(к, r) {
                                в.set_visible(шифр);
                            }
                        }
                    }
                    кнопки(true, Some("Далее"), !(шифр && (п.chars().count() < 8 || не_латиница)));
                }
            };
            {
                let проверь = проверь.clone();
                флажок.connect_active_notify(move |ф| {
                    let в = ф.is_active();
                    с_мастером(|м| м.с.шифр = в);
                    проверь();
                });
            }
            {
                let проверь = проверь.clone();
                п2.connect_changed(move |п| {
                    let т = п.text().to_string();
                    с_мастером(|м| м.с.пароль_диска = т);
                    проверь();
                });
            }
            т.show_all();
            проверь();
            return;
        }
        3 => {
            let (с_диск, диски, имя, пароль, шифр) = с_мастером(|м| (м.с.диск.clone().unwrap_or_default(), м.с.диски.clone(), м.с.имя_машины.clone(), !м.с.пароль.is_empty(), м.с.шифр)).unwrap_or_default();
            let д = диски.iter().find(|д| д["dev"].as_str() == Some(&с_диск)).cloned().unwrap_or_default();
            т.add(&метка("Проверьте перед началом", "h2"));
            let карточка = gtk::Box::new(gtk::Orientation::Vertical, 0);
            карточка.style_context().add_class("card");
            for (значок, к, в) in [
                (значок_диска(&д), "Диск", д["model"].as_str().filter(|м| !м.is_empty()).or(д["name"].as_str()).unwrap_or(&с_диск).to_string()),
                ("media-flash-symbolic", "Устройство", с_диск.clone()),
                ("drive-harddisk-symbolic", "Объём", размер(д["size"].as_f64().unwrap_or(0.0))),
                ("computer-symbolic", "Имя машины", имя),
                ("dialog-password-symbolic", "Пароль", if пароль { "задан".into() } else { "без пароля".into() }),
                ("security-high-symbolic", "Шифрование", if шифр { "диск будет зашифрован".into() } else { "нет".into() }),
            ] {
                let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 12);
                ряд.style_context().add_class("sum-row");
                let з = gtk::Image::from_icon_name(Some(значок), gtk::IconSize::Button);
                з.set_pixel_size(16);
                з.set_opacity(0.7);
                ряд.add(&з);
                let кл = метка(к, "sum-k");
                кл.set_width_chars(14);
                ряд.add(&кл);
                let л = gtk::Label::new(None);
                л.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(&в)));
                л.set_xalign(0.0);
                ряд.add(&л);
                карточка.add(&ряд);
            }
            т.add(&карточка);
            т.add(&метка(
                &if чиним {
                    format!("Разметка и личные файлы не трогаются. Заменяются файлы системы: {с_диск}.")
                } else {
                    format!("Диск {с_диск} будет стёрт полностью, вместе со всеми разделами и данными. Отменить это будет нечем.")
                },
                if чиним { "note" } else { "warn" },
            ));
            кнопки(true, Some(if чиним { "Восстановить систему" } else { "Стереть диск и установить" }), true);
        }
        4 => установка(&т),
        5 => {
            let к = кружок("object-select-symbolic", 44, &["huge", "ok"]);
            к.set_margin_bottom(6);
            т.add(&к);
            let диск = с_мастером(|м| м.с.диск.clone().unwrap_or_default()).unwrap_or_default();
            т.add(&метка(if чиним { "Система восстановлена" } else { "Система установлена" }, "h1"));
            т.add(&метка(
                &format!("GlowerOS перенесена на {диск}. Нажмите «Перезагрузить» — и выньте установочный носитель, когда экран погаснет. Дальше машина будет грузиться с диска и запомнит всё, что вы настроите."),
                "lead",
            ));
            т.add(&метка("Носитель нужен до самой перезагрузки: пока она не началась, система работает с него и без него остановится.", "note"));
            кнопки(false, Some("Перезагрузить"), true);
        }
        _ => {}
    }
    т.show_all();
}

fn значок_диска(д: &Value) -> &'static str {
    if д["removable"].as_bool() == Some(true) {
        "drive-removable-media-symbolic"
    } else if д["kind"].as_str() == Some("твердотельный") {
        "drive-harddisk-solidstate-symbolic"
    } else {
        "drive-harddisk-symbolic"
    }
}

/// Диск плиткой: значок, имя, подробности, объём крупно справа
fn плитка_диска(д: &Value, выбран: Option<&str>) -> gtk::Button {
    let dev = д["dev"].as_str().unwrap_or("").to_string();
    let живой = д["live"].as_bool() == Some(true);
    let б = gtk::Button::new();
    б.style_context().add_class("disk");
    let вкл = выбран == Some(dev.as_str());
    if вкл {
        б.style_context().add_class("on");
    }
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    let з = gtk::Image::from_icon_name(Some(значок_диска(д)), gtk::IconSize::Dialog);
    з.set_pixel_size(26);
    з.style_context().add_class("disk-ico");
    ряд.add(&з);
    let к = gtk::Box::new(gtk::Orientation::Vertical, 3);
    к.set_valign(gtk::Align::Center);
    к.set_hexpand(true);
    let имя = д["model"].as_str().filter(|м| !м.is_empty()).or(д["name"].as_str()).unwrap_or(&dev).to_string();
    let и = gtk::Label::new(None);
    и.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(&имя)));
    и.set_xalign(0.0);
    к.add(&и);
    let вид = д["kind"].as_str().map(crate::i18n::tr).unwrap_or_default();
    к.add(&метка(&format!("{dev} · {вид}{}", if живой { crate::i18n::tr(" · носитель системы") } else { String::new() }), "muted"));
    ряд.add(&к);
    let объём = метка(&размер(д["size"].as_f64().unwrap_or(0.0)), "disk-size");
    объём.set_valign(gtk::Align::Center);
    ряд.add(&объём);
    let галка = gtk::Image::from_icon_name(Some("object-select-symbolic"), gtk::IconSize::Button);
    галка.style_context().add_class("tick");
    галка.set_opacity(if вкл { 1.0 } else { 0.0 });
    ряд.add(&галка);
    б.add(&ряд);
    б.set_sensitive(!живой);
    б.connect_clicked(move |_| {
        с_мастером(|м| м.с.диск = Some(dev.clone()));
        нарисуй();
    });
    б
}

/// Пробный режим: установщик закрывается, а сеть и остальные службы,
/// выключенные ради быстрой загрузки, поднимаются без перезагрузки
fn попробуй() {
    if !подтверди(
        "Попробовать GlowerOS без установки?",
        "Установщик закроется, включатся сеть и остальные службы, и откроется рабочий стол. На диск ничего не запишется, а всё сделанное забудется при выключении. Вернуться к установке можно значком «Установка GlowerOS» в меню «Пуск».",
        "Попробовать",
    ) {
        return;
    }
    if let Err(е) = std::process::Command::new("sudo").args(["-n", "/usr/bin/glower-try"]).spawn() {
        eprintln!("glower-installer: пробный режим: {е}");
    }
    if let Some(окно) = с_мастером(|м| м.окно.clone()) {
        unsafe { окно.destroy() };
    }
}

fn подтверди(заголовок: &str, текст: &str, кнопка: &str) -> bool {
    let Some(окно) = с_мастером(|м| м.окно.clone()) else { return false };
    let д = gtk::MessageDialog::new(Some(&окно), gtk::DialogFlags::MODAL, gtk::MessageType::Warning, gtk::ButtonsType::None, заголовок);
    д.set_secondary_text(Some(текст));
    д.add_button("Отмена", gtk::ResponseType::Cancel);
    д.add_button(кнопка, gtk::ResponseType::Accept);
    let ответ = д.run();
    unsafe { д.destroy() };
    ответ == gtk::ResponseType::Accept
}

fn вперёд() {
    let Some((шаг, чиним, диск)) = с_мастером(|м| (м.с.шаг, м.с.чиним, м.с.диск.clone().unwrap_or_default())) else { return };
    match шаг {
        3 => {
            let да = if чиним {
                подтверди(&format!("Восстановить систему на {диск}?"), "Файлы самой системы будут заменены. Личные файлы останутся на месте.", "Восстановить")
            } else {
                подтверди(&format!("Стереть диск {диск}?"), "Все разделы и данные на этом диске исчезнут. Это необратимо.", "Стереть и установить")
            };
            if !да {
                return;
            }
        }
        5 => return перезагрузи(),
        6 => {
            с_мастером(|м| м.с.шаг = 1);
            return нарисуй();
        }
        _ => {}
    }
    с_мастером(|м| м.с.шаг += 1);
    нарисуй();
}

/// Пока пишется диск — о том, что ждёт в системе
const КАРТОЧКИ: [(&str, &str, &str); 6] = [
    ("view-app-grid-symbolic", "Всё под рукой", "Win открывает меню программ с поиском — и программ, и файлов на диске."),
    ("edit-paste-symbolic", "История буфера", "Win+V — всё, что вы копировали. Нужное можно закрепить."),
    ("media-record-symbolic", "Запись экрана", "Win+Shift+R записывает экран в видео, Win+Shift+S — снимок части экрана."),
    ("preferences-desktop-appearance-symbolic", "По-своему", "Тема, цвет акцента, обои и виджеты на рабочем столе — в Параметрах."),
    ("system-software-install-symbolic", "Магазин", "Программы ставятся в одно нажатие: браузеры, офис, игры, мессенджеры."),
    ("security-high-symbolic", "Ваши данные — ваши", "Без рекламы и слежки. Диск можно зашифровать целиком."),
];

fn установка(т: &gtk::Box) {
    let чиним = с_мастером(|м| м.с.чиним).unwrap_or(false);
    т.add(&метка(if чиним { "Восстанавливаю систему" } else { "Устанавливаю GlowerOS" }, "h2"));
    let проценты = метка("0%", "percent");
    т.add(&проценты);
    let полоса = gtk::ProgressBar::new();
    т.add(&полоса);
    let шаг = метка("Начинаю…", "muted");
    т.add(&шаг);

    // Сменяющиеся карточки
    let смена = gtk::Stack::new();
    смена.set_transition_type(gtk::StackTransitionType::Crossfade);
    смена.set_transition_duration(450);
    смена.set_margin_top(10);
    for (i, (значок, имя, текст)) in КАРТОЧКИ.iter().enumerate() {
        let к = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        к.style_context().add_class("slide");
        к.add(&кружок(значок, 20, &["acc"]));
        let с = gtk::Box::new(gtk::Orientation::Vertical, 4);
        с.set_valign(gtk::Align::Center);
        с.add(&метка(имя, "slide-title"));
        с.add(&метка(текст, "lead"));
        к.add(&с);
        смена.add_named(&к, &i.to_string());
    }
    т.add(&смена);
    let точки = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    точки.style_context().add_class("dots");
    точки.set_halign(gtk::Align::Center);
    for i in 0..КАРТОЧКИ.len() {
        let т = gtk::Label::new(Some("●"));
        if i == 0 {
            т.style_context().add_class("on");
        }
        точки.add(&т);
    }
    т.add(&точки);
    {
        let (смена, точки) = (смена.clone(), точки.clone());
        let mut n = 0usize;
        glib::timeout_add_local(Duration::from_secs(7), move || {
            // Установка кончилась — карточки больше не нужны
            if смена.parent().is_none() {
                return glib::ControlFlow::Break;
            }
            n = (n + 1) % КАРТОЧКИ.len();
            смена.set_visible_child_name(&n.to_string());
            for (i, т) in точки.children().iter().enumerate() {
                if i == n {
                    т.style_context().add_class("on");
                } else {
                    т.style_context().remove_class("on");
                }
            }
            glib::ControlFlow::Continue
        });
    }

    let подробно = gtk::Expander::new(Some("Подробности"));
    let журнал = gtk::TextView::new();
    журнал.set_editable(false);
    журнал.set_monospace(true);
    let л = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    л.set_min_content_height(160);
    л.add(&журнал);
    подробно.add(&л);
    подробно.set_margin_top(6);
    т.add(&подробно);
    // Пока пишется диск, жать нечего — и кнопок нет
    кнопки(false, None, false);
    т.show_all();

    let Some(с) = с_мастером(|м| {
        json!({
            "disk": м.с.диск,
            "password": м.с.пароль,
            "hostname": м.с.имя_машины,
            "repair": м.с.чиним,
            "crypt": м.с.шифр,
            "cryptPassword": if м.с.шифр { json!(м.с.пароль_диска) } else { Value::Null },
            // Пояс, выбранный в Параметрах живой системы, — если выбран
            "tz": std::fs::read_to_string(glib::user_config_dir().join("glower").join("settings.json"))
                .ok()
                .and_then(|т| serde_json::from_str::<Value>(&т).ok())
                .and_then(|н| н["tz"].as_str().filter(|п| !п.is_empty()).map(String::from)),
        })
    }) else {
        return;
    };
    спроси("install.start", с, move |о| {
        if let Err(е) = о {
            return беда(&е);
        }
        glib::timeout_add_local(Duration::from_millis(700), move || {
            let с = if показ() { ответ_показа("install.state") } else { зов("install.state", json!({}), 5) };
            let с = match с {
                Ok(с) => с,
                Err(е) => {
                    беда(&е);
                    return glib::ControlFlow::Break;
                }
            };
            let доля = с["percent"].as_f64().unwrap_or(0.0);
            полоса.set_fraction(доля / 100.0);
            проценты.set_text(&format!("{}%", доля.floor() as i64));
            шаг.set_text(с["step"].as_str().unwrap_or(""));
            журнал.buffer().map(|б| б.set_text(с["log"].as_str().unwrap_or("")));
            if с["running"].as_bool() == Some(true) {
                return glib::ControlFlow::Continue;
            }
            if с["ok"].as_bool() == Some(true) {
                с_мастером(|м| м.с.шаг = 5);
                нарисуй();
            } else {
                беда(с["error"].as_str().unwrap_or("установка прервалась"));
            }
            glib::ControlFlow::Break
        });
    });
}

fn беда(текст: &str) {
    let Some(т) = очисти() else { return };
    с_мастером(|м| м.с.шаг = 6);
    нарисуй_шаги(6, с_мастером(|м| м.с.чиним).unwrap_or(false));
    проявись(&т);
    let к = кружок("dialog-warning-symbolic", 40, &["huge", "bad"]);
    к.set_margin_bottom(6);
    т.add(&к);
    т.add(&метка("Установка не завершилась", "h1"));
    т.add(&метка(текст, "warn"));
    т.add(&метка("Диск мог остаться размеченным наполовину. Можно попробовать снова — установка начинается с полной очистки диска.", "note"));
    кнопки(false, Some("Начать заново"), true);
    т.show_all();
}

/// Перезагрузка после установки — сразу, как от кнопки на корпусе:
/// корень живой системы лежит на носителе, и аккуратное отключение
/// всего подключённого спотыкается, стоит носитель вынуть
fn перезагрузи() {
    кнопки(false, Some("Перезагружаю…"), false);
    if показ() {
        gtk::main_quit();
        return;
    }
    позови("sys.power", json!({ "action": "reboot", "сразу": true }), |о| {
        if let Err(е) = о {
            беда(&format!("Перезагрузка не удалась: {е}. Выключите машину кнопкой и включите заново — система на диске уже готова."));
        }
    });
}
