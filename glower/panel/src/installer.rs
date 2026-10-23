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

use crate::agent::{зов, позови};
use crate::apps;
use gtk::glib;
use gtk::prelude::*;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

const СТИЛЬ: &str = r#"
.installer .body { padding: 34px 40px 10px 40px; }
.installer .foot { padding: 14px 20px; border-top: 1px solid alpha(@glower_fg, 0.07); }
.installer .h1 { font-size: 26px; font-weight: 300; }
.installer .h2 { font-size: 20px; font-weight: 300; }
.installer .lead { opacity: 0.8; }
.installer .note { background-color: alpha(@glower_accent, 0.12); border-radius: 10px; padding: 10px 14px; }
.installer .warn { background-color: rgba(255, 90, 90, 0.14); border-radius: 10px; padding: 10px 14px; color: #ffb4b4; }
.installer button.disk { padding: 12px 14px; border-radius: 12px; }
.installer button.disk.on { background-color: alpha(@glower_accent, 0.30); box-shadow: inset 0 0 0 1px @glower_accent; }
.installer .big { font-size: 26px; }
.installer button:disabled { opacity: 0.4; }
.installer .sum-k { opacity: 0.6; }
.installer progressbar trough { min-height: 10px; border-radius: 5px; background-color: alpha(@glower_fg, 0.08); }
.installer progressbar progress { min-height: 10px; border-radius: 5px; background-image: linear-gradient(90deg, @glower_accent, @glower_accent2); }
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
    назад: gtk::Button,
    далее: gtk::Button,
    с: Состояние,
}

