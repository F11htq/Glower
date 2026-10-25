//! Общие дела с файлами — для Проводника, стола и дока: копировать,
//! перенести, бросить мышью, отправить в корзину и общий буфер обмена.
//!
//! Буфер — системный, в том же виде, что у Nautilus и других программ
//! (x-special/gnome-copied-files и text/uri-list). Поэтому файл,
//! скопированный в Проводнике, вставляется на стол и обратно: это разные
//! программы, и своего буфера у каждой было бы мало.

use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use std::path::{Path, PathBuf};

/// Свободное имя в папке: «имя», а занято — «имя (2)», «имя (3)»…
pub fn свободное_имя(папка: &Path, имя: &str) -> PathBuf {
    let п = папка.join(имя);
    if !п.exists() && п.symlink_metadata().is_err() {
        return п;
    }
    let (основа, хвост) = match имя.rfind('.') {
        Some(т) if т > 0 => (&имя[..т], &имя[т..]),
        _ => (имя, ""),
    };
    (2..).map(|н| папка.join(format!("{основа} ({н}){хвост}"))).find(|п| !п.exists()).unwrap()
}

pub fn скопируй(откуда: &Path, куда: &Path) -> std::io::Result<()> {
    let м = откуда.symlink_metadata()?;
    if м.file_type().is_symlink() {
        std::os::unix::fs::symlink(std::fs::read_link(откуда)?, куда)
    } else if м.is_dir() {
        std::fs::create_dir(куда)?;
        for з in std::fs::read_dir(откуда)? {
            let з = з?;
            скопируй(&з.path(), &куда.join(з.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(откуда, куда).map(|_| ())
    }
}

pub fn перенеси(откуда: &Path, куда: &Path) -> std::io::Result<()> {
    match std::fs::rename(откуда, куда) {
        Ok(()) => Ok(()),
        // Другой диск: переименованием не перенести — копируем и убираем
        Err(е) if е.raw_os_error() == Some(18) => {
            скопируй(откуда, куда)?;
            if откуда.is_dir() && !откуда.symlink_metadata()?.file_type().is_symlink() {
                std::fs::remove_dir_all(откуда)
            } else {
                std::fs::remove_file(откуда)
            }
        }
        Err(е) => Err(е),
    }
}

/// Итог вставки: что появилось в папке и что не вышло
#[derive(Default, Debug)]
pub struct Итог {
    pub новые: Vec<PathBuf>,
    pub беды: Vec<String>,
}

/// Вставить пути в папку: копией или переносом
pub fn вставь(пути: &[PathBuf], папка: &Path, вырезать: bool) -> Итог {
    let mut итог = Итог::default();
    for п in пути {
        let имя = п.file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_default();
        if папка.starts_with(п) {
            итог.беды.push(format!("«{имя}»: папку нельзя положить в саму себя"));
            continue;
        }
        // Перенос туда же, где лежит, — ничего не делать
        if вырезать && п.parent() == Some(папка) {
            continue;
        }
        let куда = свободное_имя(папка, &имя);
        let р = if вырезать { перенеси(п, &куда) } else { скопируй(п, &куда) };
        match р {
            Ok(()) => итог.новые.push(куда),
            Err(е) => итог.беды.push(format!("«{имя}»: {е}")),
        }
    }
    итог
}

/// То же, но в отдельном потоке: большие папки копируются долго, а окно
/// должно жить. Готово — зовём в главном потоке
pub fn вставь_в_фоне(пути: Vec<PathBuf>, папка: PathBuf, вырезать: bool, готово: impl FnOnce(Итог) + 'static) {
    let (куда, откуда) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = куда.send_blocking(вставь(&пути, &папка, вырезать));
    });
    glib::MainContext::default().spawn_local(async move {
        if let Ok(итог) = откуда.recv().await {
            готово(итог);
        }
    });
}

fn диск(п: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    std::fs::symlink_metadata(п).map(|м| м.dev()).ok()
}

/// Бросили мышью: как в Windows — на тот же диск переносим, на другой
/// копируем; с Ctrl — всегда копия, с Shift — всегда перенос
pub fn переносить(пути: &[PathBuf], папка: &Path) -> bool {
    let м = клавиши();
    if м.contains(gdk::ModifierType::CONTROL_MASK) {
        return false;
    }
    if м.contains(gdk::ModifierType::SHIFT_MASK) {
        return true;
    }
    let д = диск(папка);
    пути.iter().all(|п| диск(п) == д)
}

/// Какие клавиши-модификаторы зажаты сейчас
fn клавиши() -> gdk::ModifierType {
    gdk::Display::default()
        .and_then(|д| gdk::Keymap::for_display(&д))
        .map(|к| gdk::ModifierType::from_bits_truncate(к.modifier_state()))
        .unwrap_or_else(gdk::ModifierType::empty)
}

/// Бросить пути в папку — с правилом выше, в фоне
pub fn брось(пути: Vec<PathBuf>, папка: PathBuf, готово: impl FnOnce(Итог) + 'static) {
    let пути: Vec<PathBuf> = пути.into_iter().filter(|п| *п != папка).collect();
    if пути.is_empty() {
        return готово(Итог::default());
    }
    let вырезать = переносить(&пути, &папка);
    вставь_в_фоне(пути, папка, вырезать, готово);
}

/// В корзину; что не вышло — списком
pub fn в_корзину(пути: &[PathBuf]) -> Vec<String> {
    let mut беды = Vec::new();
    for п in пути {
        if let Err(е) = gio::File::for_path(п).trash(gio::Cancellable::NONE) {
            let имя = п.file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_default();
            беды.push(format!("«{имя}»: {е}"));
        }
    }
    беды
}

/// Ссылки file:// — в пути; чужое (http и прочее) пропускаем
pub fn пути_из_ссылок<S: AsRef<str>>(ссылки: &[S]) -> Vec<PathBuf> {
    ссылки.iter().filter_map(|у| gio::File::for_uri(у.as_ref()).path()).collect()
}

pub fn ссылки(пути: &[PathBuf]) -> Vec<String> {
    пути.iter().map(|п| gio::File::for_path(п).uri().to_string()).collect()
}

/// Что таскают мышью между программами: список файлов
pub fn цели_переноса() -> Vec<gtk::TargetEntry> {
    vec![gtk::TargetEntry::new("text/uri-list", gtk::TargetFlags::empty(), 0)]
}

pub fn атом_ссылок() -> gdk::Atom {
    gdk::Atom::intern("text/uri-list")
}

/* ---------- буфер обмена ---------- */

const ФАЙЛЫ_GNOME: &str = "x-special/gnome-copied-files";

fn буфер() -> gtk::Clipboard {
    gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD)
}

