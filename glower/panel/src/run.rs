//! «Выполнить» (glower-run, Win+R) — как в Windows: одна строка, и система
//! открывает то, что в ней написано.
//!
//! Понимает:
//! — команды с доводами: `gimp ~/фото.png`, `foot -e htop` (в кавычках —
//!   как в оболочке, но без оболочки: ни `;`, ни `|` здесь ничего не делают);
//! — папки и файлы: `~/Загрузки`, `/etc/hosts`, `.` — домашняя папка;
//!   исполняемый файл запускается, остальное открывается своей программой;
//! — адреса: `https://…`, `www.…`, `mailto:…`;
//! — имена программ из меню: «Калькулятор», «firefox»;
//! — привычные по Windows слова: cmd, notepad, calc, control, taskmgr,
//!   explorer, mspaint, ms-settings:, appwiz.cpl, regedit…
//!
//! Запускает от имени человека, с его правами — как терминал. «С правами
//! администратора» — через pkexec: система спросит пароль сама.
//! Что набирали — помнится (~/.config/glower/выполнить.json) и
//! подсказывается вместе с программами из PATH.

use crate::apps::подготовь;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use serde_json::Value;
use std::path::{Path, PathBuf};

const ИСТОРИЯ: &str = "выполнить.json";

const СТИЛЬ: &str = r#"
.run { padding: 18px; }
.run .lead { opacity: 0.8; }
.run button.main { background-image: none; background-color: @glower_accent; border: none; }
.run button.main label { color: #ffffff; font-weight: 600; }
.run button.main:disabled { opacity: 0.45; }
"#;

/// Что сделать со строкой — решается без запуска, чтобы это можно было
/// проверить
#[derive(Debug, PartialEq)]
pub enum Дело {
    /// Программа и её доводы
    Запуск(Vec<String>),
    /// Открыть своей программой: адрес, файл, папка (gio)
    Открыть(String),
    /// Ярлык программы из меню
    Ярлык(String),
    Ничего,
    Нет(String),
}

/// Привычные по Windows слова — нашими программами
fn слово_windows(с: &str) -> Option<Vec<&'static str>> {
    Some(match с.to_lowercase().trim_end_matches(".exe") {
        "cmd" | "powershell" | "pwsh" | "terminal" | "wt" | "терминал" => vec!["foot"],
        "notepad" | "блокнот" => vec!["glower-notepad"],
        "calc" | "калькулятор" => vec!["glower-calc"],
        "control" | "панель управления" | "control panel" => vec!["glower-control"],
        "taskmgr" | "диспетчер задач" => vec!["glower-taskmgr"],
        "explorer" | "проводник" => vec!["glower-files"],
        "mspaint" | "paint" => vec!["glower-paint"],
        "ms-settings:" | "settings" | "параметры" => vec!["glower-settings"],
        "ms-windows-store:" | "store" | "магазин" => vec!["glower-store"],
        "appwiz.cpl" => vec!["glower-store", "installed"],
        "ncpa.cpl" => vec!["glower-settings", "net"],
        "mmsys.cpl" => vec!["glower-settings", "sound"],
        "desk.cpl" => vec!["glower-settings", "display"],
        "main.cpl" => vec!["glower-settings", "mouse"],
        "timedate.cpl" | "intl.cpl" => vec!["glower-settings", "time"],
        "powercfg.cpl" => vec!["glower-settings", "system"],
        "firewall.cpl" => vec!["glower-settings", "privacy"],
        "sysdm.cpl" | "winver" | "msinfo32" => vec!["glower-settings", "about"],
        "snippingtool" | "ножницы" => vec!["glower-snip"],
        "regedit" => vec!["foot", "-e", "dconf", "watch", "/"],
        "eventvwr" | "eventvwr.msc" => vec!["foot", "-e", "journalctl", "-b", "-e"],
        "services.msc" => vec!["foot", "-e", "systemctl", "list-units", "--type=service"],
        _ => return None,
    })
}

