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
    /// Что откуда куда легло — для Ctrl+Z
    pub пары: Vec<(PathBuf, PathBuf)>,
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
            Ok(()) => {
                итог.пары.push((п.clone(), куда.clone()));
                итог.новые.push(куда);
            }
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
            if !итог.пары.is_empty() {
                запомни(if вырезать { Дело::Перенос(итог.пары.clone()) } else { Дело::Копия(итог.новые.clone()) });
            }
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
    let mut ушли = Vec::new();
    for п in пути {
        match gio::File::for_path(п).trash(gio::Cancellable::NONE) {
            Ok(_) => ушли.push(п.clone()),
            Err(е) => {
                let имя = п.file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_default();
                беды.push(format!("«{имя}»: {е}"));
            }
        }
    }
    if !ушли.is_empty() {
        запомни(Дело::Корзина(ушли));
    }
    беды
}

/// Удалить насовсем, мимо корзины (Shift+Delete). Вернуть нельзя
pub fn удали_навсегда(пути: &[PathBuf]) -> Vec<String> {
    let mut беды = Vec::new();
    for п in пути {
        let папка = п.symlink_metadata().map(|м| м.is_dir()).unwrap_or(false);
        let р = if папка { std::fs::remove_dir_all(п) } else { std::fs::remove_file(п) };
        if let Err(е) = р {
            let имя = п.file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_default();
            беды.push(format!("«{имя}»: {е}"));
        }
    }
    беды
}

/* ---------- Ctrl+Z для файлов ---------- */

/// Что сделали с файлами — чтобы отменить, как в Проводнике Windows
#[derive(Debug, Clone, PartialEq)]
pub enum Дело {
    /// Перенесли или переименовали: откуда → куда
    Перенос(Vec<(PathBuf, PathBuf)>),
    /// Сделали копии — отмена убирает их в корзину
    Копия(Vec<PathBuf>),
    /// Убрали в корзину — отмена достаёт обратно
    Корзина(Vec<PathBuf>),
    /// Создали папку или файл
    Создание(Vec<PathBuf>),
}

/// Журнал общий для стола и всех окон Проводника — это разные программы,
/// а Ctrl+Z должен отменить последнее, где бы его ни сделали. Лежит в
/// каталоге сеанса: после перезагрузки отменять уже нечего, как в Windows
fn путь_журнала() -> PathBuf {
    glib::user_runtime_dir().join("glower-otmena.json")
}

fn в_json(д: &Дело) -> serde_json::Value {
    let пути = |с: &[PathBuf]| serde_json::json!(с.iter().map(|п| п.to_string_lossy()).collect::<Vec<_>>());
    match д {
        Дело::Перенос(пары) => serde_json::json!({ "что": "перенос", "пары": пары.iter().map(|(а, б)| [а.to_string_lossy(), б.to_string_lossy()]).collect::<Vec<_>>() }),
        Дело::Копия(с) => serde_json::json!({ "что": "копия", "пути": пути(с) }),
        Дело::Корзина(с) => serde_json::json!({ "что": "корзина", "пути": пути(с) }),
        Дело::Создание(с) => serde_json::json!({ "что": "создание", "пути": пути(с) }),
    }
}

fn из_json(з: &serde_json::Value) -> Option<Дело> {
    let пути = || -> Vec<PathBuf> { з["пути"].as_array().map(|а| а.iter().filter_map(|п| п.as_str().map(PathBuf::from)).collect()).unwrap_or_default() };
    Some(match з["что"].as_str()? {
        "перенос" => Дело::Перенос(
            з["пары"]
                .as_array()?
                .iter()
                .filter_map(|п| Some((PathBuf::from(п[0].as_str()?), PathBuf::from(п[1].as_str()?))))
                .collect(),
        ),
        "копия" => Дело::Копия(пути()),
        "корзина" => Дело::Корзина(пути()),
        "создание" => Дело::Создание(пути()),
        _ => return None,
    })
}

fn прочти_журнал() -> Vec<serde_json::Value> {
    std::fs::read_to_string(путь_журнала()).ok().and_then(|т| serde_json::from_str(&т).ok()).unwrap_or_default()
}