thread_local! {
    static М: RefCell<Option<Rc<RefCell<Мастер>>>> = const { RefCell::new(None) };
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
    окно.set_default_size(760, 580);
    окно.style_context().add_class("glower-app");
    окно.style_context().add_class("installer");
    окно.connect_destroy(|_| gtk::main_quit());
    // Пока идёт запись на диск, окно не закрыть: оборванная установка
    // оставляет диск размеченным наполовину
    окно.connect_delete_event(|_, _| {
        if с_мастером(|м| м.с.шаг == 4) == Some(true) {
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });

    let колонка = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let тело = gtk::Box::new(gtk::Orientation::Vertical, 14);
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
    колонка.add(&листание);
    колонка.add(&низ);
    окно.add(&колонка);

    назад.connect_clicked(|_| {
        с_мастером(|м| м.с.шаг = м.с.шаг.saturating_sub(1));
        нарисуй();
    });
    далее.connect_clicked(|_| вперёд());

    М.with(|я| {
        *я.borrow_mut() = Some(Rc::new(RefCell::new(Мастер {
            окно: окно.clone(),
            тело,
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
    окно.show_all();
    if во_весь_экран {
        окно.fullscreen();
    }
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
        м.назад.set_visible(назад);
        м.далее.set_visible(далее.is_some());
        if let Some(т) = далее {
            м.далее.set_label(т);
        }
        м.далее.set_sensitive(можно);
    });
}

fn логотип() -> Option<gtk::Image> {
    crate::setup::логотип(96).map(|л| gtk::Image::from_pixbuf(Some(&л)))
}

fn нарисуй() {
    let Some(т) = очисти() else { return };
    let шаг = с_мастером(|м| м.с.шаг).unwrap_or(0);
    let чиним = с_мастером(|м| м.с.чиним).unwrap_or(false);
    match шаг {
        0 => {
            if let Some(л) = логотип() {
                л.set_halign(gtk::Align::Start);
                т.add(&л);
            }
            т.add(&метка(if чиним { "Восстановление GlowerOS" } else { "Установка GlowerOS" }, "h1"));
            т.add(&метка(
                if чиним {
                    "Системные файлы будут положены заново поверх установленной системы. Личные файлы и папка пользователя останутся на месте."
                } else {
                    "Сейчас система работает из памяти и всё забывает при выключении. Установка перенесёт её на диск: настройки, файлы и учётная запись начнут сохраняться."
                },
                "lead",
            ));
            if !чиним && в_ядре("glower.install=1") {
                т.add(&метка("Esc — попробовать систему без установки", "muted"));
            }
            кнопки(false, Some("Далее"), false);
            let место = т.clone();
            let ждём = метка("Проверяю, можно ли ставить…", "muted");
            т.add(&ждём);
            позови("install.can", json!({}), move |о| {
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
                кнопки(false, Some("Далее"), можно);
            });
        }
        1 => {
            т.add(&метка("Куда установить", "h2"));
            let список = gtk::Box::new(gtk::Orientation::Vertical, 8);
            т.add(&список);
            список.add(&метка("Ищу диски…", "muted"));
            кнопки(true, Some("Далее"), false);
            позови("install.disks", json!({}), move |о| {
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
                            let dev = д["dev"].as_str().unwrap_or("").to_string();
                            let живой = д["live"].as_bool() == Some(true);
                            let б = gtk::Button::new();
                            б.style_context().add_class("disk");
                            if выбран.as_deref() == Some(dev.as_str()) {
                                б.style_context().add_class("on");
                            }
                            let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 14);
                            let з = gtk::Label::new(Some(if д["removable"].as_bool() == Some(true) { "🔌" } else { "💽" }));
                            з.style_context().add_class("big");
                            ряд.add(&з);
                            let к = gtk::Box::new(gtk::Orientation::Vertical, 2);
                            let имя = д["model"].as_str().filter(|м| !м.is_empty()).or(д["name"].as_str()).unwrap_or(&dev).to_string();
                            let и = gtk::Label::new(None);
                            и.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(&имя)));
                            и.set_xalign(0.0);
                            к.add(&и);
                            к.add(&метка(
                                &format!(
                                    "{dev} · {} · {}{}",
                                    размер(д["size"].as_f64().unwrap_or(0.0)),
                                    д["kind"].as_str().unwrap_or(""),
                                    if живой { " · носитель системы" } else { "" }
                                ),
                                "muted",
                            ));
                            ряд.add(&к);
                            б.add(&ряд);
                            б.set_sensitive(!живой);
                            б.connect_clicked(move |_| {
                                с_мастером(|м| м.с.диск = Some(dev.clone()));
                                нарисуй();
                            });
                            список.add(&б);
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
            сетка.set_row_spacing(12);
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
            let строка = |r: i32, з: &str, имя: &str, пояснение: &str, поле: &gtk::Widget| {
                let л = gtk::Box::new(gtk::Orientation::Vertical, 2);
                л.add(&метка(&format!("{з}  {имя}"), ""));
                let п = метка(пояснение, "muted");
                // Ширина с самого начала: узкая колонка с переносом просит
                // у окна огромную высоту, и кнопки уезжают за край экрана
                п.set_width_chars(44);
                п.set_max_width_chars(46);
                л.add(&п);
                сетка.attach(&л, 0, r, 1, 1);
                поле.set_valign(gtk::Align::Center);
                сетка.attach(поле, 1, r, 1, 1);
            };
            строка(0, "🔑", "Пароль системной записи", "Нужен для входа в консоль и для sudo. Можно оставить пустым, как в живой системе.", п1.upcast_ref());
            строка(1, "🏷", "Имя машины", "Так система будет представляться в сети", хн.upcast_ref());
            // Шифрование: пароль входа защищает от того, кто сядет за машину,
            // шифрование — от того, кто унесёт диск
            let флажок = gtk::Switch::new();
            флажок.set_active(шифр);
            флажок.set_halign(gtk::Align::Start);
            строка(2, "🔒", "Зашифровать диск", "Система будет спрашивать пароль при включении. Без него файлы не прочитать даже вынув диск.", флажок.upcast_ref());
            let п2 = gtk::Entry::new();
            п2.set_visibility(false);
            п2.set_placeholder_text(Some("Пароль диска (не короче 8 знаков)"));
            п2.set_text(&пароль_диска);
            строка(3, "🔑", "Пароль диска", "Отдельный от пароля записи. Забыть его нельзя: восстановить данные будет нечем.", п2.upcast_ref());
            т.add(&сетка);
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
            let сетка = gtk::Grid::new();
            сетка.set_row_spacing(8);
            сетка.set_column_spacing(24);
            for (i, (к, в)) in [
                ("Диск", д["model"].as_str().filter(|м| !м.is_empty()).or(д["name"].as_str()).unwrap_or(&с_диск).to_string()),
                ("Устройство", с_диск.clone()),
                ("Объём", размер(д["size"].as_f64().unwrap_or(0.0))),
                ("Имя машины", имя),
                ("Пароль", if пароль { "задан".into() } else { "без пароля".into() }),
                ("Шифрование", if шифр { "диск будет зашифрован".into() } else { "нет".into() }),
            ]
            .into_iter()
            .enumerate()
            {
                сетка.attach(&метка(к, "sum-k"), 0, i as i32, 1, 1);
                let л = gtk::Label::new(None);
                л.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(&в)));
                л.set_xalign(0.0);
                сетка.attach(&л, 1, i as i32, 1, 1);
            }
            т.add(&сетка);
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
            if let Some(л) = логотип() {
                л.set_halign(gtk::Align::Start);
                т.add(&л);
            }
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

fn установка(т: &gtk::Box) {
    т.add(&метка("Устанавливаю", "h2"));
    let полоса = gtk::ProgressBar::new();
    т.add(&полоса);
    let шаг = метка("Начинаю…", "muted");
    т.add(&шаг);
    let подробно = gtk::Expander::new(Some("Подробности"));
    let журнал = gtk::TextView::new();
    журнал.set_editable(false);
    журнал.set_monospace(true);
    let л = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    л.set_min_content_height(200);
    л.add(&журнал);
    подробно.add(&л);
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
    позови("install.start", с, move |о| {
        if let Err(е) = о {
            return беда(&е);
        }
        glib::timeout_add_local(Duration::from_millis(700), move || {
            let с = match зов("install.state", json!({}), 5) {
                Ok(с) => с,
                Err(е) => {
                    беда(&е);
                    return glib::ControlFlow::Break;
                }
            };
            полоса.set_fraction(с["percent"].as_f64().unwrap_or(0.0) / 100.0);
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
    т.add(&метка("Установка не завершилась", "h2"));
    т.add(&метка(текст, "warn"));
    т.add(&метка("Диск мог остаться размеченным наполовину. Можно попробовать снова — установка начинается с полной очистки диска.", "note"));
    с_мастером(|м| м.с.шаг = 6);
    кнопки(false, Some("Начать заново"), true);
    т.show_all();
}

/// Перезагрузка после установки — сразу, как от кнопки на корпусе:
/// корень живой системы лежит на носителе, и аккуратное отключение
/// всего подключённого спотыкается, стоит носитель вынуть
fn перезагрузи() {
    кнопки(false, Some("Перезагружаю…"), false);
    позови("sys.power", json!({ "action": "reboot", "сразу": true }), |о| {
        if let Err(е) = о {
            беда(&format!("Перезагрузка не удалась: {е}. Выключите машину кнопкой и включите заново — система на диске уже готова."));
        }
    });
}
