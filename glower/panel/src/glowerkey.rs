//! Ключ Glower в ядре панели: стол с ключа, слежение за ключами.
//!
//! Стол с ключа применяет ядро, потому что настройки пишет только оно. Перед
//! первым применением прежний стол запоминается в before-key.json, и «Вернуть
//! как было» возвращает его целиком, вместе с мышью.
//!
//! Слежение: раз в две секунды ядро спрашивает агента, какие свои ключи
//! вставлены. Появился ключ, для которого включена копия, — копируем файлы.
//! Пропал ключ, для которого включена блокировка, — запираем экран, как
//! «Динамическая блокировка» в Windows, только по флешке.

use crate::agent::{позови, позови_за};
use crate::core::{задай, настройка};
use crate::notify;
use gtk::glib;
use gtk::prelude::*;
use serde_json::{json, Map, Value};
use std::cell::RefCell;
use std::path::PathBuf;
use std::time::Duration;

/// Что из настроек — «мой стол»; тот же список, что у агента (agent/key.mjs)
pub const СТОЛ: &[&str] = &[
    "accent", "accentCustom", "theme", "transparency", "wallpaper", "wallShuffle", "dockApps", "deskWidgets",
    "showDeskWidgets", "showDesktopIcons", "clock24", "showSeconds", "city", "desks", "nightLight", "focusLen", "userEmoji",
];