fn в_path(имя: &str) -> Option<PathBuf> {
    if имя.contains('/') {
        let п = PathBuf::from(имя);
        return исполняемый(&п).then_some(п);
    }
    std::env::var_os("PATH").and_then(|п| std::env::split_paths(&п).map(|к| к.join(имя)).find(|п| исполняемый(п)))
}

fn исполняемый(п: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    п.is_file() && п.metadata().is_ok_and(|м| м.permissions().mode() & 0o111 != 0)
}

/// `~/x` → домашняя папка; `.` — сама домашняя папка
fn раскрой(путь: &str, дом: &Path) -> PathBuf {
    if путь == "." || путь == "~" {
        return дом.to_path_buf();
    }
    if let Some(хвост) = путь.strip_prefix("~/") {
        return дом.join(хвост);
    }
    PathBuf::from(путь)
}

/// Ярлык программы по имени или id: «Калькулятор», «firefox»
fn найди_ярлык(что: &str) -> Option<String> {
    let что = что.to_lowercase();
    let все = gio::AppInfo::all();
    все.iter()
        .find(|а| а.name().to_lowercase() == что || а.id().is_some_and(|и| и.to_lowercase().trim_end_matches(".desktop") == что))
        .and_then(|а| а.id().map(|и| и.to_string()))
}

pub fn разбери(строка: &str, дом: &Path, ярлык: impl Fn(&str) -> Option<String>, есть: impl Fn(&str) -> bool) -> Дело {
    let с = строка.trim();
    if с.is_empty() {
        return Дело::Ничего;
    }
    let нижн = с.to_lowercase();
    if ["http://", "https://", "ftp://", "mailto:", "file://"].iter().any(|п| нижн.starts_with(п)) {
        return Дело::Открыть(с.to_string());
    }
    if нижн.starts_with("www.") && !с.contains(' ') {
        return Дело::Открыть(format!("https://{с}"));
    }
    if let Some(к) = слово_windows(с) {
        return Дело::Запуск(к.into_iter().map(String::from).collect());
    }
    // Путь: папка или файл
    if с.starts_with('/') || с.starts_with('~') || с == "." || с.starts_with("./") {
        let п = раскрой(с, дом);
        if п.is_dir() {
            return Дело::Запуск(vec!["glower-files".into(), п.to_string_lossy().into()]);
        }
        if исполняемый(&п) {
            return Дело::Запуск(vec![п.to_string_lossy().into()]);
        }
        if п.exists() {
            return Дело::Открыть(п.to_string_lossy().into());
        }
    }
    // Команда с доводами — разобранная как в оболочке, но без оболочки
    let доводы: Vec<String> = match glib::shell_parse_argv(с) {
        Ok(д) => д.into_iter().map(|д| д.to_string_lossy().into_owned()).collect(),
        Err(_) => return Дело::Нет(с.to_string()),
    };
    if let Some(первый) = доводы.first() {
        if есть(первый) {
            let mut д = доводы.clone();
            // ~/ в доводах — как в оболочке
            for х in д.iter_mut().skip(1) {
                if х.starts_with("~/") || х == "~" {
                    *х = раскрой(х, дом).to_string_lossy().into();
                }
            }
            return Дело::Запуск(д);
        }
    }
    // Имя программы из меню — целиком, с пробелами
    if let Some(id) = ярлык(с) {
        return Дело::Ярлык(id);
    }
    // Файл в домашней папке без ~/
    let п = дом.join(с);
    if !с.starts_with('/') && п.exists() {
        return if п.is_dir() { Дело::Запуск(vec!["glower-files".into(), п.to_string_lossy().into()]) } else { Дело::Открыть(п.to_string_lossy().into()) };
    }
    Дело::Нет(с.to_string())
}

fn история() -> Vec<String> {
    crate::apps::читай_данные(ИСТОРИЯ).as_array().map(|а| а.iter().filter_map(|х| х.as_str().map(String::from)).collect()).unwrap_or_default()
}

