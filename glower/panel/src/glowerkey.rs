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

use crate::agent::позови;
use crate::core::{задай, настройка};
use crate::notify;
use gtk::glib;
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

/// Применить стол с ключа на флешке `dev`
pub fn примени_стол(dev: &str) {
    let dev = dev.to_string();
    позови("key.profile.read", json!({ "dev": dev }), move |о| {
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
            Box::new(move || примени_стол(&dev2)),
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
            Box::new(move || продолжи(&dev2)),
        );
    });
}

/// Продолжить с места: файлы, вкладки и буфер открывает агент, программы — мы
pub fn продолжи(dev: &str) {
    позови("relay.continue", json!({ "dev": dev }), |о| match о {
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
