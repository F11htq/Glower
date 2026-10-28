//! Что играет — по стандарту MPRIS: наша Музыка, Firefox, Chromium,
//! Telegram, Spotify, VLC — любой проигрыватель Linux. Плеер на верхней
//! полосе и быстрые настройки показывают его и дают нажать «пауза»,
//! «назад» и «дальше»; клавиши мультимедиа идут сюда же.

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

const ПРЕФИКС: &str = "org.mpris.MediaPlayer2.";
const ПУТЬ: &str = "/org/mpris/MediaPlayer2";

fn шина() -> Option<gio::DBusConnection> {
    gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).ok()
}

fn зов(ш: &gio::DBusConnection, кому: &str, путь: &str, интерфейс: &str, метод: &str, доводы: Option<&glib::Variant>) -> Option<glib::Variant> {
    ш.call_sync(Some(кому), путь, интерфейс, метод, доводы, None, gio::DBusCallFlags::NONE, 400, gio::Cancellable::NONE).ok()
}

/// Все проигрыватели на шине
pub fn плееры() -> Vec<String> {
    let Some(ш) = шина() else { return Vec::new() };
    зов(&ш, "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "ListNames", None)
        .and_then(|о| о.child_value(0).get::<Vec<String>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|и| и.starts_with(ПРЕФИКС))
        .collect()
}

fn свойство_из(ш: &gio::DBusConnection, кому: &str, интерфейс: &str, имя: &str) -> Option<glib::Variant> {
    зов(ш, кому, ПУТЬ, "org.freedesktop.DBus.Properties", "Get", Some(&(интерфейс, имя).to_variant()))
        .map(|о| о.child_value(0))
        .map(|в| в.as_variant().unwrap_or(в))
}

fn свойство(ш: &gio::DBusConnection, кому: &str, имя: &str) -> Option<glib::Variant> {
    свойство_из(ш, кому, "org.mpris.MediaPlayer2.Player", имя)
}

/// Число из варианта: проигрыватели пишут длину и позицию кто как — x, t, i, d
fn число(в: &glib::Variant) -> Option<i64> {
    в.get::<i64>()
        .or_else(|| в.get::<u64>().map(|ч| ч as i64))
        .or_else(|| в.get::<i32>().map(i64::from))
        .or_else(|| в.get::<u32>().map(i64::from))
        .or_else(|| в.get::<f64>().map(|ч| ч as i64))
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Играет {
    /// Имя на шине — по нему и шлём команды
    pub кто: String,
    pub статус: String,
    pub название: String,
    pub исполнитель: String,
    pub альбом: String,
    /// Обложка: file://, https:// или data:
    pub обложка: String,
    /// Длина в микросекундах; 0 — неизвестна (поток, прямой эфир)
    pub длина: i64,
    /// Чей плеер: «Firefox», «Музыка», «Telegram»
    pub программа: String,
    /// Ярлык программы (firefox, glower-music) — для значка
    pub ярлык: String,
    pub назад: bool,
    pub вперёд: bool,
}

impl Играет {
    pub fn играет(&self) -> bool {
        self.статус == "Playing"
    }
}

fn прочти(ш: &gio::DBusConnection, кто: String) -> Играет {
    let статус = свойство(ш, &кто, "PlaybackStatus").and_then(|в| в.str().map(String::from)).unwrap_or_default();
    let мета = свойство(ш, &кто, "Metadata");
    let словарь = мета.as_ref().map(|м| glib::VariantDict::new(Some(м)));
    let поле = |ключ: &str| -> Option<glib::Variant> { словарь.as_ref().and_then(|с| с.lookup_value(ключ, None)) };
    let строка = |ключ: &str| поле(ключ).and_then(|в| в.str().map(String::from)).unwrap_or_default();
    let список = |ключ: &str| поле(ключ).and_then(|в| в.get::<Vec<String>>()).map(|в| в.join(", ")).unwrap_or_default();
    let флаг = |имя: &str| свойство(ш, &кто, имя).and_then(|в| в.get::<bool>()).unwrap_or(false);
    let корень = |имя: &str| свойство_из(ш, &кто, "org.mpris.MediaPlayer2", имя).and_then(|в| в.str().map(String::from)).unwrap_or_default();
    let mut программа = корень("Identity");
    if программа.is_empty() {
        // org.mpris.MediaPlayer2.firefox.instance_1_42 → firefox
        программа = кто.trim_start_matches(ПРЕФИКС).split('.').next().unwrap_or("").to_string();
    }
    Играет {
        статус,
        название: строка("xesam:title"),
        исполнитель: список("xesam:artist"),
        альбом: строка("xesam:album"),
        обложка: строка("mpris:artUrl"),
        длина: поле("mpris:length").as_ref().and_then(число).unwrap_or(0),
        программа,
        ярлык: корень("DesktopEntry"),
        назад: флаг("CanGoPrevious"),
        вперёд: флаг("CanGoNext"),
        кто,
    }
}

/// Все проигрыватели, где есть что показать
pub fn все() -> Vec<Играет> {
    let Some(ш) = шина() else { return Vec::new() };
    плееры()
        .into_iter()
        .map(|кто| прочти(&ш, кто))
        .filter(|и| !и.название.is_empty() || и.играет())
        .collect()
}

/// Главный проигрыватель: тот, что играет, а нет такого — на паузе
pub fn что_играет() -> Option<Играет> {
    let mut все = все();
    все.sort_by_key(|и| match и.статус.as_str() {
        "Playing" => 0,
        "Paused" => 1,
        _ => 2,
    });
    if std::env::var_os("GLOWER_DEBUG").is_some() {
        eprintln!("glower-panel: проигрыватели {:?}", все.iter().map(|и| (&и.кто, &и.статус, &и.название)).collect::<Vec<_>>());
    }
    все.into_iter().next()
}

/// Сколько уже проиграно, в микросекундах
pub fn позиция(кто: &str) -> Option<i64> {
    let ш = шина()?;
    свойство(&ш, кто, "Position").as_ref().and_then(число)
}

/// Нажать кнопку у этого проигрывателя: PlayPause, Next, Previous
pub fn нажми_у(кто: &str, метод: &str) {
    if let Some(ш) = шина() {
        let _ = зов(&ш, кто, ПУТЬ, "org.mpris.MediaPlayer2.Player", метод, None);
    }
}

/// Нажать кнопку у главного проигрывателя (клавиши мультимедиа)
pub fn нажми(метод: &str) {
    if let Some(и) = что_играет() {
        нажми_у(&и.кто, метод);
    }
}

/// Показать окно проигрывателя (вкладку браузера, Музыку)
pub fn подними(кто: &str) {
    if let Some(ш) = шина() {
        let _ = зов(&ш, кто, ПУТЬ, "org.mpris.MediaPlayer2", "Raise", None);
    }
}

/// Слушать перемены: проигрыватель появился, ушёл или сменил трек. Зовём
/// `дело` в главном потоке; что именно поменялось — пусть спросит сам
pub fn следи(дело: impl Fn() + 'static) {
    let Some(ш) = шина() else { return };
    let дело = std::rc::Rc::new(дело);
    let (куда, откуда) = async_channel::unbounded::<()>();
    let куда2 = куда.clone();
    // Сигналы приходят в главный поток GLib, но замыкание должно быть
    // Send — передаём весточку каналом
    ш.signal_subscribe(None, Some("org.freedesktop.DBus.Properties"), Some("PropertiesChanged"), Some(ПУТЬ), None, gio::DBusSignalFlags::NONE, move |_, _, _, _, _, _| {
        let _ = куда.try_send(());
    });
    ш.signal_subscribe(Some("org.freedesktop.DBus"), Some("org.freedesktop.DBus"), Some("NameOwnerChanged"), None, Some(ПРЕФИКС.trim_end_matches('.')), gio::DBusSignalFlags::MATCH_ARG0_NAMESPACE, move |_, _, _, _, _, _| {
        let _ = куда2.try_send(());
    });
    glib::MainContext::default().spawn_local(async move {
        while откуда.recv().await.is_ok() {
            // Пачку сигналов (трек сменился — пять свойств) — одним разом
            while откуда.try_recv().is_ok() {}
            дело();
        }
    });
    // Шину держим, пока живёт программа
    std::mem::forget(ш);
}

/// «3:07» из микросекунд
pub fn время(мкс: i64) -> String {
    let с = (мкс.max(0) / 1_000_000) as u64;
    if с >= 3600 {
        format!("{}:{:02}:{:02}", с / 3600, с / 60 % 60, с % 60)
    } else {
        format!("{}:{:02}", с / 60, с % 60)
    }
}

#[cfg(test)]
mod проверки {
    use super::*;

    #[test]
    fn время_словами() {
        assert_eq!(время(187_000_000), "3:07");
        assert_eq!(время(3_725_000_000), "1:02:05");
        assert_eq!(время(-5), "0:00");
    }
}
