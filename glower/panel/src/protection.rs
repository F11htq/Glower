//! «Защита» — окно проверки на вирусы (glower-scan).
//!
//! Открывается из «Параметров» (быстрая и полная проверка) и из меню по
//! правой кнопке — «Проверить на вирусы» на файлах и папках. Проверяет
//! агент (ClamAV), окно только показывает ход и найденное и даёт решить, что
//! с ним делать: в карантин, удалить или оставить.
//!
//! glower-scan --quick | --full | путь… ; без доводов — показать уже идущую.

use crate::agent::позови;
use gtk::glib;
use gtk::prelude::*;
use serde_json::{json, Value};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

const СТИЛЬ: &str = r#"
.scan .h1 { font-size: 20px; font-weight: 600; }
.scan .d { font-size: 12px; opacity: 0.65; }
.scan .big { font-size: 40px; }
.scan .found { background-color: alpha(@glower_fg, 0.05); border-radius: 10px; padding: 8px 12px; }
.scan .threat { color: #ff8a80; font-size: 12px; }
.scan progressbar trough { min-height: 8px; border-radius: 4px; }
.scan progressbar progress { min-height: 8px; border-radius: 4px; }
"#;

/// Включена ли «Защита» и есть ли чем проверять. По этому решаем, давать
/// ли пункт «Проверить на вирусы» и проверять ли скачанное: выключенная
/// «Защита» не должна напоминать о себе нигде
pub fn доступна() -> bool {
    let файл = glib::home_dir().join(".config/glower/protection.json");
    let вкл = std::fs::read_to_string(файл)
        .ok()
        .and_then(|т| serde_json::from_str::<Value>(&т).ok())
        .and_then(|в| в["вкл"].as_bool())
        .unwrap_or(true);
    вкл && ["/usr/bin/clamscan", "/usr/local/bin/clamscan"].iter().any(|п| std::path::Path::new(п).exists())
}

/// Открыть окно проверки этих файлов и папок
pub fn проверь(пути: &[std::path::PathBuf]) {
    if let Err(е) = std::process::Command::new("glower-scan").args(пути).spawn() {
        crate::notify::покажи("Проверка на вирусы", &format!("Не открылась: {е}"), "⚠️");
    }
}

/// Пункт меню «Проверить на вирусы» — или ничего, если «Защита» выключена
pub fn пункт_меню(пути: Vec<std::path::PathBuf>) -> Option<gtk::MenuItem> {
    if !доступна() || пути.is_empty() {
        return None;
    }
    let п = gtk::MenuItem::with_label(&crate::i18n::tr("Проверить на вирусы"));
    п.connect_activate(move |_| проверь(&пути));
    Some(п)
}

struct Окно {
    окно: gtk::Window,
    заголовок: gtk::Label,
    знак: gtk::Label,
    ход: gtk::ProgressBar,
    итог: gtk::Label,
    текущий: gtk::Label,
    найдено: gtk::Box,
    кнопка: gtk::Button,
    все_в_карантин: gtk::Button,
    /// Что уже показали в списке найденного
    показано: Cell<usize>,
    идёт: Cell<bool>,
}

fn ярлык(т: &str, класс: &str) -> gtk::Label {
    let л = gtk::Label::new(Some(т));
    л.set_xalign(0.0);
    л.set_line_wrap(true);
    if !класс.is_empty() {
        л.style_context().add_class(класс);
    }
    л
}

fn имя_файла(путь: &str) -> String {
    std::path::Path::new(путь).file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_else(|| путь.to_string())
}

fn число(н: u64) -> String {
    let с = н.to_string();
    let mut в = String::new();
    for (i, ч) in с.chars().enumerate() {
        if i > 0 && (с.len() - i) % 3 == 0 {
            в.push('\u{202f}');
        }
        в.push(ч);
    }
    в
}

pub fn запусти(доводы: Vec<String>) -> ! {
    crate::apps::подготовь("glower-scan", "Проверка на вирусы");
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль проверки");
    gtk::StyleContext::add_provider_for_screen(&gtk::gdk::Screen::default().expect("экран"), &стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_title(&crate::i18n::tr("Проверка на вирусы"));
    окно.set_default_size(560, 440);
    окно.set_icon_name(Some("security-high"));
    окно.style_context().add_class("glower-app");
    crate::apps::строка_заголовка(&окно);
    окно.style_context().add_class("scan");
    окно.connect_destroy(|_| gtk::main_quit());

    let к = gtk::Box::new(gtk::Orientation::Vertical, 10);
    к.set_margin(20);
    окно.add(&к);
    let шапка = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    let знак = ярлык("🛡️", "big");
    шапка.add(&знак);
    let тексты = gtk::Box::new(gtk::Orientation::Vertical, 4);
    тексты.set_valign(gtk::Align::Center);
    let заголовок = ярлык("Проверка на вирусы", "h1");
    тексты.add(&заголовок);
    let итог = ярлык("Загружаю базу вирусов…", "d");
    тексты.add(&итог);
    шапка.add(&тексты);
    к.add(&шапка);
    let ход = gtk::ProgressBar::new();
    к.add(&ход);
    let текущий = ярлык("", "d");
    текущий.set_line_wrap(false);
    текущий.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    к.add(&текущий);

    let найдено = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let прокрутка = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    прокрутка.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    прокрутка.set_vexpand(true);
    прокрутка.add(&найдено);
    к.add(&прокрутка);

    let низ = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    низ.set_halign(gtk::Align::End);
    let все_в_карантин = gtk::Button::with_label(&crate::i18n::tr("Всё в карантин"));
    все_в_карантин.style_context().add_class("accent");
    все_в_карантин.set_no_show_all(true);
    низ.add(&все_в_карантин);
    let кнопка = gtk::Button::with_label(&crate::i18n::tr("Остановить"));
    низ.add(&кнопка);
    к.add(&низ);

    let о = Rc::new(Окно {
        окно: окно.clone(),
        заголовок,
        знак,
        ход,
        итог,
        текущий,
        найдено,
        кнопка: кнопка.clone(),
        все_в_карантин: все_в_карантин.clone(),
        показано: Cell::new(0),
        идёт: Cell::new(true),
    });
    {
        let о2 = о.clone();
        кнопка.connect_clicked(move |_| {
            if о2.идёт.get() {
                позови("av.scan.stop", json!({}), |_| {});
            } else {
                о2.окно.close();
            }
        });
    }
    {
        let о2 = о.clone();
        все_в_карантин.connect_clicked(move |б| {
            б.hide();
            for р in о2.найдено.children() {
                // У каждой строки первая кнопка — «В карантин»
                if let Some(к) = кнопки_строки(&р).into_iter().next() {
                    if к.is_sensitive() {
                        к.clicked();
                    }
                }
            }
        });
    }
    окно.show_all();

    let (вид, пути): (&str, Vec<String>) = match доводы.first().map(String::as_str) {
        Some("--quick") => ("быстрая", vec![]),
        Some("--full") => ("полная", vec![]),
        None => ("", vec![]),
        _ => ("файлы", доводы.iter().filter(|д| !д.starts_with("--")).map(|д| crate::filemanager::путь_из(д).map(|п| п.to_string_lossy().to_string()).unwrap_or_default()).filter(|д| д.starts_with('/')).collect()),
    };
    о.заголовок.set_text(&crate::i18n::tr(match вид {
        "быстрая" => "Быстрая проверка",
        "полная" => "Полная проверка",
        _ => "Проверка на вирусы",
    }));
    if вид == "файлы" && пути.len() == 1 {
        о.заголовок.set_text(&format!("{} «{}»", crate::i18n::tr("Проверка"), имя_файла(&пути[0])));
    }
    let о2 = о.clone();
    let следи = move || {
        let о3 = о2.clone();
        glib::timeout_add_local(Duration::from_millis(700), move || {
            if !о3.идёт.get() {
                return glib::ControlFlow::Break;
            }
            let о4 = о3.clone();
            позови("av.scan.state", json!({}), move |р| {
                if let Ok(р) = р {
                    покажи(&о4, &р);
                }
            });
            glib::ControlFlow::Continue
        });
    };
    if вид.is_empty() {
        следи();
    } else {
        let о2 = о.clone();
        позови("av.scan", json!({ "вид": вид, "пути": пути }), move |р| match р {
            Ok(р) => {
                покажи(&о2, &р);
                следи();
            }
            // Уже идёт другая — показываем её, а не ругаемся
            Err(е) if е.contains("уже идёт") => следи(),
            Err(е) => беда(&о2, &е),
        });
    }
    gtk::main();
    std::process::exit(0);
}

fn кнопки_строки(р: &gtk::Widget) -> Vec<gtk::Button> {
    let Some(к) = р.clone().downcast::<gtk::Box>().ok() else { return vec![] };
    к.children()
        .into_iter()
        .filter_map(|в| в.downcast::<gtk::Box>().ok())
        .flat_map(|в| в.children())
        .filter_map(|в| в.downcast::<gtk::Button>().ok())
        .collect()
}

fn беда(о: &Rc<Окно>, текст: &str) {
    о.идёт.set(false);
    о.знак.set_text("⚠️");
    о.итог.set_text(текст);
    о.ход.set_fraction(0.0);
    о.кнопка.set_label(&crate::i18n::tr("Закрыть"));
    // Выключена или нет базы — сразу дорога туда, где это чинится
    if текст.contains("выключена") || текст.contains("база") || текст.contains("ClamAV") {
        let б = gtk::Button::with_label(&crate::i18n::tr("Открыть «Защиту»"));
        б.style_context().add_class("accent");
        б.connect_clicked(|_| {
            let _ = std::process::Command::new("glower-settings").arg("protect").spawn();
        });
        if let Some(низ) = о.кнопка.parent().and_then(|н| н.downcast::<gtk::Box>().ok()) {
            низ.add(&б);
            низ.reorder_child(&б, 0);
            б.show();
        }
    }
}

fn покажи(о: &Rc<Окно>, р: &Value) {
    let проверено = р["проверено"].as_u64().unwrap_or(0);
    let всего = р["всего"].as_u64().unwrap_or(0);
    let идёт = р["идёт"].as_bool() == Some(true);
    if р["стадия"].as_str() == Some("база") && идёт {
        о.итог.set_text(&crate::i18n::tr("Загружаю базу вирусов… Это занимает полминуты"));
        о.ход.pulse();
    } else if идёт {
        о.итог.set_text(&if всего > 0 {
            format!("{} {} {} {}", crate::i18n::tr("Проверено"), число(проверено), crate::i18n::tr("из"), число(всего))
        } else {
            format!("{} {}", crate::i18n::tr("Проверено файлов:"), число(проверено))
        });
        if всего > 0 {
            о.ход.set_fraction((проверено as f64 / всего as f64).min(1.0));
        } else {
            о.ход.pulse();
        }
        о.текущий.set_text(р["текущий"].as_str().unwrap_or(""));
    }
    let список = р["найдено"].as_array().cloned().unwrap_or_default();
    for н in список.iter().skip(о.показано.get()) {
        добавь_найденное(о, н["путь"].as_str().unwrap_or(""), н["угроза"].as_str().unwrap_or(""));
    }
    о.показано.set(список.len());
    if !идёт {
        о.идёт.set(false);
        о.текущий.set_text("");
        о.кнопка.set_label(&crate::i18n::tr("Закрыть"));
        if let Some(е) = р["ошибка"].as_str() {
            return беда(о, е);
        }
        о.ход.set_fraction(1.0);
        let остановлена = р["остановлена"].as_bool() == Some(true);
        if список.is_empty() {
            о.знак.set_text(if остановлена { "⏹" } else { "✅" });
            о.итог.set_text(&if остановлена {
                format!("{} {} {}", crate::i18n::tr("Проверка остановлена. Проверено файлов:"), число(проверено), crate::i18n::tr("— угроз среди них нет"))
            } else {
                format!("{} {}", crate::i18n::tr("Угроз не найдено. Проверено файлов:"), число(проверено))
            });
        } else {
            о.знак.set_text("⚠️");
            о.итог.set_text(&format!("{} {}", crate::i18n::tr("Найдено угроз:"), список.len()));
            if список.len() > 1 {
                о.все_в_карантин.show();
            }
        }
    }
}

fn добавь_найденное(о: &Rc<Окно>, путь: &str, угроза: &str) {
    let р = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    р.style_context().add_class("found");
    let т = gtk::Box::new(gtk::Orientation::Vertical, 2);
    т.set_hexpand(true);
    т.add(&ярлык(&имя_файла(путь), ""));
    т.add(&ярлык(угроза, "threat"));
    let где = ярлык(путь, "d");
    где.set_line_wrap(false);
    где.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    т.add(&где);
    р.add(&т);
    let кнопки = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    кнопки.set_valign(gtk::Align::Center);
    let в_карантин = gtk::Button::with_label(&crate::i18n::tr("В карантин"));
    в_карантин.style_context().add_class("accent");
    let удалить = gtk::Button::with_label(&crate::i18n::tr("Удалить"));
    кнопки.add(&в_карантин);
    кнопки.add(&удалить);
    р.add(&кнопки);
    о.найдено.add(&р);
    р.show_all();

    let сделано = {
        let кнопки = кнопки.clone();
        move |итог: Result<&str, String>| {
            for к in кнопки.children() {
                кнопки.remove(&к);
            }
            let л = match итог {
                Ok(что) => ярлык(&crate::i18n::tr(что), "d"),
                Err(е) => {
                    let л = ярлык(&е, "threat");
                    л.set_max_width_chars(28);
                    л
                }
            };
            кнопки.add(&л);
            кнопки.show_all();
        }
    };
    let сделано = Rc::new(сделано);
    {
        let (путь, угроза, сделано) = (путь.to_string(), угроза.to_string(), сделано.clone());
        в_карантин.connect_clicked(move |б| {
            б.set_sensitive(false);
            let сделано = сделано.clone();
            позови("av.quarantine", json!({ "путь": путь, "угроза": угроза }), move |р| сделано(р.map(|_| "В карантине")));
        });
    }
    {
        let (путь, сделано) = (путь.to_string(), сделано.clone());
        удалить.connect_clicked(move |б| {
            б.set_sensitive(false);
            let сделано = сделано.clone();
            позови("av.delete", json!({ "путь": путь }), move |р| сделано(р.map(|_| "Удалено")));
        });
    }
}

/// Перед установкой пакета или запуском программы: проверить и, если всё
/// чисто (или «Защита» выключена, или проверить нечем), идти `дальше`.
/// Нашлось — спросить, как Windows SmartScreen: по умолчанию — не ставить и
/// убрать файл в карантин; «Всё равно» — за человеком.
pub fn перед(путь: &str, имя: &str, что_делаем: &'static str, дальше: impl FnOnce(Option<String>) + 'static) {
    if !доступна() {
        return дальше(None);
    }
    // Проверка с загрузкой базы — секунд двадцать; повторная — мгновенно.
    // Сообщаем, только если ответа нет дольше полутора секунд
    let ответил = Rc::new(Cell::new(false));
    {
        let (ответил, имя) = (ответил.clone(), имя.to_string());
        glib::timeout_add_local_once(Duration::from_millis(1500), move || {
            if !ответил.get() {
                crate::notify::покажи(&имя, &crate::i18n::tr("Проверяю на вирусы…"), "🛡️");
            }
        });
    }
    let (путь, имя) = (путь.to_string(), имя.to_string());
    crate::agent::позови_за("av.check", json!({ "путь": путь }), 600, move |р| {
        ответил.set(true);
        let р = match р {
            Ok(р) => р,
            // Проверка сорвалась — не прячем, но и не держим человека
            Err(е) => return дальше(Some(format!("{} {е}", crate::i18n::tr("Проверить на вирусы не удалось:")))),
        };
        if р["чисто"].as_bool() != Some(false) {
            return дальше(None);
        }
        let угроза = р["угроза"].as_str().unwrap_or("неизвестная угроза").to_string();
        let где = р["где"].as_str().unwrap_or("").to_string();
        let д = gtk::MessageDialog::new(
            None::<&gtk::Window>,
            gtk::DialogFlags::MODAL,
            gtk::MessageType::Warning,
            gtk::ButtonsType::None,
            &format!("{} «{имя}»", crate::i18n::tr("Найдена угроза в")),
        );
        let mut текст = угроза.clone();
        if !где.is_empty() {
            текст.push_str(&format!("\n{} {где}", crate::i18n::tr("Внутри:")));
        }
        текст.push_str(&format!("\n\n{}", crate::i18n::tr(что_делаем)));
        д.set_secondary_text(Some(&текст));
        д.add_button(&crate::i18n::tr("Всё равно продолжить"), gtk::ResponseType::Reject);
        д.add_button(&crate::i18n::tr("В карантин"), gtk::ResponseType::Accept);
        д.set_default_response(gtk::ResponseType::Accept);
        д.set_keep_above(true);
        let ответ = д.run();
        unsafe { д.destroy() };
        match ответ {
            gtk::ResponseType::Reject => дальше(None),
            gtk::ResponseType::Accept => {
                let имя2 = имя.clone();
                позови("av.quarantine", json!({ "путь": путь, "угроза": угроза }), move |р| match р {
                    Ok(_) => crate::notify::покажи(&имя2, &crate::i18n::tr("Файл убран в карантин. Вернуть его можно в «Параметрах» → «Защита»."), "🛡️"),
                    Err(е) => crate::notify::покажи(&имя2, &е, "⚠️"),
                });
            }
            _ => {}
        }
    });
}