thread_local! {
    /// Какие свои ключи были вставлены в прошлый раз
    static БЫЛИ: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

fn файл_прежнего() -> PathBuf {
    glib::user_config_dir().join("glower").join("before-key.json")
}

/// Окно с полем пароля. None — отказались
pub fn спроси_пароль(заголовок: &str, текст: &str, беда: &str, дважды: bool) -> Option<String> {
    let д = gtk::Dialog::with_buttons(Some(заголовок), None::<&gtk::Window>, gtk::DialogFlags::MODAL,
        &[("Отмена", gtk::ResponseType::Cancel), ("Готово", gtk::ResponseType::Accept)]);
    д.set_default_response(gtk::ResponseType::Accept);
    д.set_keep_above(true);
    д.set_position(gtk::WindowPosition::Center);
    let место = д.content_area();
    место.set_spacing(8);
    место.set_margin_top(14);
    место.set_margin_bottom(6);
    место.set_margin_start(18);
    место.set_margin_end(18);
    let т = gtk::Label::new(Some(текст));
    т.set_line_wrap(true);
    т.set_max_width_chars(48);
    т.set_xalign(0.0);
    место.add(&т);
    let поле = |подсказка: &str| {
        let п = gtk::Entry::new();
        п.set_visibility(false);
        п.set_input_purpose(gtk::InputPurpose::Password);
        п.set_placeholder_text(Some(подсказка));
        п.set_activates_default(true);
        место.add(&п);
        п
    };
    let первое = поле("Пароль ключа");
    let второе = дважды.then(|| поле("Ещё раз"));
    let ош = gtk::Label::new(Some(беда));
    ош.style_context().add_class("error");
    ош.set_xalign(0.0);
    ош.set_line_wrap(true);
    место.add(&ош);
    д.show_all();
    loop {
        if д.run() != gtk::ResponseType::Accept {
            unsafe { д.destroy() };
            return None;
        }
        let п = первое.text().to_string();
        if п.chars().count() < 4 {
            ош.set_text("Не короче четырёх знаков");
            continue;
        }
        if let Some(в) = &второе {
            if в.text().as_str() != п {
                ош.set_text("Пароли не совпали");
                continue;
            }
        }
        unsafe { д.destroy() };
        return Some(п);
    }
}

/// Подтвердить, что вы хозяин ключа на флешке `dev`: отпечатком, если ключ
/// здесь свой и сканер есть, иначе паролем ключа. Удалось — `дело(токен)`
pub fn подтверди(dev: &str, дело: Box<dyn Fn(String)>) {
    let dev = dev.to_string();
    let дело: std::rc::Rc<dyn Fn(String)> = std::rc::Rc::from(дело);
    позови("key.confirm.ways", json!({ "dev": dev }), move |о| {
        let с = match о {
            Ok(с) => с,
            Err(е) => return notify::покажи("Ключ Glower", &е, "⚠️"),
        };
        let имя = с["имя"].as_str().unwrap_or("Ключ").to_string();
        if с["палец"].as_bool() == Some(true) {
            палец(&dev, &имя, дело.clone(), с["пароль"].as_bool() == Some(true));
        } else {
            пароль(&dev, &имя, дело.clone(), с["пароль"].as_bool() == Some(true), с["свой"].as_bool() == Some(true), "");
        }
    });
}

fn палец(dev: &str, имя: &str, дело: std::rc::Rc<dyn Fn(String)>, есть_пароль: bool) {
    let д = gtk::MessageDialog::new(None::<&gtk::Window>, gtk::DialogFlags::MODAL, gtk::MessageType::Info, gtk::ButtonsType::None,
        &format!("Ключ «{имя}»: приложите палец к сканеру"));
    д.set_secondary_text(Some("Так система убедится, что ключ в руках у хозяина"));
    д.add_button("Ввести пароль", gtk::ResponseType::Other(1));
    д.add_button("Отмена", gtk::ResponseType::Cancel);
    д.set_keep_above(true);
    let токен = std::rc::Rc::new(std::cell::RefCell::new(None::<String>));
    {
        let (д, токен) = (д.clone(), токен.clone());
        позови_за("key.confirm", json!({ "dev": dev, "палец": true }), 45, move |о| match о {
            Ok(о) => {
                *токен.borrow_mut() = о["токен"].as_str().map(String::from);
                д.response(gtk::ResponseType::Accept);
            }
            Err(е) => д.set_secondary_text(Some(&format!("{е}. Можно ввести пароль ключа"))),
        });
    }
    let ответ = д.run();
    unsafe { д.destroy() };
    if let Some(т) = токен.borrow_mut().take() {
        return дело(т);
    }
    if ответ == gtk::ResponseType::Other(1) {
        пароль(dev, имя, дело, есть_пароль, true, "");
    }
}

fn пароль(dev: &str, имя: &str, дело: std::rc::Rc<dyn Fn(String)>, есть_пароль: bool, свой: bool, беда: &str) {
    if !есть_пароль {
        if !свой {
            return notify::покажи(&format!("Ключ «{имя}»"), "У ключа ещё нет пароля. Задайте его на компьютере, где ключ свой: Параметры → Ключ Glower", "🔑");
        }
        let Some(новый) = спроси_пароль(&format!("Пароль для ключа «{имя}»"), "Ключ сделан до паролей. Задайте пароль: им вы будете подтверждать, что ключ ваш, на других компьютерах и там, где нет сканера отпечатков.", беда, true) else { return };
        let (dev2, имя2, дело2) = (dev.to_string(), имя.to_string(), дело.clone());
        позови("key.password.set", json!({ "dev": dev, "новый": новый }), move |о| match о {
            Ok(о) => дело2(о["токен"].as_str().unwrap_or("").to_string()),
            Err(е) => пароль(&dev2, &имя2, дело2.clone(), false, true, &е),
        });
        return;
    }
    let Some(п) = спроси_пароль(&format!("Ключ «{имя}»"), "Введите пароль ключа — так система убедится, что ключ ваш.", беда, false) else { return };
    let (dev2, имя2, дело2) = (dev.to_string(), имя.to_string(), дело.clone());
    позови_за("key.confirm", json!({ "dev": dev, "пароль": п }), 20, move |о| match о {
        Ok(о) => дело2(о["токен"].as_str().unwrap_or("").to_string()),
        Err(е) => пароль(&dev2, &имя2, дело2.clone(), true, свой, &е),
    });
}

/// Применить стол с ключа на флешке `dev`
pub fn примени_стол(dev: &str, токен: &str) {
    let dev = dev.to_string();
    позови("key.profile.read", json!({ "dev": dev, "токен": токен }), move |о| {
        let п = match о {
            Ok(п) => п,
            Err(е) => return notify::покажи("Ключ Glower", &format!("Стол не применён: {е}"), "⚠️"),
        };
        let новое = п["настройки"].as_object().cloned().unwrap_or_default();
        let мышь = п["мышь"].clone();
        // Прежний стол — только один раз: применили два ключа подряд, а
        // вернуть всё равно нужно своё, а не стол первого ключа
        let прежнее_есть = файл_прежнего().exists();
        позови("sys.mouse", json!({}), move |м| {
            if !прежнее_есть {
                let mut было = Map::new();
                for к in СТОЛ {
                    было.insert(к.to_string(), настройка(к));
                }
                let снимок = json!({ "настройки": было, "мышь": м.ok().map(|м| м["настройки"].clone()).unwrap_or(Value::Null) });
                let _ = std::fs::create_dir_all(файл_прежнего().parent().unwrap_or(std::path::Path::new(".")));
                let _ = std::fs::write(файл_прежнего(), serde_json::to_vec_pretty(&снимок).unwrap_or_default());
            }
            for (к, в) in &новое {
                if СТОЛ.contains(&к.as_str()) {
                    задай(к, в.clone());
                }
            }
            if мышь.is_object() {
                позови("sys.mouse.set", мышь.clone(), |_| {});
            }
            notify::покажи_с_делом("Стол с ключа применён", "Тема, обои, док и виджеты — как на ключе", "🔑", "Вернуть как было", Box::new(верни_стол));
            let нет: Vec<Value> = п["нет"].as_array().cloned().unwrap_or_default();
            if !нет.is_empty() {
                let сколько = нет.len();
                notify::покажи_с_делом(
                    "Программы с ключа",
                    &format!("Здесь нет {сколько} из ваших программ"),
                    "📦",
                    "Поставить",
                    Box::new(move || поставь_программы(&нет)),
                );
            }
        });
    });
}

/// Поставить программы с ключа одной работой
pub fn поставь_программы(ids: &[Value]) {
    позови("pkg.install.many", json!({ "flatpak": ids }), |о| match о {
        Ok(_) => notify::покажи("Программы с ключа", "Ставлю — ход виден в Магазине", "📦"),
        Err(е) => notify::покажи("Программы с ключа", &format!("Не вышло: {е}"), "⚠️"),
    });
}

/// Вернуть стол, каким он был до ключа
pub fn верни_стол() {
    let Some(снимок) = std::fs::read_to_string(файл_прежнего()).ok().and_then(|т| serde_json::from_str::<Value>(&т).ok()) else {
        return notify::покажи("Ключ Glower", "Прежний стол не сохранён — возвращать нечего", "ℹ️");
    };
    if let Some(н) = снимок["настройки"].as_object() {
        for (к, в) in н {
            if СТОЛ.contains(&к.as_str()) {
                задай(к, в.clone());
            }
        }
    }
    if снимок["мышь"].is_object() {
        позови("sys.mouse.set", снимок["мышь"].clone(), |_| {});
    }
    let _ = std::fs::remove_file(файл_прежнего());
    notify::покажи("Стол возвращён", "Всё как было до ключа", "↩️");
}

/// Флешку только что вставили: если на ней чужой ключ (с другого
/// компьютера) — предложить свой стол. Свои ключи ведёт `следи`
pub fn новая_флешка(dev: &str) {
    let dev = dev.to_string();
    позови("key.state", json!({ "подключить": true }), move |о| {
        let Ok(с) = о else { return };
        let Some(ф) = с["флешки"].as_array().and_then(|ф| ф.iter().find(|ф| ф["dev"].as_str() == Some(&dev))).cloned() else { return };
        if !ф["ключ"].is_object() || ф["ключ"]["свой"].as_bool() == Some(true) || !ф["профиль"].is_object() {
            return;
        }
        let имя = ф["ключ"]["имя"].as_str().unwrap_or("Ключ").to_string();
        let dev2 = dev.clone();
        notify::покажи_с_делом(
            &format!("Ключ Glower «{имя}»"),
            "На ключе сохранён стол — тема, обои, док, виджеты и программы",
            "🔑",
            "Применить мой стол",
            Box::new(move || подтверди(&dev2, Box::new({ let d = dev2.clone(); move |т| примени_стол(&d, &т) }))),
        );
    });
}

/// Раз в две секунды: какие свои ключи вставлены
pub fn следи() {
    glib::timeout_add_local(Duration::from_secs(2), || {
        позови("key.present", json!({}), |о| {
            let Ok(о) = о else { return };
            if о["ключей"].as_u64().unwrap_or(0) == 0 {
                БЫЛИ.with(|б| *б.borrow_mut() = Some(vec![]));
                return;
            }
            let есть: Vec<String> = о["есть"].as_array().map(|а| а.iter().filter_map(|х| х.as_str().map(String::from)).collect()).unwrap_or_default();
            let блокировать: Vec<String> = о["блокировать"].as_array().map(|а| а.iter().filter_map(|х| х.as_str().map(String::from)).collect()).unwrap_or_default();
            let было = БЫЛИ.with(|б| б.borrow_mut().replace(есть.clone()));
            // Первый опрос после входа — только запоминаем
            let Some(было) = было else { return };
            if было.iter().any(|id| !есть.contains(id) && блокировать.contains(id)) {
                позови("sys.power", json!({ "action": "lock" }), |_| {});
            }
            let новые: Vec<String> = есть.iter().filter(|id| !было.contains(id)).cloned().collect();
            if !новые.is_empty() {
                пришёл_свой(новые);
            }
        });
        glib::ControlFlow::Continue
    });
}

/// Вставили свой ключ: поздороваться и, если так настроено, скопировать файлы
fn пришёл_свой(новые: Vec<String>) {
    позови("key.state", json!({}), move |о| {
        let Ok(с) = о else { return };
        for id in &новые {
            let Some(к) = с["ключи"].as_array().and_then(|к| к.iter().find(|к| к["id"].as_str() == Some(id.as_str()))).cloned() else { continue };
            let Some(ф) = с["флешки"].as_array().and_then(|ф| ф.iter().find(|ф| ф["ключ"]["id"].as_str() == Some(id.as_str()))).cloned() else { continue };
            let имя = к["имя"].as_str().unwrap_or("Ключ").to_string();
            let dev = ф["dev"].as_str().unwrap_or("").to_string();
            предложи_эстафету(&dev);
            if к["копия"].as_bool() == Some(true) {
                копируй(&dev, &имя);
            } else {
                notify::покажи(&format!("Ключ «{имя}» подключён"), "Его возможности — в Параметрах, раздел «Ключ Glower»", "🔑");
            }
        }
    });
}

/// На ключе — эстафета с другого компьютера: предложить продолжить
pub fn предложи_эстафету(dev: &str) {
    let dev = dev.to_string();
    позови("relay.state", json!({ "dev": dev }), move |о| {
        let Ok(с) = о else { return };
        if с["новая"].as_bool() != Some(true) {
            return;
        }
        let мы = |n: u64, одно: &str, два: &str, много: &str| -> Option<String> {
            if n == 0 {
                return None;
            }
            let слово = if n % 10 == 1 && n % 100 != 11 { одно } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) { два } else { много };
            Some(format!("{n} {слово}"))
        };
        let что: Vec<String> = [
            мы(с["программ"].as_u64().unwrap_or(0), "программа", "программы", "программ"),
            мы(с["файлов"].as_u64().unwrap_or(0), "файл", "файла", "файлов"),
            мы(с["вкладок"].as_u64().unwrap_or(0), "вкладка", "вкладки", "вкладок"),
            if с["буфер"].as_u64().unwrap_or(0) > 0 { Some("буфер обмена".to_string()) } else { None },
        ]
        .into_iter()
        .flatten()
        .collect();
        if что.is_empty() {
            return;
        }
        let откуда = с["компьютер"].as_str().unwrap_or("другого компьютера").to_string();
        let dev2 = dev.clone();
        notify::покажи_с_делом(
            &format!("Продолжить с «{откуда}»?"),
            &что.join(" · "),
            "🏃",
            "Продолжить",
            Box::new(move || подтверди(&dev2, Box::new({ let d = dev2.clone(); move |т| продолжи(&d, &т) }))),
        );
    });
}

