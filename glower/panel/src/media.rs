//! Что играет — по стандарту MPRIS: наша Музыка, Firefox, Telegram,
//! любой проигрыватель Linux. Быстрые настройки показывают его и дают
//! нажать «пауза» и «дальше»; клавиши мультимедиа идут сюда же.

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

const ПРЕФИКС: &str = "org.mpris.MediaPlayer2.";

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

fn свойство(ш: &gio::DBusConnection, кому: &str, имя: &str) -> Option<glib::Variant> {
    зов(ш, кому, "/org/mpris/MediaPlayer2", "org.freedesktop.DBus.Properties", "Get", Some(&("org.mpris.MediaPlayer2.Player", имя).to_variant()))
        .map(|о| о.child_value(0))
        .map(|в| в.as_variant().unwrap_or(в))
}

#[derive(Clone, Debug, Default)]
pub struct Играет {
    pub кто: String,
    pub статус: String,
    pub название: String,
    pub исполнитель: String,
}

/// Главный проигрыватель: тот, что играет, а нет такого — первый
pub fn что_играет() -> Option<Играет> {
    let ш = шина()?;
    if std::env::var_os("GLOWER_DEBUG").is_some() {
        eprintln!("glower-panel: проигрыватели {:?}", плееры());
    }
    let mut все: Vec<Играет> = плееры()
        .into_iter()
        .map(|кто| {
            let статус = свойство(&ш, &кто, "PlaybackStatus").and_then(|в| в.str().map(String::from)).unwrap_or_default();
            let мета = свойство(&ш, &кто, "Metadata");
            let словарь = мета.as_ref().map(|м| glib::VariantDict::new(Some(м)));
            let поле = |ключ: &str| -> Option<glib::Variant> { словарь.as_ref().and_then(|с| с.lookup_value(ключ, None)) };
            let название = поле("xesam:title").and_then(|в| в.str().map(String::from)).unwrap_or_default();
            let исполнитель = поле("xesam:artist").and_then(|в| в.get::<Vec<String>>()).map(|в| в.join(", ")).unwrap_or_default();
            if std::env::var_os("GLOWER_DEBUG").is_some() {
                eprintln!("glower-panel: {кто}: {статус:?} {название:?} {:?}", мета.as_ref().map(|м| м.type_().to_string()));
            }
            Играет { кто, статус, название, исполнитель }
        })
        .collect();
    все.sort_by_key(|и| match и.статус.as_str() {
        "Playing" => 0,
        "Paused" => 1,
        _ => 2,
    });
    все.into_iter().find(|и| !и.название.is_empty() || и.статус == "Playing")
}

/// Нажать кнопку у главного проигрывателя: PlayPause, Next, Previous
pub fn нажми(метод: &str) {
    let Some(и) = что_играет() else { return };
    if let Some(ш) = шина() {
        let _ = зов(&ш, &и.кто, "/org/mpris/MediaPlayer2", "org.mpris.MediaPlayer2.Player", метод, None);
    }
}