/// Положить файлы в буфер: «копировать» или «вырезать». Заодно — путями
/// текстом: вставятся в терминал или письмо
pub fn в_буфер(пути: Vec<PathBuf>, вырезать: bool) {
    if пути.is_empty() {
        return;
    }
    let цели = [
        gtk::TargetEntry::new(ФАЙЛЫ_GNOME, gtk::TargetFlags::empty(), 0),
        gtk::TargetEntry::new("text/uri-list", gtk::TargetFlags::empty(), 1),
        gtk::TargetEntry::new("UTF8_STRING", gtk::TargetFlags::empty(), 2),
        gtk::TargetEntry::new("text/plain;charset=utf-8", gtk::TargetFlags::empty(), 2),
        gtk::TargetEntry::new("text/plain", gtk::TargetFlags::empty(), 2),
    ];
    let ссылки = ссылки(&пути);
    буфер().set_with_data(&цели, move |_, данные, что| match что {
        0 => {
            let текст = format!("{}\n{}", if вырезать { "cut" } else { "copy" }, ссылки.join("\n"));
            данные.set(&gdk::Atom::intern(ФАЙЛЫ_GNOME), 8, текст.as_bytes());
        }
        1 => {
            let с: Vec<&str> = ссылки.iter().map(String::as_str).collect();
            данные.set_uris(&с);
        }
        _ => {
            let текст = пути.iter().map(|п| п.to_string_lossy().to_string()).collect::<Vec<_>>().join("\n");
            данные.set_text(&текст);
        }
    });
}

/// Есть ли в буфере файлы — чтобы «Вставить» в меню было живым
pub fn в_буфере_файлы() -> bool {
    let (цели, _) = буфер().wait_for_targets().map(|ц| (ц, ())).unwrap_or_default();
    цели.iter().any(|ц| {
        let имя = ц.name();
        имя == ФАЙЛЫ_GNOME || имя == "text/uri-list"
    })
}

/// Разобрать x-special/gnome-copied-files: первая строка — copy или cut
pub fn разбери_gnome(текст: &str) -> (Vec<PathBuf>, bool) {
    let mut строки = текст.lines();
    let вырезать = строки.next().map(str::trim) == Some("cut");
    let ссылки: Vec<&str> = строки.map(str::trim).filter(|с| !с.is_empty()).collect();
    (пути_из_ссылок(&ссылки), вырезать)
}