fn запомни(строка: &str) {
    let mut и = история();
    и.retain(|х| х != строка);
    и.insert(0, строка.to_string());
    и.truncate(25);
    crate::apps::пиши_данные(ИСТОРИЯ, &Value::Array(и.into_iter().map(Value::from).collect()));
}

/// Программы из PATH — для подсказок
fn программы_path() -> Vec<String> {
    let mut все: Vec<String> = Vec::new();
    if let Some(п) = std::env::var_os("PATH") {
        for к in std::env::split_paths(&п) {
            if let Ok(чт) = std::fs::read_dir(&к) {
                for ф in чт.flatten() {
                    if let Some(и) = ф.file_name().to_str() {
                        if !и.starts_with('.') && исполняемый(&ф.path()) {
                            все.push(и.to_string());
                        }
                    }
                }
            }
        }
    }
    все.sort();
    все.dedup();
    все
}

fn ошибка(окно: &gtk::Window, заголовок: &str, текст: &str) {
    let д = gtk::MessageDialog::new(Some(окно), gtk::DialogFlags::MODAL, gtk::MessageType::Error, gtk::ButtonsType::Ok, заголовок);
    д.set_secondary_text(Some(текст));
    д.run();
    unsafe { д.destroy() };
}

/// Запустить отвязанно: программа живёт дальше, когда окно закроется
fn спавн(доводы: &[String], админ: bool) -> Result<(), String> {
    let mut все: Vec<String> = Vec::new();
    if админ {
        все.push("pkexec".into());
        // pkexec не передаёт программе окружение — экран ей нужен
        все.push("env".into());
        for к in ["DISPLAY", "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR", "XAUTHORITY"] {
            if let Ok(з) = std::env::var(к) {
                все.push(format!("{к}={з}"));
            }
        }
    }
    все.extend(доводы.iter().cloned());
    let mut к = std::process::Command::new(&все[0]);
    к.args(&все[1..]).current_dir(glib::home_dir()).stdin(std::process::Stdio::null());
    use std::os::unix::process::CommandExt;
    к.process_group(0);
    match к.spawn() {
        Ok(mut ребёнок) => {
            std::thread::spawn(move || ребёнок.wait());
            Ok(())
        }
        Err(е) => Err(е.to_string()),
    }
}

/// Сделать дело. true — получилось, окно можно закрыть
fn сделай(окно: &gtk::Window, строка: &str, админ: bool) -> bool {
    let дом = glib::home_dir();
    let дело = разбери(строка, &дом, найди_ярлык, |к| в_path(к).is_some());
    let не_найдено = |что: &str| {
        ошибка(окно, &format!("Не удаётся найти «{что}»"), "Проверьте, правильно ли указано имя, и повторите попытку.");
        false
    };
    match дело {
        Дело::Ничего => false,
        Дело::Нет(что) => не_найдено(&что),
        Дело::Запуск(доводы) => match спавн(&доводы, админ) {
            Ok(()) => true,
            Err(е) => {
                ошибка(окно, &format!("Не удалось запустить «{}»", доводы[0]), &е);
                false
            }
        },
        Дело::Ярлык(id) => {
            let Some(а) = gio::DesktopAppInfo::new(&id) else { return не_найдено(строка) };
            if админ {
                // Ярлык с правами — его команда через pkexec
                let команда = а.commandline().map(|к| к.to_string_lossy().split_whitespace().filter(|ч| !ч.starts_with('%')).map(String::from).collect::<Vec<_>>()).unwrap_or_default();
                if команда.is_empty() {
                    return не_найдено(строка);
                }
                return спавн(&команда, true).map_err(|е| ошибка(окно, "Не удалось запустить", &е)).is_ok();
            }
            match а.launch(&[], None::<&gio::AppLaunchContext>) {
                Ok(()) => true,
                Err(е) => {
                    ошибка(окно, "Не удалось запустить", &е.to_string());
                    false
                }
            }
        }
        Дело::Открыть(что) => {
            let адрес = if что.contains("://") || что.starts_with("mailto:") { что.clone() } else { gio::File::for_path(&что).uri().to_string() };
            match gio::AppInfo::launch_default_for_uri(&адрес, None::<&gio::AppLaunchContext>) {
                Ok(()) => true,
                Err(е) => {
                    ошибка(окно, &format!("Не удалось открыть «{что}»"), &е.to_string());
                    false
                }
            }
        }
    }
}