/// Записать дело в журнал (хранится полсотни последних)
pub fn запомни(д: Дело) {
    let mut ж = прочти_журнал();
    ж.push(в_json(&д));
    let лишних = ж.len().saturating_sub(50);
    ж.drain(..лишних);
    let _ = std::fs::write(путь_журнала(), serde_json::Value::Array(ж).to_string());
}

/// Достать из корзины то, что лежало по этому пути (последнее удалённое)
fn верни_из_корзины(путь: &Path) -> Result<PathBuf, String> {
    let корзина = glib::user_data_dir().join("Trash");
    let искомое = путь.to_string_lossy().to_string();
    let mut лучшее: Option<(String, String)> = None;
    for з in std::fs::read_dir(корзина.join("info")).map_err(|е| е.to_string())?.filter_map(|з| з.ok()) {
        let Ok(т) = std::fs::read_to_string(з.path()) else { continue };
        let откуда = т.lines().find_map(|с| с.strip_prefix("Path=")).map(|с| glib::uri_unescape_string(с, None::<&str>).map(|г| г.to_string()).unwrap_or_else(|| с.to_string()));
        if откуда.as_deref() != Some(искомое.as_str()) {
            continue;
        }
        let когда = т.lines().find_map(|с| с.strip_prefix("DeletionDate=")).unwrap_or("").to_string();
        let имя = з.file_name().to_string_lossy().trim_end_matches(".trashinfo").to_string();
        if лучшее.as_ref().is_none_or(|(к, _)| когда > *к) {
            лучшее = Some((когда, имя));
        }
    }
    let (_, имя) = лучшее.ok_or("в корзине его уже нет")?;
    let папка = путь.parent().ok_or("неизвестно, куда вернуть")?;
    let _ = std::fs::create_dir_all(папка);
    let куда = свободное_имя(папка, &путь.file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_default());
    перенеси(&корзина.join("files").join(&имя), &куда).map_err(|е| е.to_string())?;
    let _ = std::fs::remove_file(корзина.join("info").join(format!("{имя}.trashinfo")));
    Ok(куда)
}

/// Выполнить обратное делу. Что не вышло — списком
pub fn обрати(д: &Дело) -> Vec<String> {
    let имя = |п: &Path| п.file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_default();
    let mut беды = Vec::new();
    match д {
        Дело::Перенос(пары) => {
            for (откуда, куда) in пары.iter().rev() {
                if !куда.exists() && куда.symlink_metadata().is_err() {
                    беды.push(format!("«{}»: его уже нет на месте", имя(куда)));
                    continue;
                }
                let назад = match откуда.parent() {
                    Some(папка) if откуда.exists() => свободное_имя(папка, &имя(откуда)),
                    _ => откуда.clone(),
                };
                if let Some(папка) = назад.parent() {
                    let _ = std::fs::create_dir_all(папка);
                }
                if let Err(е) = перенеси(куда, &назад) {
                    беды.push(format!("«{}»: {е}", имя(куда)));
                }
            }
        }
        Дело::Копия(с) | Дело::Создание(с) => {
            for п in с {
                if п.exists() {
                    if let Err(е) = gio::File::for_path(п).trash(gio::Cancellable::NONE) {
                        беды.push(format!("«{}»: {е}", имя(п)));
                    }
                }
            }
        }
        Дело::Корзина(с) => {
            for п in с {
                if let Err(е) = верни_из_корзины(п) {
                    беды.push(format!("«{}»: {е}", имя(п)));
                }
            }
        }
    }
    беды
}

/// Ctrl+Z: отменить последнее дело с файлами. None — отменять нечего
pub fn отмени_последнее(готово: impl FnOnce(Option<Vec<String>>) + 'static) {
    let mut ж = прочти_журнал();
    let Some(д) = ж.pop().as_ref().and_then(из_json) else {
        let _ = std::fs::write(путь_журнала(), serde_json::Value::Array(ж).to_string());
        return готово(None);
    };
    let _ = std::fs::write(путь_журнала(), serde_json::Value::Array(ж).to_string());
    let (куда, откуда) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = куда.send_blocking(обрати(&д));
    });
    glib::MainContext::default().spawn_local(async move {
        if let Ok(беды) = откуда.recv().await {
            готово(Some(беды));
        }
    });
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