/// Продолжить с места: файлы, вкладки и буфер открывает агент, программы — мы
pub fn продолжи(dev: &str, токен: &str) {
    позови("relay.continue", json!({ "dev": dev, "токен": токен }), |о| match о {
        Ok(о) => {
            for ярлык in о["программы"].as_array().cloned().unwrap_or_default() {
                позови("sys.launch", json!({ "id": ярлык }), |_| {});
            }
            notify::покажи("Эстафета принята", "Всё открывается там, где вы остановились. Файлы — в «Документы/Эстафета»", "🏃");
        }
        Err(е) => notify::покажи("Эстафета", &format!("Не вышло: {е}"), "⚠️"),
    });
}

/// Копия файлов на ключ с итогом в уведомлении
pub fn копируй(dev: &str, имя: &str) {
    let имя = имя.to_string();
    позови("key.backup", json!({ "dev": dev }), move |о| {
        if let Err(е) = о {
            return notify::покажи(&format!("Ключ «{имя}»"), &format!("Копия не началась: {е}"), "⚠️");
        }
        notify::покажи(&format!("Ключ «{имя}»"), "Копирую файлы на ключ — не вынимайте его", "🔑");
        let кончено = std::rc::Rc::new(std::cell::Cell::new(false));
        glib::timeout_add_local(Duration::from_secs(2), move || {
            if кончено.get() {
                return glib::ControlFlow::Break;
            }
            let (имя, кончено) = (имя.clone(), кончено.clone());
            позови("key.backup.state", json!({}), move |о| {
                let Ok(с) = о else { return };
                if с["идёт"].as_bool() == Some(true) || кончено.replace(true) {
                    return;
                }
                if с["ok"].as_bool() == Some(true) {
                    notify::покажи(&format!("Ключ «{имя}»"), "Файлы скопированы — ключ можно вынимать", "✅");
                } else {
                    notify::покажи(&format!("Ключ «{имя}»"), &format!("Копия не удалась: {}", с["error"].as_str().unwrap_or("неизвестно почему")), "⚠️");
                }
            });
            glib::ControlFlow::Continue
        });
    });
}

#[cfg(test)]
mod проверки {
    #[test]
    fn список_стола_как_у_агента() {
        let агент = include_str!("../../agent/key.mjs");
        for к in super::СТОЛ {
            assert!(агент.contains(&format!("'{к}'")), "{к} нет в agent/key.mjs");
        }
    }
}
