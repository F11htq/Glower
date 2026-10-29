//! org.freedesktop.FileManager1 — «Показать в папке» для чужих программ.
//!
//! Firefox после скачивания, Chrome, Telegram, Thunderbird по щелчку
//! «Открыть папку» зовут эту службу на шине сеанса и просят показать файл.
//! Её не было, и в лучшем случае программа открывала папку как придётся, а
//! в худшем не открывалось ничего. Служба живёт в панели — она запущена
//! всегда — и открывает Проводник на папке файла, с самим файлом
//! выделенным, как Windows по «Показать в папке».

use gtk::gio;
use gtk::prelude::*;

const ИМЯ: &str = "org.freedesktop.FileManager1";
const ПУТЬ: &str = "/org/freedesktop/FileManager1";
const ОПИСАНИЕ: &str = r#"<node>
  <interface name="org.freedesktop.FileManager1">
    <method name="ShowFolders">
      <arg type="as" name="URIs" direction="in"/>
      <arg type="s" name="StartupId" direction="in"/>
    </method>
    <method name="ShowItems">
      <arg type="as" name="URIs" direction="in"/>
      <arg type="s" name="StartupId" direction="in"/>
    </method>
    <method name="ShowItemProperties">
      <arg type="as" name="URIs" direction="in"/>
      <arg type="s" name="StartupId" direction="in"/>
    </method>
  </interface>
</node>"#;

/// Ссылка (file:///…) или путь → путь на диске
pub fn путь_из(что: &str) -> Option<std::path::PathBuf> {
    if что.contains("://") {
        gio::File::for_uri(что).path()
    } else {
        Some(std::path::PathBuf::from(что))
    }
}

fn открой(метод: &str, ссылки: &[String]) {
    // Каждую папку — своим окном, как просят; но не больше трёх: чужая
    // программа может прислать сотню выделенных файлов разом
    let mut папки_уже: Vec<std::path::PathBuf> = Vec::new();
    for с in ссылки.iter().take(20) {
        let Some(п) = путь_из(с) else { continue };
        let (программа, довод) = match метод {
            "ShowItemProperties" => ("glower-props", п.clone()),
            // Файл — его папку, с ним выделенным; папку — её саму
            _ => ("glower-files", п.clone()),
        };
        let ключ = if метод == "ShowItems" { п.parent().map(|р| р.to_path_buf()).unwrap_or(п.clone()) } else { п.clone() };
        if папки_уже.contains(&ключ) {
            continue;
        }
        папки_уже.push(ключ);
        if папки_уже.len() > 3 {
            break;
        }
        if let Err(е) = std::process::Command::new(программа).arg(&довод).spawn() {
            eprintln!("glower-panel: FileManager1: {программа}: {е}");
        }
    }
}

/// Занять имя службы на шине сеанса. Если оно уже занято (стоит другой
/// файловый менеджер) — не отбираем: пусть отвечает он
pub fn подключи() {
    let id = gio::bus_own_name(
        gio::BusType::Session,
        ИМЯ,
        gio::BusNameOwnerFlags::NONE,
        |связь, _| {
            let Ok(узел) = gio::DBusNodeInfo::for_xml(ОПИСАНИЕ) else { return };
            let Some(интерфейс) = узел.lookup_interface(ИМЯ) else { return };
            let итог = связь.register_object(
                ПУТЬ,
                &интерфейс,
                |_, _, _, _, метод, доводы, ответ| {
                    let ссылки: Vec<String> = доводы.child_value(0).get().unwrap_or_default();
                    let метод = метод.to_string();
                    // Сюда приходят из потока шины — окна запускаем в главном
                    gtk::glib::MainContext::default().invoke(move || открой(&метод, &ссылки));
                    ответ.return_value(None);
                },
                |_, _, _, _, _| gtk::glib::Variant::from(()),
                |_, _, _, _, _, _| false,
            );
            if let Err(е) = итог {
                eprintln!("glower-panel: FileManager1 не встал: {е}");
            }
        },
        |_, _| eprintln!("glower-panel: показываю файлы для других программ (FileManager1)"),
        |_, _| {},
    );
    std::mem::forget(id);
}

#[cfg(test)]
mod проверки {
    use super::*;

    #[test]
    fn ссылки_и_пути() {
        assert_eq!(путь_из("file:///home/me/%D0%97%D0%B0%D0%B3%D1%80%D1%83%D0%B7%D0%BA%D0%B8/a%20b.deb").unwrap(), std::path::PathBuf::from("/home/me/Загрузки/a b.deb"));
        assert_eq!(путь_из("/tmp/x").unwrap(), std::path::PathBuf::from("/tmp/x"));
        assert!(путь_из("https://example.com/").is_none());
    }
}