/// Окно программы принимает брошенные в него файлы — «открыть их»,
/// как в Windows: бросил картинку на Paint — она открылась
pub fn принимай_в_окно<W: IsA<gtk::Widget>>(в: &W, открой: impl Fn(Vec<PathBuf>) + 'static) {
    в.drag_dest_set(gtk::DestDefaults::ALL, &цели_переноса(), gdk::DragAction::COPY);
    в.connect_drag_data_received(move |_, _, _, _, данные, _, _| {
        let пути = пути_из_ссылок(&данные.uris());
        if !пути.is_empty() {
            открой(пути);
        }
    });
}

/* ---------- архивы ---------- */

const АРХИВЫ: &[&str] = &[".tar.gz", ".tar.xz", ".tar.bz2", ".tar.zst", ".tgz", ".txz", ".tbz", ".tbz2", ".zip", ".rar", ".7z", ".tar", ".cab", ".cpio", ".xar", ".lzh"];

/// Имя без расширения архива: «Отчёт.tar.gz» → «Отчёт»
fn без_расширения(имя: &str) -> Option<String> {
    let малое = имя.to_lowercase();
    АРХИВЫ.iter().find(|р| малое.ends_with(*р) && малое.len() > р.len()).map(|р| имя[..имя.len() - р.len()].to_string())
}

pub fn архив_ли(путь: &Path) -> bool {
    путь.is_file() && путь.file_name().is_some_and(|и| без_расширения(&и.to_string_lossy()).is_some())
}

fn bsdtar(доводы: &[&std::ffi::OsStr]) -> Result<(), String> {
    match std::process::Command::new("bsdtar").args(доводы).output() {
        Err(е) if е.kind() == std::io::ErrorKind::NotFound => Err("нет программы bsdtar (пакет libarchive-tools)".into()),
        Err(е) => Err(е.to_string()),
        Ok(о) if о.status.success() => Ok(()),
        Ok(о) => {
            let т = String::from_utf8_lossy(&о.stderr).trim().to_string();
            Err(if т.contains("passphrase") || т.contains("Encrypted") { "архив под паролем — такие пока не распаковываются".into() } else { т.lines().last().unwrap_or("не вышло").to_string() })
        }
    }
}

/// Распаковать: в папку с именем архива рядом с ним (как «Извлечь всё» в
/// Windows) или прямо туда, где лежит архив. Готово — путь к тому, что
/// открыть: новая папка или та, где архив
pub fn распакуй(архив: PathBuf, в_свою_папку: bool, готово: impl FnOnce(Result<PathBuf, String>) + 'static) {
    let (куда, откуда) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let папка = архив.parent().map(Path::to_path_buf).unwrap_or_default();
        let итог = if в_свою_папку {
            let имя = без_расширения(&архив.file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_default()).unwrap_or_else(|| "Архив".into());
            let цель = свободное_имя(&папка, &имя);
            std::fs::create_dir(&цель).map_err(|е| е.to_string()).and_then(|_| {
                bsdtar(&["-xf".as_ref(), архив.as_os_str(), "-C".as_ref(), цель.as_os_str()]).map(|_| (цель.clone(), vec![цель.clone()])).inspect_err(|_| {
                    let _ = std::fs::remove_dir_all(&цель);
                })
            })
        } else {
            // Сюда: что окажется новым — то и «создано» (для Ctrl+Z)
            let было: std::collections::HashSet<_> = std::fs::read_dir(&папка).map(|ч| ч.filter_map(|з| з.ok()).map(|з| з.path()).collect()).unwrap_or_default();
            bsdtar(&["-xkf".as_ref(), архив.as_os_str(), "-C".as_ref(), папка.as_os_str()]).map(|_| {
                let стало: Vec<PathBuf> = std::fs::read_dir(&папка).map(|ч| ч.filter_map(|з| з.ok()).map(|з| з.path()).filter(|п| !было.contains(п)).collect()).unwrap_or_default();
                (папка.clone(), стало)
            })
        };
        let _ = куда.send_blocking(итог);
    });
    glib::MainContext::default().spawn_local(async move {
        if let Ok(итог) = откуда.recv().await {
            // Что появилось — в журнал: Ctrl+Z уберёт распакованное
            готово(итог.map(|(открыть, новые)| {
                if !новые.is_empty() {
                    запомни(Дело::Создание(новые));
                }
                открыть
            }));
        }
    });
}

