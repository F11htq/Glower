//! Музыка — нативный проигрыватель, настоящим окном (glower-music).
//!
//! Играет mpv без окна: мы говорим с ним по его сокету (JSON построчно) —
//! загрузить, пауза, перемотка, громкость, — а он сам рассказывает, где
//! играет и когда трек кончился. Список — папка «Музыка» и то, что
//! передали при запуске (двойной щелчок по файлу в Проводнике).
//! «Добавить» копирует файлы в «Музыку»: так они не теряются.

use crate::apps::{кнопка, подготовь};
use gtk::glib;
use gtk::prelude::*;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

const СТИЛЬ: &str = r#"
.music .art {
  font-size: 64px;
  min-width: 160px;
  min-height: 160px;
  border-radius: 24px;
  background-image: linear-gradient(140deg, #f472b6, #a855f7);
  color: #ffffff;
}
.music .m-title { font-size: 18px; font-weight: bold; }
.music button.pp { min-width: 54px; min-height: 54px; border-radius: 27px; font-size: 20px; background-image: linear-gradient(140deg, @glower_accent, @glower_accent2); }
.music button.ctl { font-size: 16px; min-width: 42px; min-height: 42px; border-radius: 21px; }
.music list { background-color: transparent; }
.music list row { padding: 6px 10px; border-radius: 8px; }
.music list row:selected { background-color: alpha(@glower_accent, 0.25); }
.music .now { color: #8fb0ff; font-weight: bold; }
"#;

const РАСШИРЕНИЯ: &[&str] = &["mp3", "flac", "ogg", "oga", "opus", "m4a", "aac", "wav", "wma", "webm", "mka"];

fn звук_ли(п: &Path) -> bool {
    п.extension().and_then(|р| р.to_str()).is_some_and(|р| РАСШИРЕНИЯ.contains(&р.to_lowercase().as_str()))
}

/// Где лежит музыка: «Музыка» системы, а нет её — привычные папки
fn папка_музыки() -> PathBuf {
    let дом = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()));
    let корень = std::env::var("GLOWER_ROOT").map(PathBuf::from).unwrap_or_else(|_| дом.join("GlowerOS"));
    for п in [корень.join("Музыка"), glib::user_special_dir(glib::UserDirectory::Music).unwrap_or_default(), дом.join("Музыка"), дом.join("Music")] {
        if !п.as_os_str().is_empty() && п.is_dir() {
            return п;
        }
    }
    корень.join("Музыка")
}

fn собери(папка: &Path, куда: &mut Vec<PathBuf>, глубина: u8) {
    let Ok(ч) = std::fs::read_dir(папка) else { return };
    for з in ч.flatten() {
        let п = з.path();
        if п.is_dir() && глубина < 3 && !з.file_name().to_string_lossy().starts_with('.') {
            собери(&п, куда, глубина + 1);
        } else if п.is_file() && звук_ли(&п) {
            куда.push(п);
        }
    }
}

fn название(п: &Path) -> String {
    п.file_stem().map(|и| и.to_string_lossy().replace('_', " ")).unwrap_or_default()
}

fn мин_сек(с: f64) -> String {
    if !с.is_finite() || с < 0.0 {
        return "0:00".into();
    }
    let с = с as u64;
    format!("{}:{:02}", с / 60, с % 60)
}

/// Связь с mpv: команды пишем в сокет, ответы и события читает поток
struct Плеер {
    сокет: RefCell<Option<UnixStream>>,
    /// Сказанное до того, как mpv отозвался: уйдёт, как только он готов
    ждут: RefCell<Vec<Value>>,
    процесс: RefCell<Option<std::process::Child>>,
}

impl Плеер {
    fn скажи(&self, команда: Value) {
        match self.сокет.borrow_mut().as_mut() {
            Some(с) => {
                let _ = writeln!(с, "{}", json!({ "command": команда }));
            }
            None => self.ждут.borrow_mut().push(команда),
        }
    }

    fn подключён(&self, с: UnixStream) {
        *self.сокет.borrow_mut() = Some(с);
        let ждут = std::mem::take(&mut *self.ждут.borrow_mut());
        for к in ждут {
            self.скажи(к);
        }
    }
}

impl Drop for Плеер {
    fn drop(&mut self) {
        if let Some(mut п) = self.процесс.borrow_mut().take() {
            let _ = п.kill();
            let _ = п.wait();
        }
    }
}