/// Взять файлы из буфера: пути и «вырезано ли»
pub fn из_буфера(готово: impl FnOnce(Vec<PathBuf>, bool) + 'static) {
    буфер().request_contents(&gdk::Atom::intern(ФАЙЛЫ_GNOME), move |б, данные| {
        let байты = данные.data();
        if !байты.is_empty() {
            let (пути, вырезать) = разбери_gnome(&String::from_utf8_lossy(&байты));
            if !пути.is_empty() {
                return готово(пути, вырезать);
            }
        }
        // Программа, что положила файлы, нашего вида не знает — просто ссылки
        б.request_uris(move |_, ссылки| {
            let с: Vec<String> = ссылки.iter().map(|у| у.to_string()).collect();
            готово(пути_из_ссылок(&с), false);
        });
    });
}

/// Вставить из буфера в папку. Вырезанное после вставки из буфера уходит —
/// второй раз вставлять уже нечего, как в Windows
pub fn вставь_из_буфера(папка: PathBuf, готово: impl FnOnce(Итог) + 'static) {
    из_буфера(move |пути, вырезать| {
        if пути.is_empty() {
            return;
        }
        if вырезать {
            буфер().set_text("");
        }
        вставь_в_фоне(пути, папка, вырезать, готово);
    });
}

/* ---------- значок под мышью ---------- */

/// Картинка для переноса: значок и, если файлов несколько, их число в
/// кружке — как в Windows
pub fn картинка_переноса(вид: &gtk::Widget, сколько: usize, за: (f64, f64)) -> Option<gtk::cairo::ImageSurface> {
    let (ш, в) = (вид.allocated_width().max(1), вид.allocated_height().max(1));
    let к_ш = ш + 12;
    let картинка = gtk::cairo::ImageSurface::create(gtk::cairo::Format::ARgb32, к_ш, в + 6).ok()?;
    {
        let к = gtk::cairo::Context::new(&картинка).ok()?;
        к.push_group();
        вид.draw(&к);
        let _ = к.pop_group_to_source();
        let _ = к.paint_with_alpha(0.8);
        if сколько > 1 {
            let текст = сколько.to_string();
            к.set_font_size(11.0);
            let р = 9.0;
            let (cx, cy) = (ш as f64 + 2.0, р + 1.0);
            к.arc(cx, cy, р, 0.0, std::f64::consts::TAU);
            к.set_source_rgb(0.23, 0.51, 0.96);
            let _ = к.fill();
            if let Ok(р_т) = к.text_extents(&текст) {
                к.move_to(cx - р_т.width() / 2.0 - р_т.x_bearing(), cy + р_т.height() / 2.0);
            }
            к.set_source_rgb(1.0, 1.0, 1.0);
            let _ = к.show_text(&текст);
        }
    }
    картинка.set_device_offset(-за.0, -за.1);
    Some(картинка)
}

#[cfg(test)]
mod проверки {
    use super::*;

    #[test]
    fn копирует_переносит_и_не_затирает() {
        let корень = std::env::temp_dir().join(format!("glower-fileops-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&корень);
        let (а, б) = (корень.join("а"), корень.join("б"));
        std::fs::create_dir_all(а.join("внутри")).unwrap();
        std::fs::create_dir_all(&б).unwrap();
        std::fs::write(а.join("внутри").join("x.txt"), "привет").unwrap();
        std::fs::write(б.join("x.txt"), "старое").unwrap();
        // Копия папки — со всем, что внутри
        let и = вставь(&[а.join("внутри")], &б, false);
        assert!(и.беды.is_empty());
        assert_eq!(и.новые, vec![б.join("внутри")]);
        assert_eq!(std::fs::read_to_string(б.join("внутри").join("x.txt")).unwrap(), "привет");
        // Занятое имя не затираем — рядом встаёт «x (2).txt»
        let и = вставь(&[а.join("внутри").join("x.txt")], &б, true);
        assert!(и.беды.is_empty());
        assert_eq!(и.новые, vec![б.join("x (2).txt")]);
        assert_eq!(std::fs::read_to_string(б.join("x.txt")).unwrap(), "старое");
        assert_eq!(std::fs::read_to_string(б.join("x (2).txt")).unwrap(), "привет");
        assert!(!а.join("внутри").join("x.txt").exists());
        // Папку в саму себя — нельзя
        assert_eq!(вставь(&[б.clone()], &б.join("внутри"), false).беды.len(), 1);
        let _ = std::fs::remove_dir_all(&корень);
    }

    #[test]
    fn буфер_gnome_разбирается() {
        let (п, вырезать) = разбери_gnome("cut\nfile:///tmp/%D0%B0.txt\nfile:///tmp/b\n");
        assert!(вырезать);
        assert_eq!(п, vec![PathBuf::from("/tmp/а.txt"), PathBuf::from("/tmp/b")]);
        let (п, вырезать) = разбери_gnome("copy\nfile:///x");
        assert!(!вырезать);
        assert_eq!(п, vec![PathBuf::from("/x")]);
    }
}