/// Сжать в ZIP рядом с первым: один файл — «Имя.zip», несколько — «Архив.zip»
pub fn сожми(пути: Vec<PathBuf>, готово: impl FnOnce(Result<PathBuf, String>) + 'static) {
    let (куда, откуда) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let итог = (|| {
            let первый = пути.first().ok_or("нечего сжимать")?;
            let папка = первый.parent().ok_or("неизвестно, где")?.to_path_buf();
            let имя = if пути.len() == 1 {
                let и = первый.file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_default();
                // У файла — без его расширения: «Отчёт.docx» → «Отчёт.zip»
                let основа = if первый.is_dir() { и.clone() } else { и.rsplit_once('.').map(|(а, _)| а.to_string()).filter(|а| !а.is_empty()).unwrap_or(и.clone()) };
                format!("{основа}.zip")
            } else {
                "Архив.zip".into()
            };
            let цель = свободное_имя(&папка, &имя);
            let mut доводы: Vec<std::ffi::OsString> = vec!["--format".into(), "zip".into(), "-cf".into(), цель.clone().into(), "-C".into(), папка.clone().into()];
            for п in &пути {
                if п.parent() != Some(папка.as_path()) {
                    return Err("сжимать можно файлы из одной папки".to_string());
                }
                доводы.push(п.file_name().ok_or("без имени")?.to_os_string());
            }
            let ссылки: Vec<&std::ffi::OsStr> = доводы.iter().map(|д| д.as_os_str()).collect();
            bsdtar(&ссылки).inspect_err(|_| {
                let _ = std::fs::remove_file(&цель);
            })?;
            Ok(цель)
        })();
        let _ = куда.send_blocking(итог);
    });
    glib::MainContext::default().spawn_local(async move {
        if let Ok(итог) = откуда.recv().await {
            if let Ok(п) = &итог {
                запомни(Дело::Создание(vec![п.clone()]));
            }
            готово(итог);
        }
    });
}

/// Пункты для меню файлов: распаковать (у одного архива) и сжать в ZIP.
/// готово — что сделать по окончании (обновить папку, показать ошибку)
pub fn пункты_архива(меню: &gtk::Menu, пути: &[PathBuf], готово: std::rc::Rc<dyn Fn(Result<PathBuf, String>, bool)>) {
    if let [один] = пути {
        if архив_ли(один) {
            let имя = без_расширения(&один.file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_default()).unwrap_or_default();
            let пункт = crate::apps::пункт_меню(&format!("🗜 Извлечь всё в «{имя}»"));
            let (п, г) = (один.clone(), готово.clone());
            пункт.connect_activate(move |_| {
                let г = г.clone();
                распакуй(п.clone(), true, move |итог| г(итог, true));
            });
            меню.append(&пункт);
            let пункт = crate::apps::пункт_меню("Извлечь сюда");
            let (п, г) = (один.clone(), готово.clone());
            пункт.connect_activate(move |_| {
                let г = г.clone();
                распакуй(п.clone(), false, move |итог| г(итог, false));
            });
            меню.append(&пункт);
        }
    }
    if !пути.is_empty() {
        let пункт = crate::apps::пункт_меню("🗜 Сжать в ZIP");
        let (пути, г) = (пути.to_vec(), готово);
        пункт.connect_activate(move |_| {
            let г = г.clone();
            сожми(пути.clone(), move |итог| г(итог, false));
        });
        меню.append(&пункт);
    }
}

/* ---------- открыть с помощью ---------- */

/// Свои программы — на случай, если их ярлыков в системе нет (запуск не
/// из образа): исполняемый файл, имя, значок и какие типы берут
const СВОИ: &[(&str, &str, &str, &[&str])] = &[
    ("glower-notepad", "Блокнот", "accessories-text-editor", &["text/"]),
    ("glower-photos", "Фото", "image-viewer", &["image/"]),
    ("glower-paint", "Paint", "applications-graphics", &["image/png", "image/jpeg", "image/bmp", "image/gif", "image/webp", "image/tiff"]),
    ("glower-music", "Музыка", "audio-x-generic", &["audio/"]),
];

