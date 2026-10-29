//! Разговор с агентом: вызовы и общая шина.
//!
//! Агент — тот же, с которым говорит рабочий стол: POST на localhost/rpc.
//! Сеть здесь всегда ждёт, поэтому каждый вызов идёт в своём потоке, а
//! ответ возвращается в главный цикл GTK — трогать окна можно только из
//! него.

use gtk::glib;
use serde_json::{json, Value};
use std::sync::OnceLock;
use std::time::Duration;

/// Адрес агента. Оболочка передаёт его через GLOWER_URL
/// («http://localhost:8123/»); без неё — порт по умолчанию.
pub fn адрес() -> String {
    if let Ok(u) = std::env::var("GLOWER_URL") {
        if let Some(хвост) = u.strip_prefix("http://") {
            let хост = хвост.split('/').next().unwrap_or("");
            if !хост.is_empty() {
                return format!("http://{хост}");
            }
        }
    }
    let порт = std::env::var("GLOWER_PORT").unwrap_or_else(|_| "8123".into());
    format!("http://127.0.0.1:{порт}")
}

fn связь() -> &'static ureq::Agent {
    static СВЯЗЬ: OnceLock<ureq::Agent> = OnceLock::new();
    СВЯЗЬ.get_or_init(|| ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(3)).build())
}

/// Вызов, который ждёт ответа. Только из рабочих потоков.
pub fn зов(метод: &str, доводы: Value, срок: u64) -> Result<Value, String> {
    let ответ = связь()
        .post(&format!("{}/rpc", адрес()))
        .timeout(Duration::from_secs(срок))
        .send_json(json!({ "method": метод, "params": доводы }));
    let тело: Value = match ответ {
        Ok(о) => о.into_json().map_err(|е| е.to_string())?,
        // Агент отвечает отказом с телом — причину берём из него
        Err(ureq::Error::Status(_, о)) => о.into_json().map_err(|е| е.to_string())?,
        // Связь с агентом — словами, а не «http://localhost:8123/rpc: Network
        // Error: Error encountered in the status line»: человеку важно, что
        // случилось, а не адрес
        Err(е) => {
            let т = е.to_string();
            eprintln!("glower-panel: {метод}: {т}");
            return Err(if т.contains("timed out") || т.contains("Timeout") {
                "система не успела ответить — попробуйте ещё раз".into()
            } else if т.contains("refused") || т.contains("Connection Failed") {
                "служба системы ещё не запустилась — подождите немного".into()
            } else {
                т
            });
        }
    };
    if тело["ok"].as_bool() == Some(true) {
        Ok(тело["result"].clone())
    } else {
        Err(тело["error"].as_str().unwrap_or("агент отказал").to_string())
    }
}

/// Вызов из главного цикла: ответ придёт в `дело`, тоже в главном цикле.
pub fn позови<F>(метод: &str, доводы: Value, дело: F)
where
    F: FnOnce(Result<Value, String>) + 'static,
{
    позови_за(метод, доводы, 10, дело);
}

/// Сколько ждать ответа. Почти всё агент отвечает сразу, но кое-что зовёт
/// медленные программы системы: ubuntu-drivers на старом ноутбуке думает
/// полминуты, apt считает обновления до тридцати секунд, служба прошивок
/// обходит всё железо и сама ждёт до полутора минут. Раньше на всё было
/// десять секунд — и страница «Обновления» на таких машинах сплошь
/// показывала «timed out», хотя система просто ещё не договорила.
fn срок_для(метод: &str) -> u64 {
    match метод {
        "sys.firmware" | "sys.firmware.refresh" | "sys.firmware.update" => 200,
        "sys.drivers" | "pkg.upgrade.check" | "av.check" | "av.update" | "av.purge" => 150,
        _ => 0,
    }
}

/// То же, но со своим сроком ответа — для вызовов, которые сами ходят в сеть.
pub fn позови_за<F>(метод: &str, доводы: Value, срок: u64, дело: F)
where
    F: FnOnce(Result<Value, String>) + 'static,
{
    let срок = срок.max(срок_для(метод));
    let (куда, откуда) = async_channel::bounded(1);
    let метод = метод.to_string();
    std::thread::spawn(move || {
        let _ = куда.send_blocking(зов(&метод, доводы, срок));
    });
    glib::MainContext::default().spawn_local(async move {
        if let Ok(ответ) = откуда.recv().await {
            // Не изнутри задачи: в ответе часто спрашивают окном
            crate::apps::вне_задачи(move || дело(ответ));
        }
    });
}

/// Сказать столу по шине. Ответ не нужен: просьба либо дошла, либо нет.
pub fn скажи(тема: &str, что: Value) {
    позови("ui.say", json!({ "тема": тема, "что": что }), |_| {});
}

/// Весть с шины.
pub struct Весть {
    pub тема: String,
    pub что: Value,
}

/// Слушать шину. `состояния` — темы, нынешнее значение которых нужно сразу
/// (раскладка, заряд: узнал — и живи с этим). `просьбы` — однократные
/// («открой Пуск»): их прошлое не повторяем, иначе панель при запуске
/// выполнит просьбу, которую никто сейчас не делал.
///
/// Связь одна на все темы — как у страничной панели: шина держит её
/// открытой до сорока секунд, и на каждую тему по своей не напасёшься.
pub fn слушай(состояния: &[&str], просьбы: &[&str]) -> async_channel::Receiver<Весть> {
    let (куда, откуда) = async_channel::unbounded();
    let состояния: Vec<String> = состояния.iter().map(|т| т.to_string()).collect();
    let все: Vec<String> = состояния
        .iter()
        .cloned()
        .chain(просьбы.iter().map(|т| т.to_string()))
        .collect();
    std::thread::spawn(move || {
        // Сначала — с какого места слушать, потом — как дела сейчас. В
        // обратном порядке весть, пришедшая между двумя вопросами,
        // потерялась бы навсегда.
        let mut с: u64 = loop {
            match зов("ui.last", json!({ "тема": "" }), 5) {
                Ok(д) => break д["n"].as_u64().unwrap_or(0),
                Err(_) => std::thread::sleep(Duration::from_secs(2)),
            }
        };
        for тема in &состояния {
            if let Ok(д) = зов("ui.last", json!({ "тема": тема }), 5) {
                if !д["что"].is_null() {
                    let _ = куда.send_blocking(Весть { тема: тема.clone(), что: д["что"].clone() });
                }
            }
        }
        loop {
            match зов("ui.hear", json!({ "с": с, "темы": все }), 40) {
                Ok(д) => {
                    // Агент перезапустился — счёт у него пошёл с нуля, и
                    // со старым номером мы не услышали бы больше ничего.
                    if д["n"].as_u64().is_some_and(|n| n < с) {
                        с = 0;
                    }
                    if let Some(список) = д["список"].as_array() {
                        for м in список {
                            let n = м["n"].as_u64().unwrap_or(0);
                            if n <= с {
                                continue;
                            }
                            let тема = м["тема"].as_str().unwrap_or("").to_string();
                            if куда.send_blocking(Весть { тема, что: м["что"].clone() }).is_err() {
                                return;
                            }
                            с = с.max(n);
                        }
                    }
                    if let Some(n) = д["n"].as_u64() {
                        с = с.max(n);
                    }
                }
                // Агент перезапускается или машина просыпается — подождём
                Err(_) => std::thread::sleep(Duration::from_secs(3)),
            }
        }
    });
    откуда
}