/// Поднять mpv и подключиться к нему. Вести от него идут в канал
fn подними_mpv() -> Result<(Плеер, async_channel::Receiver<Value>, async_channel::Receiver<UnixStream>), String> {
    let папка = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let mut путь = format!("{папка}/glower-music-{}.sock", std::process::id());
    // Адрес сокета не длиннее 108 байт — иначе mpv его просто не заведёт
    if путь.len() > 100 {
        путь = format!("/tmp/glower-music-{}.sock", std::process::id());
    }
    let _ = std::fs::remove_file(&путь);
    let процесс = std::process::Command::new("mpv")
        .args(["--idle=yes", "--no-video", "--no-terminal", "--audio-display=no", &format!("--input-ipc-server={путь}")])
        .spawn()
        .map_err(|_| "На машине нет mpv — играть нечем".to_string())?;
    // mpv поднимается не мгновенно: на слабой машине — секунды. Ждём его
    // в стороне, окно тем временем живёт, а команды копятся
    let (куда, откуда) = async_channel::unbounded();
    let (готов_куда, готов) = async_channel::bounded::<UnixStream>(1);
    std::thread::spawn(move || {
        let mut сокет = None;
        for _ in 0..200 {
            if let Ok(с) = UnixStream::connect(&путь) {
                сокет = Some(с);
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = std::fs::remove_file(&путь);
        let Some(сокет) = сокет else {
            let _ = куда.send_blocking(json!({ "event": "нет-mpv" }));
            return;
        };
        let Ok(запись) = сокет.try_clone() else { return };
        let _ = готов_куда.send_blocking(запись);
        for строка in BufReader::new(сокет).lines() {
            let Ok(строка) = строка else { break };
            if let Ok(в) = serde_json::from_str::<Value>(&строка) {
                if куда.send_blocking(в).is_err() {
                    break;
                }
            }
        }
    });
    let п = Плеер { сокет: RefCell::new(None), ждут: RefCell::new(Vec::new()), процесс: RefCell::new(Some(процесс)) };
    // Где играет и сколько длится — mpv сам сообщает при каждой перемене
    п.скажи(json!(["observe_property", 1, "time-pos"]));
    п.скажи(json!(["observe_property", 2, "duration"]));
    п.скажи(json!(["observe_property", 3, "pause"]));
    Ok((п, откуда, готов))
}

/* ---------- MPRIS: «что играет» для всей системы ----------

   Стандарт freedesktop: проигрыватель объявляет себя на шине сеанса, и
   быстрые настройки, клавиши мультимедиа и любые другие программы видят,
   что играет, и могут нажать «пауза» или «дальше». */

const MPRIS: &str = r#"<node>
  <interface name="org.mpris.MediaPlayer2">
    <method name="Raise"/><method name="Quit"/>
    <property name="CanQuit" type="b" access="read"/><property name="CanRaise" type="b" access="read"/>
    <property name="HasTrackList" type="b" access="read"/><property name="Identity" type="s" access="read"/>
    <property name="DesktopEntry" type="s" access="read"/>
    <property name="SupportedUriSchemes" type="as" access="read"/><property name="SupportedMimeTypes" type="as" access="read"/>
  </interface>
  <interface name="org.mpris.MediaPlayer2.Player">
    <method name="Next"/><method name="Previous"/><method name="Pause"/><method name="PlayPause"/>
    <method name="Stop"/><method name="Play"/>
    <property name="PlaybackStatus" type="s" access="read"/><property name="Metadata" type="a{sv}" access="read"/>
    <property name="CanGoNext" type="b" access="read"/><property name="CanGoPrevious" type="b" access="read"/>
    <property name="CanPlay" type="b" access="read"/><property name="CanPause" type="b" access="read"/>
    <property name="CanControl" type="b" access="read"/><property name="CanSeek" type="b" access="read"/>
  </interface>
</node>"#;

#[derive(Default, Clone)]
struct ЧтоИграет {
    статус: String,
    название: String,
    альбом: String,
}

static ИГРАЕТ: std::sync::Mutex<Option<ЧтоИграет>> = std::sync::Mutex::new(None);
static ШИНА_MPRIS: std::sync::OnceLock<gtk::gio::DBusConnection> = std::sync::OnceLock::new();

fn метаданные(ч: &ЧтоИграет) -> glib::Variant {
    let словарь = glib::VariantDict::new(None);
    словарь.insert_value("mpris:trackid", &glib::Variant::parse(Some(glib::VariantTy::OBJECT_PATH), "'/org/glower/music/track'").unwrap_or_else(|_| "/".to_variant()));
    словарь.insert_value("xesam:title", &ч.название.to_variant());
    словарь.insert_value("xesam:album", &ч.альбом.to_variant());
    словарь.insert_value("xesam:artist", &vec![ч.альбом.clone()].to_variant());
    словарь.end()
}

fn свойство_mpris(имя: &str) -> glib::Variant {
    let ч = ИГРАЕТ.lock().ok().and_then(|ч| ч.clone()).unwrap_or_default();
    match имя {
        "PlaybackStatus" => (if ч.статус.is_empty() { "Stopped" } else { &ч.статус }).to_variant(),
        "Metadata" => метаданные(&ч),
        "Identity" => "Музыка".to_variant(),
        "DesktopEntry" => "glower-music".to_variant(),
        "SupportedUriSchemes" => vec!["file".to_string()].to_variant(),
        "SupportedMimeTypes" => vec!["audio/mpeg".to_string(), "audio/ogg".to_string(), "audio/flac".to_string()].to_variant(),
        "CanQuit" | "CanRaise" | "CanGoNext" | "CanGoPrevious" | "CanPlay" | "CanPause" | "CanControl" => true.to_variant(),
        _ => false.to_variant(),
    }
}

/// Сказать шине, что поменялось
fn mpris_обнови(статус: &str, название: &str, альбом: &str) {
    if let Ok(mut ч) = ИГРАЕТ.lock() {
        *ч = Some(ЧтоИграет { статус: статус.into(), название: название.into(), альбом: альбом.into() });
    }
    let Some(шина) = ШИНА_MPRIS.get() else { return };
    let перемены = glib::VariantDict::new(None);
    перемены.insert_value("PlaybackStatus", &свойство_mpris("PlaybackStatus"));
    перемены.insert_value("Metadata", &свойство_mpris("Metadata"));
    let доводы = ("org.mpris.MediaPlayer2.Player", перемены.end(), Vec::<String>::new()).to_variant();
    let _ = шина.emit_signal(None, "/org/mpris/MediaPlayer2", "org.freedesktop.DBus.Properties", "PropertiesChanged", Some(&доводы));
}

/// Поднять службу; просьбы с шины приходят словами в канал
fn mpris_служба() -> async_channel::Receiver<String> {
    let (куда, откуда) = async_channel::unbounded::<String>();
    let куда = std::sync::Arc::new(куда);
    gtk::gio::bus_own_name(
        gtk::gio::BusType::Session,
        &format!("org.mpris.MediaPlayer2.glower_music.instance{}", std::process::id()),
        gtk::gio::BusNameOwnerFlags::NONE,
        move |шина, _| {
            let Ok(узел) = gtk::gio::DBusNodeInfo::for_xml(MPRIS) else { return };
            let _ = ШИНА_MPRIS.set(шина.clone());
            for имя in ["org.mpris.MediaPlayer2", "org.mpris.MediaPlayer2.Player"] {
                let Some(описание) = узел.lookup_interface(имя) else { continue };
                let куда = куда.clone();
                let _ = шина.register_object(
                    "/org/mpris/MediaPlayer2",
                    &описание,
                    move |_, _, _, _, метод, _, ответ| {
                        let _ = куда.send_blocking(метод.to_string());
                        ответ.return_value(None);
                    },
                    |_, _, _, _, свойство| свойство_mpris(свойство),
                    |_, _, _, _, _, _| false,
                );
            }
        },
        |_, _| {},
        |_, _| {},
    );
    откуда
}

struct Музыка {
    окно: gtk::Window,
    плеер: Option<Плеер>,
    треки: RefCell<Vec<PathBuf>>,
    номер: Cell<Option<usize>>,
    играет: Cell<bool>,
    длина: Cell<f64>,
    тянем: Cell<bool>,
    список: gtk::ListBox,
    заголовок: gtk::Label,
    подпись: gtk::Label,
    пп: gtk::Button,
    ползунок: gtk::Scale,
    время: gtk::Label,
}

type М = Rc<Музыка>;

fn перечитай(м: &М, ещё: &[PathBuf]) {
    let mut треки = Vec::new();
    собери(&папка_музыки(), &mut треки, 0);
    треки.sort_by_key(|п| название(п).to_lowercase());
    // Переданные при запуске — первыми, и без повторов
    let mut все: Vec<PathBuf> = ещё.iter().filter(|п| звук_ли(п)).cloned().collect();
    for т in треки {
        if !все.contains(&т) {
            все.push(т);
        }
    }
    let играл = м.номер.get().and_then(|н| м.треки.borrow().get(н).cloned());
    *м.треки.borrow_mut() = все;
    м.номер.set(играл.and_then(|п| м.треки.borrow().iter().position(|т| *т == п)));
    нарисуй_список(м);
}

fn нарисуй_список(м: &М) {
    for р in м.список.children() {
        м.список.remove(&р);
    }
    let треки = м.треки.borrow().clone();
    if треки.is_empty() {
        let л = gtk::Label::new(Some("Музыки пока нет. Нажмите «Добавить» или положите файлы в папку «Музыка»."));
        л.set_line_wrap(true);
        л.style_context().add_class("muted");
        л.set_margin_top(20);
        м.список.add(&л);
    }
    for (i, т) in треки.iter().enumerate() {
        let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let знак = gtk::Image::from_icon_name(Some(if Some(i) == м.номер.get() { "media-playback-start-symbolic" } else { "audio-x-generic-symbolic" }), gtk::IconSize::Button);
        ряд.add(&знак);
        let и = gtk::Label::new(Some(&название(т)));
        и.set_xalign(0.0);
        и.set_ellipsize(gtk::pango::EllipsizeMode::End);
        if Some(i) == м.номер.get() {
            и.style_context().add_class("now");
        }
        ряд.add(&и);
        м.список.add(&ряд);
    }
    м.список.show_all();
}

fn играй(м: &М, i: usize) {
    let Some(путь) = м.треки.borrow().get(i).cloned() else { return };
    м.номер.set(Some(i));
    м.длина.set(0.0);
    if let Some(п) = &м.плеер {
        п.скажи(json!(["loadfile", путь.to_string_lossy(), "replace"]));
        п.скажи(json!(["set_property", "pause", false]));
    }
    м.играет.set(true);
    м.заголовок.set_text(&название(&путь));
    м.подпись.set_text(&путь.parent().and_then(|р| р.file_name()).map(|и| и.to_string_lossy().to_string()).unwrap_or_default());
    м.окно.set_title(&format!("{} — Музыка", название(&путь)));
    crate::apps::знак_кнопки(&м.пп, "⏸");
    нарисуй_список(м);
    mpris_обнови("Playing", &название(&путь), &м.подпись.text());
}

fn пауза(м: &М) {
    match м.номер.get() {
        None if !м.треки.borrow().is_empty() => играй(м, 0),
        None => {}
        Some(_) => {
            let теперь = !м.играет.get();
            if let Some(п) = &м.плеер {
                п.скажи(json!(["set_property", "pause", !теперь]));
            }
            м.играет.set(теперь);
            crate::apps::знак_кнопки(&м.пп, if теперь { "⏸" } else { "▶" });
        }
    }
}

fn сосед(м: &М, шаг: i64) {
    let всего = м.треки.borrow().len() as i64;
    if всего == 0 {
        return;
    }
    let i = м.номер.get().map(|н| н as i64 + шаг).unwrap_or(0).rem_euclid(всего);
    играй(м, i as usize);
}

fn добавь(м: &М) {
    let д = gtk::FileChooserNative::new(Some("Добавить музыку"), Some(&м.окно), gtk::FileChooserAction::Open, Some("Добавить"), Some("Отмена"));
    д.set_select_multiple(true);
    let ф = gtk::FileFilter::new();
    ф.set_name(Some("Музыка"));
    ф.add_mime_type("audio/*");
    д.add_filter(ф);
    if д.run() != gtk::ResponseType::Accept {
        return;
    }
    let папка = папка_музыки();
    let _ = std::fs::create_dir_all(&папка);
    for файл in д.files() {
        let Some(путь) = файл.path() else { continue };
        if путь.parent() == Some(папка.as_path()) {
            continue;
        }
        if let Some(имя) = путь.file_name() {
            let mut куда = папка.join(имя);
            let mut n = 2;
            while куда.exists() {
                куда = папка.join(format!("{} ({n}).{}", название(&путь), путь.extension().map(|р| р.to_string_lossy().to_string()).unwrap_or_default()));
                n += 1;
            }
            let _ = std::fs::copy(&путь, &куда);
        }
    }
    перечитай(м, &[]);
}

pub fn запусти(доводы: Vec<String>) -> ! {
    подготовь("glower-music", "Музыка");
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль музыки");
    gtk::StyleContext::add_provider_for_screen(&gtk::gdk::Screen::default().expect("экран"), &стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_title("Музыка");
    окно.set_default_size(460, 640);
    окно.set_icon_name(Some("audio-x-generic"));
    окно.style_context().add_class("glower-app");
    crate::apps::строка_заголовка(&окно);
    окно.style_context().add_class("music");
    окно.connect_destroy(|_| gtk::main_quit());

    let колонка = gtk::Box::new(gtk::Orientation::Vertical, 10);
    колонка.set_margin_top(20);
    колонка.set_margin_bottom(12);
    колонка.set_margin_start(18);
    колонка.set_margin_end(18);
    окно.add(&колонка);

    let обложка = gtk::Image::from_icon_name(Some("audio-x-generic-symbolic"), gtk::IconSize::Dialog);
    обложка.set_pixel_size(56);
    обложка.style_context().add_class("art");
    обложка.set_halign(gtk::Align::Center);
    колонка.add(&обложка);
    let заголовок = gtk::Label::new(Some("Ничего не играет"));
    заголовок.style_context().add_class("m-title");
    заголовок.set_ellipsize(gtk::pango::EllipsizeMode::End);
    колонка.add(&заголовок);
    let подпись = gtk::Label::new(None);
    подпись.style_context().add_class("muted");
    колонка.add(&подпись);

    let ползунок = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 1.0);
    ползунок.set_draw_value(false);
    let время = gtk::Label::new(Some("0:00 / 0:00"));
    время.style_context().add_class("muted");
    колонка.add(&ползунок);
    колонка.add(&время);

    let кнопки = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    кнопки.set_halign(gtk::Align::Center);
    let назад = кнопка("⏮", "Предыдущий");
    назад.style_context().add_class("ctl");
    let пп = кнопка("▶", "Играть / пауза");
    пп.style_context().add_class("pp");
    let вперёд = кнопка("⏭", "Следующий");
    вперёд.style_context().add_class("ctl");
    кнопки.add(&назад);
    кнопки.add(&пп);
    кнопки.add(&вперёд);
    колонка.add(&кнопки);

    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let добавить = кнопка("➕ Добавить", "Скопировать файлы в «Музыку»");
    добавить.style_context().add_class("accent");
    ряд.add(&добавить);
    let папка = кнопка("📁", "Открыть папку «Музыка»");
    ряд.add(&папка);
    let громкость = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    громкость.set_value(80.0);
    громкость.set_draw_value(false);
    громкость.set_hexpand(true);
    громкость.set_tooltip_text(Some("Громкость"));
    ряд.add(&gtk::Image::from_icon_name(Some("audio-volume-high-symbolic"), gtk::IconSize::Button));
    ряд.add(&громкость);
    колонка.add(&ряд);

    let список = gtk::ListBox::new();
    let листание = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    листание.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    листание.set_vexpand(true);
    листание.add(&список);
    колонка.add(&листание);

    let (плеер, вести, готов) = match подними_mpv() {
        Ok((п, в, г)) => (Some(п), Some(в), Some(г)),
        Err(е) => {
            подпись.set_text(&е);
            (None, None, None)
        }
    };
    let м: М = Rc::new(Музыка {
        окно: окно.clone(),
        плеер,
        треки: RefCell::new(Vec::new()),
        номер: Cell::new(None),
        играет: Cell::new(false),
        длина: Cell::new(0.0),
        тянем: Cell::new(false),
        список: список.clone(),
        заголовок,
        подпись,
        пп: пп.clone(),
        ползунок: ползунок.clone(),
        время,
    });
    if let Some(п) = &м.плеер {
        п.скажи(json!(["set_property", "volume", 80]));
    }

    let переданные: Vec<PathBuf> = доводы.iter().filter(|д| !д.starts_with("--")).map(PathBuf::from).collect();
    перечитай(&м, &переданные);

    {
        let м = м.clone();
        список.connect_row_activated(move |_, р| играй(&м, р.index() as usize));
    }
    {
        let м = м.clone();
        пп.connect_clicked(move |_| пауза(&м));
    }
    {
        let м = м.clone();
        назад.connect_clicked(move |_| сосед(&м, -1));
    }
    {
        let м = м.clone();
        вперёд.connect_clicked(move |_| сосед(&м, 1));
    }
    {
        let м = м.clone();
        добавить.connect_clicked(move |_| добавь(&м));
    }
    папка.connect_clicked(|_| {
        let п = папка_музыки();
        let _ = std::fs::create_dir_all(&п);
        let _ = std::process::Command::new("glower-files").arg(&п).spawn();
    });
    {
        let м = м.clone();
        громкость.connect_value_changed(move |г| {
            if let Some(п) = &м.плеер {
                п.скажи(json!(["set_property", "volume", г.value().round()]));
            }
        });
    }
    // Перемотка: пока тянут ползунок, вести mpv его не дёргают
    {
        let м = м.clone();
        ползунок.connect_button_press_event(move |_, _| {
            м.тянем.set(true);
            glib::Propagation::Proceed
        });
    }
    {
        let м = м.clone();
        ползунок.connect_button_release_event(move |п, _| {
            м.тянем.set(false);
            if let Some(пл) = &м.плеер {
                пл.скажи(json!(["seek", п.value(), "absolute"]));
            }
            glib::Propagation::Proceed
        });
    }
    // Пробел — пауза, как в любом проигрывателе
    {
        let м = м.clone();
        окно.connect_key_press_event(move |_, е| {
            if е.keyval() == gtk::gdk::keys::constants::space {
                пауза(&м);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
    }

    if let Some(готов) = готов {
        let м = м.clone();
        glib::MainContext::default().spawn_local(async move {
            if let (Ok(с), Some(п)) = (готов.recv().await, м.плеер.as_ref()) {
                п.подключён(с);
            }
        });
    }
    if let Some(вести) = вести {
        let м = м.clone();
        glib::MainContext::default().spawn_local(async move {
            while let Ok(в) = вести.recv().await {
                match (в["event"].as_str(), в["name"].as_str()) {
                    (Some("property-change"), Some("time-pos")) => {
                        let т = в["data"].as_f64().unwrap_or(0.0);
                        if !м.тянем.get() {
                            м.ползунок.set_value(т);
                        }
                        м.время.set_text(&format!("{} / {}", мин_сек(т), мин_сек(м.длина.get())));
                    }
                    (Some("property-change"), Some("duration")) => {
                        let д = в["data"].as_f64().unwrap_or(0.0);
                        м.длина.set(д);
                        м.ползунок.set_range(0.0, д.max(1.0));
                    }
                    (Some("property-change"), Some("pause")) => {
                        let пауза = в["data"].as_bool() == Some(true);
                        if м.номер.get().is_some() {
                            м.играет.set(!пауза);
                            crate::apps::знак_кнопки(&м.пп, if пауза { "▶" } else { "⏸" });
                            mpris_обнови(if пауза { "Paused" } else { "Playing" }, &м.заголовок.text(), &м.подпись.text());
                        }
                    }
                    (Some("нет-mpv"), _) => м.подпись.set_text("mpv не отозвался — играть нечем"),
                    // Трек доиграл сам — следующий; остановленный нами — нет
                    (Some("end-file"), _) if в["reason"].as_str() == Some("eof") => сосед(&м, 1),
                    // Не заиграло: нет звукового устройства, файл битый. Раньше
                    // кнопка так и показывала «пауза», а время стояло на нуле,
                    // и было непонятно, что случилось. Говорим словами
                    (Some("end-file"), _) if в["reason"].as_str() == Some("error") => {
                        м.играет.set(false);
                        crate::apps::знак_кнопки(&м.пп, "▶");
                        let причина = в["file_error"].as_str().unwrap_or("");
                        м.подпись.set_text(if причина.contains("audio output") {
                            "Не играет: не найдено звуковое устройство"
                        } else {
                            "Не играет: файл не открылся"
                        });
                        mpris_обнови("Stopped", &м.заголовок.text(), &м.подпись.text());
                    }
                    _ => {}
                }
            }
        });
    }

    // Просьбы с шины: клавиши мультимедиа, быстрые настройки
    {
        let м = м.clone();
        let просьбы = mpris_служба();
        glib::MainContext::default().spawn_local(async move {
            while let Ok(метод) = просьбы.recv().await {
                match метод.as_str() {
                    "PlayPause" => пауза(&м),
                    "Play" if !м.играет.get() => пауза(&м),
                    "Pause" | "Stop" if м.играет.get() => пауза(&м),
                    "Next" => сосед(&м, 1),
                    "Previous" => сосед(&м, -1),
                    "Raise" => м.окно.present(),
                    "Quit" => м.окно.close(),
                    _ => {}
                }
            }
        });
    }

    окно.show_all();
    if !переданные.is_empty() {
        играй(&м, 0);
    }
    gtk::main();
    drop(м);
    std::process::exit(0);
}