fn пункт_программы(имя: &str, значок: &gio::Icon) -> gtk::MenuItem {
    let п = gtk::MenuItem::new();
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let к = gtk::Image::from_gicon(значок, gtk::IconSize::Menu);
    к.set_pixel_size(16);
    ряд.add(&к);
    let подпись = gtk::Label::new(Some(имя));
    // Имена программ — как есть, не переводим
    подпись.style_context().add_class("no-i18n");
    ряд.add(&подпись);
    п.add(&ряд);
    п
}

/// «Открыть с помощью ›» для меню файла, как в Windows: подходящие
/// программы и «Выбрать другую программу…». None — у папок его нет
pub fn открыть_с_помощью(пути: Vec<PathBuf>) -> Option<gtk::MenuItem> {
    let первый = пути.first()?.clone();
    if первый.is_dir() {
        return None;
    }
    let тип = crate::core::тип_файла(&первый);
    let подменю = gtk::Menu::new();
    let mut программы: Vec<gio::AppInfo> = Vec::new();
    if let Some(п) = gio::AppInfo::default_for_type(&тип, false) {
        программы.push(п);
    }
    for п in gio::AppInfo::all_for_type(&тип) {
        if п.should_show() || п.executable().to_string_lossy().contains("glower-") {
            if !программы.iter().any(|б| б.id() == п.id()) {
                программы.push(п);
            }
        }
    }
    for п in &программы {
        let значок = п.icon().unwrap_or_else(|| gio::ThemedIcon::new("application-x-executable").upcast());
        let пункт = пункт_программы(&п.display_name(), &значок);
        let (п, пути) = (п.clone(), пути.clone());
        пункт.connect_activate(move |_| {
            let файлы: Vec<gio::File> = пути.iter().map(gio::File::for_path).collect();
            let _ = п.launch(&файлы, None::<&gio::AppLaunchContext>);
        });
        подменю.append(&пункт);
    }
    for (команда, имя, значок, типы) in СВОИ {
        let подходит = типы.iter().any(|т| if т.ends_with('/') { тип.starts_with(т) } else { тип == *т });
        let уже = программы.iter().any(|п| п.executable().to_string_lossy().ends_with(команда));
        if !подходит || уже {
            continue;
        }
        let пункт = пункт_программы(&crate::i18n::tr(имя), &gio::ThemedIcon::new(значок).upcast());
        let пути = пути.clone();
        пункт.connect_activate(move |_| {
            let доводы: Vec<String> = пути.iter().map(|п| п.to_string_lossy().to_string()).collect();
            let доводы: Vec<&str> = доводы.iter().map(String::as_str).collect();
            crate::core::запусти_команду(команда, &доводы);
        });
        подменю.append(&пункт);
    }
    if !подменю.children().is_empty() {
        подменю.append(&gtk::SeparatorMenuItem::new());
    }
    let другая = crate::apps::пункт_меню("Выбрать другую программу…");
    {
        let (пути, тип) = (пути.clone(), тип.clone());
        // После закрытия меню: пока оно открыто, оно держит мышь
        другая.connect_activate(move |_| {
            let (пути, тип) = (пути.clone(), тип.clone());
            glib::idle_add_local_once(move || выбери_программу(&пути, &тип));
        });
    }
    подменю.append(&другая);
    let пункт = crate::apps::пункт_меню("Открыть с помощью");
    пункт.set_submenu(Some(&подменю));
    Some(пункт)
}