pub fn запусти(доводы: Vec<String>) -> ! {
    подготовь("glower-run", "Выполнить");
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль");
    gtk::StyleContext::add_provider_for_screen(&gtk::gdk::Screen::default().expect("экран"), &стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_title("Выполнить");
    окно.set_icon_name(Some("system-run"));
    окно.set_default_size(480, -1);
    окно.set_resizable(false);
    окно.style_context().add_class("glower-app");
    crate::apps::строка_заголовка(&окно);

    let к = gtk::Box::new(gtk::Orientation::Vertical, 14);
    к.style_context().add_class("run");
    окно.add(&к);
    let верх = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    let знак = gtk::Image::from_icon_name(Some("system-run-symbolic"), gtk::IconSize::Dialog);
    знак.set_pixel_size(40);
    знак.set_valign(gtk::Align::Start);
    верх.add(&знак);
    let пояснение = gtk::Label::new(Some("Введите имя программы, папки, документа или адрес в интернете, и GlowerOS откроет их."));
    пояснение.set_line_wrap(true);
    пояснение.set_max_width_chars(46);
    пояснение.set_xalign(0.0);
    пояснение.style_context().add_class("lead");
    верх.add(&пояснение);
    к.add(&верх);

    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let подпись = gtk::Label::new(Some("Открыть:"));
    ряд.add(&подпись);
    // Поле с выпадающей историей, как в Windows
    let выбор = gtk::ComboBoxText::with_entry();
    выбор.set_hexpand(true);
    let прошлое = история();
    for х in &прошлое {
        выбор.append_text(х);
    }
    let поле = выбор.child().and_then(|ч| ч.downcast::<gtk::Entry>().ok()).expect("поле");
    поле.set_activates_default(true);
    if let Some(что) = доводы.first() {
        поле.set_text(что);
    } else if let Some(последнее) = прошлое.first() {
        поле.set_text(последнее);
    }
    поле.select_region(0, -1);
    ряд.add(&выбор);
    к.add(&ряд);
    // Подсказки: прошлое и программы из PATH
    let подсказки = gtk::ListStore::new(&[glib::Type::STRING]);
    for х in прошлое.iter().cloned().chain(программы_path()) {
        подсказки.set(&подсказки.append(), &[(0, &х)]);
    }
    let дополнение = gtk::EntryCompletion::new();
    дополнение.set_model(Some(&подсказки));
    дополнение.set_text_column(0);
    дополнение.set_minimum_key_length(2);
    дополнение.set_popup_set_width(true);
    поле.set_completion(Some(&дополнение));

    let админ = gtk::CheckButton::with_label("С правами администратора");
    админ.set_tooltip_text(Some(&crate::i18n::tr("Система спросит пароль")));
    к.add(&админ);

    let кнопки = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    кнопки.set_halign(gtk::Align::End);
    let ок = gtk::Button::with_label("ОК");
    ок.style_context().add_class("main");
    ок.set_can_default(true);
    let отмена = gtk::Button::with_label("Отмена");
    let обзор = gtk::Button::with_label("Обзор…");
    кнопки.add(&ок);
    кнопки.add(&отмена);
    кнопки.add(&обзор);
    к.add(&кнопки);
    окно.set_default(Some(&ок));

    {
        let (окно2, поле2, админ2) = (окно.clone(), поле.clone(), админ.clone());
        ок.connect_clicked(move |_| {
            let строка = поле2.text().to_string();
            if сделай(&окно2, &строка, админ2.is_active()) {
                запомни(строка.trim());
                gtk::main_quit();
            }
        });
        // ОК — только когда есть что выполнять
        let ок2 = ок.clone();
        поле.connect_changed(move |п| ок2.set_sensitive(!п.text().trim().is_empty()));
        ок.set_sensitive(!поле.text().trim().is_empty());
    }
    отмена.connect_clicked(|_| gtk::main_quit());
    {
        let (окно2, поле2) = (окно.clone(), поле.clone());
        обзор.connect_clicked(move |_| {
            let д = gtk::FileChooserDialog::with_buttons(Some(&crate::i18n::tr("Обзор")), Some(&окно2), gtk::FileChooserAction::Open, &[("Отмена", gtk::ResponseType::Cancel), ("Открыть", gtk::ResponseType::Accept)]);
            if д.run() == gtk::ResponseType::Accept {
                if let Some(п) = д.filename() {
                    let т = п.to_string_lossy().to_string();
                    // С пробелами — в кавычках: строка разбирается как команда
                    поле2.set_text(&if т.contains(' ') { glib::shell_quote(&т).to_string_lossy().into_owned() } else { т });
                }
            }
            unsafe { д.destroy() };
        });
    }
    окно.connect_key_press_event(|_, е| {
        if е.keyval() == gtk::gdk::keys::constants::Escape {
            gtk::main_quit();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    окно.connect_delete_event(|_, _| {
        gtk::main_quit();
        glib::Propagation::Proceed
    });
    окно.show_all();
    поле.grab_focus();
    gtk::main();
    std::process::exit(0);
}

#[cfg(test)]
mod проверки {
    use super::*;

    fn р(с: &str) -> Дело {
        let дом = std::env::temp_dir();
        разбери(с, &дом, |и| (и == "Мой редактор").then(|| "editor.desktop".to_string()), |к| ["gimp", "foot", "ls"].contains(&к))
    }

    #[test]
    fn понимает_как_windows() {
        assert_eq!(р(""), Дело::Ничего);
        assert_eq!(р("cmd"), Дело::Запуск(vec!["foot".into()]));
        assert_eq!(р("notepad.exe"), Дело::Запуск(vec!["glower-notepad".into()]));
        assert_eq!(р("control"), Дело::Запуск(vec!["glower-control".into()]));
        assert_eq!(р("appwiz.cpl"), Дело::Запуск(vec!["glower-store".into(), "installed".into()]));
        assert_eq!(р("www.example.org"), Дело::Открыть("https://www.example.org".into()));
        assert_eq!(р("https://flathub.org"), Дело::Открыть("https://flathub.org".into()));
    }

    #[test]
    fn команды_без_оболочки() {
        assert_eq!(р("gimp 'мой файл.png'"), Дело::Запуск(vec!["gimp".into(), "мой файл.png".into()]));
        // ; и | — просто доводы, оболочки здесь нет
        assert_eq!(р("ls ; rm -rf /"), Дело::Запуск(vec!["ls".into(), ";".into(), "rm".into(), "-rf".into(), "/".into()]));
        assert_eq!(р("Мой редактор"), Дело::Ярлык("editor.desktop".into()));
        assert_eq!(р("Калькулятор"), Дело::Запуск(vec!["glower-calc".into()]));
        assert!(matches!(р("нет-такой-программы"), Дело::Нет(_)));
        assert!(matches!(р("gimp 'не закрыта"), Дело::Нет(_)));
    }

    #[test]
    fn папки_и_файлы() {
        let дом = std::env::temp_dir();
        assert_eq!(р("."), Дело::Запуск(vec!["glower-files".into(), дом.to_string_lossy().into()]));
        assert_eq!(р("/"), Дело::Запуск(vec!["glower-files".into(), "/".into()]));
    }
}