/// Список всех программ, как «Выбрать другое приложение» в Windows, с
/// галочкой «всегда открывать этой программой»
pub fn выбери_программу(пути: &[PathBuf], тип: &str) {
    let имя = пути.first().and_then(|п| п.file_name()).map(|и| и.to_string_lossy().to_string()).unwrap_or_default();
    let д = gtk::AppChooserDialog::for_content_type(None::<&gtk::Window>, gtk::DialogFlags::MODAL, тип);
    д.style_context().add_class("glower-app");
    д.set_title(&crate::i18n::tr("Открыть с помощью"));
    д.set_heading(&crate::i18n::tr(&format!("Чем открыть «{имя}»?")));
    if let Ok(в) = д.widget().downcast::<gtk::AppChooserWidget>() {
        в.set_show_default(true);
        в.set_show_recommended(true);
        в.set_show_other(true);
    }
    // Свои кнопки GTK здесь лишние: «Показать все» — всё и так показано,
    // а «Найти новые» ведёт в GNOME Software, которого в системе нет.
    // Вместо неё — наш Магазин
    fn кнопки(в: &gtk::Widget, итог: &mut Vec<gtk::Button>) {
        if let Some(к) = в.downcast_ref::<gtk::Button>() {
            if !в.is::<gtk::ToggleButton>() {
                итог.push(к.clone());
            }
        }
        if let Some(к) = в.downcast_ref::<gtk::Container>() {
            for р in к.children() {
                кнопки(&р, итог);
            }
        }
    }
    let mut лишние = Vec::new();
    кнопки(д.content_area().upcast_ref(), &mut лишние);
    for к in &лишние {
        к.set_no_show_all(true);
        к.hide();
    }
    let магазин = crate::apps::кнопка("🛍️ Найти программу в Магазине", "");
    {
        let тип = тип.to_string();
        let д2 = д.clone();
        магазин.connect_clicked(move |_| {
            д2.response(gtk::ResponseType::Cancel);
            crate::core::запусти_команду("glower-store", &[&format!("--find={}", тип.split('/').next_back().unwrap_or(""))]);
        });
    }
    магазин.set_margin_start(12);
    магазин.set_margin_end(12);
    д.content_area().add(&магазин);
    let всегда = gtk::CheckButton::with_label(&crate::i18n::tr("Всегда открывать этой программой"));
    всегда.set_margin_start(12);
    всегда.set_margin_bottom(8);
    д.content_area().add(&всегда);
    д.show_all();
    let ответ = д.run();
    let выбор = д.app_info();
    let навсегда = всегда.is_active();
    unsafe { д.destroy() };
    if ответ != gtk::ResponseType::Ok {
        return;
    }
    let Some(п) = выбор else { return };
    if навсегда {
        let _ = п.set_as_default_for_type(тип);
    }
    let файлы: Vec<gio::File> = пути.iter().map(gio::File::for_path).collect();
    let _ = п.launch(&файлы, None::<&gio::AppLaunchContext>);
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
    fn отмена_переноса_и_копии() {
        let корень = std::env::temp_dir().join(format!("glower-otmena-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&корень);
        let (а, б) = (корень.join("а"), корень.join("б"));
        std::fs::create_dir_all(&а).unwrap();
        std::fs::create_dir_all(&б).unwrap();
        std::fs::write(а.join("x.txt"), "1").unwrap();
        let и = вставь(&[а.join("x.txt")], &б, true);
        assert!(!а.join("x.txt").exists());
        assert!(обрати(&Дело::Перенос(и.пары)).is_empty());
        assert!(а.join("x.txt").exists() && !б.join("x.txt").exists());
        // Журнал переживает запись и чтение
        for д in [Дело::Перенос(vec![(а.clone(), б.clone())]), Дело::Копия(vec![а.clone()]), Дело::Корзина(vec![б.clone()]), Дело::Создание(vec![а.clone()])] {
            assert_eq!(из_json(&в_json(&д)), Some(д));
        }
        let _ = std::fs::remove_dir_all(&корень);
    }

    #[test]
    fn сжимает_и_распаковывает() {
        if std::process::Command::new("bsdtar").arg("--version").output().is_err() {
            return;
        }
        let корень = std::env::temp_dir().join(format!("glower-arh-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&корень);
        std::fs::create_dir_all(корень.join("папка")).unwrap();
        std::fs::write(корень.join("папка").join("а.txt"), "привет").unwrap();
        let цель = корень.join("x.zip");
        bsdtar(&["--format".as_ref(), "zip".as_ref(), "-cf".as_ref(), цель.as_os_str(), "-C".as_ref(), корень.as_os_str(), "папка".as_ref()]).unwrap();
        assert!(архив_ли(&цель));
        assert_eq!(без_расширения("Отчёт.tar.gz").as_deref(), Some("Отчёт"));
        assert_eq!(без_расширения("фото.png"), None);
        let в = корень.join("вывод");
        std::fs::create_dir(&в).unwrap();
        bsdtar(&["-xf".as_ref(), цель.as_os_str(), "-C".as_ref(), в.as_os_str()]).unwrap();
        assert_eq!(std::fs::read_to_string(в.join("папка").join("а.txt")).unwrap(), "привет");
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
