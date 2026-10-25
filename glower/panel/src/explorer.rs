//! Проводник — нативный, настоящим окном (glower-files).
//!
//! Страничный Проводник жил внутри браузерного движка и видел только папку
//! системы через агента. Этот ходит по настоящему диску сам: вкладки,
//! адресная строка, сетка и таблица, поиск, копирование и перенос,
//! перетаскивание, системная корзина, флешки и диски сбоку.

use crate::apps::{кнопка, подготовь};
use gtk::gdk_pixbuf::Pixbuf;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

const СТИЛЬ: &str = r#"
.files .sidebar { background-color: @glower_bg2; border-right: 1px solid alpha(@glower_fg, 0.07); }
.files .sidebar list { background-color: transparent; }
.files .sidebar row { padding: 6px 12px; border-radius: 8px; margin: 1px 6px; }
.files .sidebar row:selected { background-color: alpha(@glower_accent, 0.28); }
.files .sidebar .title { font-size: 11px; opacity: 0.5; padding: 10px 18px 4px; }
.files .crumbs button { padding: 4px 8px; background-color: transparent; }
.files .crumbs button:hover { background-color: alpha(@glower_fg, 0.09); }
.files .crumbs .sep { opacity: 0.4; }
.files .crumbs { background-color: alpha(@glower_fg, 0.05); border-radius: 9px; padding: 0 4px; }
.files iconview { background-color: @glower_bg2; }
.files iconview:selected { background-color: alpha(@glower_accent, 0.35); border-radius: 8px; }
.files treeview { background-color: @glower_bg2; }
.files treeview:selected { background-color: alpha(@glower_accent, 0.35); }
.files treeview header button { background-color: @glower_bg; border: none; border-bottom: 1px solid alpha(@glower_fg, 0.07); padding: 4px 8px; }
.files .trashbar { padding: 6px 10px; background-color: @glower_bg3; border-bottom: 1px solid alpha(@glower_fg, 0.07); }
.files entry.error { border-color: #f87171; }
"#;

/// Столбцы модели
const ЗНАЧОК: u32 = 0;
const МАЛЫЙ: u32 = 1;
const ИМЯ: u32 = 2;
const РАЗМЕР: u32 = 3;
const ДАТА: u32 = 4;
const ТИП: u32 = 5;
const ПУТЬ: u32 = 6;
const ПАПКА: u32 = 7;
const БАЙТ: u32 = 8;
const ВРЕМЯ: u32 = 9;

#[derive(Clone, PartialEq, Debug)]
enum Место {
    Папка(PathBuf),
    Корзина,
    Поиск(PathBuf, String),
}

#[derive(Clone, Debug)]
struct Запись {
    имя: String,
    путь: PathBuf,
    папка: bool,
    байт: u64,
    время: i64,
    тип: String,
    откуда: Option<String>,
}

struct Вкладка {
    место: RefCell<Место>,
    назад: RefCell<Vec<Место>>,
    вперёд: RefCell<Vec<Место>>,
    модель: gtk::ListStore,
    сетка: gtk::IconView,
    таблица: gtk::TreeView,
    стопка: gtk::Stack,
    ярлык: gtk::Label,
    наблюдатель: RefCell<Option<gio::FileMonitor>>,
    поколение: Cell<u64>,
    выделить: RefCell<Option<String>>,
}

struct Проводник {
    окно: gtk::Window,
    вкладки: gtk::Notebook,
    список: RefCell<Vec<Rc<Вкладка>>>,
    назад: gtk::Button,
    вперёд: gtk::Button,
    вверх: gtk::Button,
    крошки: gtk::Box,
    адрес: gtk::Entry,
    адресная: gtk::Stack,
    поиск: gtk::Entry,
    строка: gtk::Label,
    корзина: gtk::Revealer,
    вид: gtk::Button,
    новая: gtk::Button,
    места: gtk::ListBox,
    цели: RefCell<Vec<Место>>,
    таблицей: Cell<bool>,
    скрытые: Cell<bool>,
    буфер: RefCell<Option<(Vec<PathBuf>, bool)>>,
    корень: PathBuf,
    значки: RefCell<HashMap<(String, i32), Option<Pixbuf>>>,
}

type П = Rc<Проводник>;

/* ---------- мелочи ---------- */

fn размер_текстом(б: u64) -> String {
    const Е: [&str; 5] = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
    let mut ч = б as f64;
    let mut i = 0;
    while ч >= 1024.0 && i < Е.len() - 1 {
        ч /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{б} Б")
    } else if ч < 10.0 {
        format!("{:.1} {}", ч, Е[i]).replace('.', ",")
    } else {
        format!("{:.0} {}", ч, Е[i])
    }
}

fn дата_текстом(с: i64) -> String {
    glib::DateTime::from_unix_local(с)
        .and_then(|д| д.format("%d.%m.%Y %H:%M"))
        .map(|т| т.to_string())
        .unwrap_or_default()
}

/// Свободное имя в папке: «имя», а занято — «имя (2)», «имя (3)»…
fn свободное_имя(папка: &Path, имя: &str) -> PathBuf {
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

fn скопируй(откуда: &Path, куда: &Path) -> std::io::Result<()> {
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

fn перенеси(откуда: &Path, куда: &Path) -> std::io::Result<()> {
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

/// Вставить пути в папку: копией или переносом. Что не вышло — списком.
fn вставь(пути: &[PathBuf], папка: &Path, вырезать: bool) -> Vec<String> {
    let mut беды = Vec::new();
    for п in пути {
        let имя = п.file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_default();
        if папка.starts_with(п) {
            беды.push(format!("«{имя}»: папку нельзя положить в саму себя"));
            continue;
        }
        if вырезать && п.parent() == Some(папка) {
            continue;
        }
        let куда = свободное_имя(папка, &имя);
        let итог = if вырезать { перенеси(п, &куда) } else { скопируй(п, &куда) };
        if let Err(е) = итог {
            беды.push(format!("«{имя}»: {е}"));
        }
    }
    беды
}

fn папка_корзины() -> PathBuf {
    glib::user_data_dir().join("Trash")
}

/// «%D0%94…» → «Д…»: так корзина записывает исходный путь
fn раскодируй(с: &str) -> String {
    let б = с.as_bytes();
    let mut итог = Vec::with_capacity(б.len());
    let mut i = 0;
    while i < б.len() {
        if б[i] == b'%' && i + 2 < б.len() {
            if let Ok(ч) = u8::from_str_radix(&с[i + 1..i + 3], 16) {
                итог.push(ч);
                i += 3;
                continue;
            }
        }
        итог.push(б[i]);
        i += 1;
    }
    String::from_utf8_lossy(&итог).to_string()
}

fn откуда_в_корзине(имя: &str) -> Option<String> {
    let т = std::fs::read_to_string(папка_корзины().join("info").join(format!("{имя}.trashinfo"))).ok()?;
    т.lines().find_map(|с| с.strip_prefix("Path=")).map(раскодируй)
}

fn запись(путь: &Path, скрытые: bool) -> Option<Запись> {
    let имя = путь.file_name()?.to_string_lossy().to_string();
    if !скрытые && имя.starts_with('.') {
        return None;
    }
    let м = std::fs::metadata(путь).or_else(|_| путь.symlink_metadata()).ok()?;
    let папка = м.is_dir();
    let тип = if папка {
        "inode/directory".to_string()
    } else {
        угадай_тип(&имя)
    };
    use std::os::unix::fs::MetadataExt;
    Some(Запись { имя, путь: путь.to_path_buf(), папка, байт: if папка { 0 } else { м.len() }, время: м.mtime(), тип, откуда: None })
}

fn прочти_папку(папка: &Path, скрытые: bool) -> std::io::Result<Vec<Запись>> {
    Ok(std::fs::read_dir(папка)?.filter_map(|з| з.ok()).filter_map(|з| запись(&з.path(), скрытые)).collect())
}

fn прочти_корзину() -> Vec<Запись> {
    let файлы = папка_корзины().join("files");
    let Ok(чтение) = std::fs::read_dir(&файлы) else { return Vec::new() };
    чтение
        .filter_map(|з| з.ok())
        .filter_map(|з| {
            let mut з2 = запись(&з.path(), true)?;
            з2.откуда = откуда_в_корзине(&з2.имя);
            Some(з2)
        })
        .collect()
}

fn ищи(папка: &Path, что: &str, скрытые: bool, итог: &mut Vec<Запись>, глубина: usize) {
    if итог.len() >= 1000 || глубина > 24 {
        return;
    }
    let Ok(чтение) = std::fs::read_dir(папка) else { return };
    for з in чтение.filter_map(|з| з.ok()) {
        let путь = з.path();
        let Some(з2) = запись(&путь, скрытые) else { continue };
        if з2.имя.to_lowercase().contains(что) {
            итог.push(з2.clone());
        }
        // По ссылкам на папки не ходим — так недолго и зациклиться
        if з2.папка && !з.file_type().map(|т| т.is_symlink()).unwrap_or(true) {
            ищи(&путь, что, скрытые, итог, глубина + 1);
        }
    }
}

/// Тип файла по имени. Обёртка gtk-rs передаёт пустые данные не пустым
/// указателем — и любой файл выходит «нулевой длины»; зовём GLib сами
fn угадай_тип(имя: &str) -> String {
    use glib::translate::{from_glib_full, ToGlibPtr};
    unsafe {
        let mut неточно = 0;
        let т: glib::GString = from_glib_full(gio::ffi::g_content_type_guess(имя.to_glib_none().0, std::ptr::null(), 0, &mut неточно));
        т.to_string()
    }
}

/* ---------- значки ---------- */

fn запасной_значок(папка: bool, размер: i32) -> Option<Pixbuf> {
    let п = gtk::cairo::ImageSurface::create(gtk::cairo::Format::ARgb32, размер, размер).ok()?;
    {
        let к = gtk::cairo::Context::new(&п).ok()?;
        let р = размер as f64;
        if папка {
            к.set_source_rgb(0.98, 0.75, 0.25);
            к.rectangle(р * 0.08, р * 0.22, р * 0.38, р * 0.14);
            к.rectangle(р * 0.08, р * 0.3, р * 0.84, р * 0.56);
        } else {
            к.set_source_rgb(0.85, 0.88, 0.95);
            к.rectangle(р * 0.2, р * 0.08, р * 0.6, р * 0.84);
        }
        к.fill().ok()?;
    }
    gtk::gdk::pixbuf_get_from_surface(&п, 0, 0, размер, размер)
}

fn особая_папка(пр: &Проводник, путь: &Path) -> Option<&'static str> {
    if путь.parent()? != пр.корень {
        return None;
    }
    Some(match путь.file_name()?.to_str()? {
        "Рабочий стол" => "user-desktop",
        "Документы" => "folder-documents",
        "Изображения" => "folder-pictures",
        "Музыка" => "folder-music",
        "Загрузки" => "folder-download",
        "Видео" => "folder-videos",
        _ => return None,
    })
}

fn значок(пр: &Проводник, з: &Запись, размер: i32) -> Option<Pixbuf> {
    let ключ = if з.папка {
        особая_папка(пр, &з.путь).unwrap_or("folder").to_string()
    } else {
        з.тип.clone()
    };
    if let Some(п) = пр.значки.borrow().get(&(ключ.clone(), размер)) {
        return п.clone();
    }
    let тема = gtk::IconTheme::default();
    let найди = |иконка: &gio::Icon| -> Option<Pixbuf> {
        тема.as_ref()?.lookup_by_gicon(иконка, размер, gtk::IconLookupFlags::FORCE_SIZE)?.load_icon().ok()
    };
    let п = if з.папка {
        найди(&gio::ThemedIcon::from_names(&[&ключ, "folder"]).upcast())
    } else {
        найди(&gio::content_type_get_icon(&з.тип)).or_else(|| найди(&gio::ThemedIcon::new("text-x-generic").upcast()))
    }
    .or_else(|| запасной_значок(з.папка, размер));
    пр.значки.borrow_mut().insert((ключ, размер), п.clone());
    п
}

fn описание_типа(з: &Запись) -> String {
    if let Some(о) = &з.откуда {
        return о.clone();
    }
    if з.папка {
        return "Папка".into();
    }
    let о = gio::content_type_get_description(&з.тип).to_string();
    if о.is_empty() { з.тип.clone() } else { о }
}

/* ---------- вкладка ---------- */

fn текущая(п: &П) -> Option<Rc<Вкладка>> {
    let н = п.вкладки.current_page()? as usize;
    п.список.borrow().get(н).cloned()
}

fn запись_из(модель: &gtk::ListStore, итер: &gtk::TreeIter) -> Запись {
    Запись {
        имя: модель.value(итер, ИМЯ as i32).get::<String>().unwrap_or_default(),
        путь: PathBuf::from(модель.value(итер, ПУТЬ as i32).get::<String>().unwrap_or_default()),
        папка: модель.value(итер, ПАПКА as i32).get::<bool>().unwrap_or_default(),
        байт: модель.value(итер, БАЙТ as i32).get::<u64>().unwrap_or_default(),
        время: модель.value(итер, ВРЕМЯ as i32).get::<i64>().unwrap_or_default(),
        тип: String::new(),
        откуда: None,
    }
}

fn выбранные_пути(в: &Вкладка, таблицей: bool) -> Vec<gtk::TreePath> {
    if таблицей {
        в.таблица.selection().selected_rows().0
    } else {
        в.сетка.selected_items()
    }
}

fn выбранные(п: &П, в: &Вкладка) -> Vec<Запись> {
    выбранные_пути(в, п.таблицей.get())
        .iter()
        .filter_map(|путь| в.модель.iter(путь))
        .map(|и| запись_из(&в.модель, &и))
        .collect()
}

fn выбери_одну(п: &П, в: &Вкладка, путь: &gtk::TreePath) {
    if п.таблицей.get() {
        в.таблица.selection().unselect_all();
        в.таблица.selection().select_path(путь);
        в.таблица.set_cursor(путь, None::<&gtk::TreeViewColumn>, false);
    } else {
        в.сетка.unselect_all();
        в.сетка.select_path(путь);
        в.сетка.set_cursor(путь, None::<&gtk::CellRenderer>, false);
    }
}

fn путь_под(п: &П, в: &Вкладка, x: f64, y: f64) -> Option<gtk::TreePath> {
    if п.таблицей.get() {
        в.таблица.path_at_pos(x as i32, y as i32).and_then(|(путь, ..)| путь)
    } else {
        в.сетка.path_at_pos(x as i32, y as i32)
    }
}

fn выбрано_ли(п: &П, в: &Вкладка, путь: &gtk::TreePath) -> bool {
    if п.таблицей.get() {
        в.таблица.selection().path_is_selected(путь)
    } else {
        в.сетка.path_is_selected(путь)
    }
}

fn имя_места(п: &Проводник, м: &Место) -> String {
    match м {
        Место::Корзина => "Корзина".into(),
        Место::Поиск(_, что) => format!("Поиск: {что}"),
        Место::Папка(папка) if *папка == п.корень => "Этот компьютер".into(),
        Место::Папка(папка) => папка.file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_else(|| "/".into()),
    }
}

fn добавь_строку(п: &П, в: &Вкладка, з: &Запись) {
    let большой = значок(п, з, 48);
    let малый = значок(п, з, 16);
    в.модель.insert_with_values(
        None,
        &[
            (ЗНАЧОК, &большой),
            (МАЛЫЙ, &малый),
            (ИМЯ, &з.имя),
            (РАЗМЕР, &if з.папка { String::new() } else { размер_текстом(з.байт) }),
            (ДАТА, &дата_текстом(з.время)),
            (ТИП, &описание_типа(з)),
            (ПУТЬ, &з.путь.to_string_lossy().to_string()),
            (ПАПКА, &з.папка),
            (БАЙТ, &з.байт),
            (ВРЕМЯ, &з.время),
        ],
    );
}

/// Картинки показываем самими картинками — по одной за проход, чтобы
/// большая папка с фотографиями не подвешивала окно
fn миниатюры(в: &Rc<Вкладка>) {
    let поколение = в.поколение.get();
    let в = в.clone();
    let номер = Cell::new(0);
    glib::idle_add_local(move || {
        if в.поколение.get() != поколение {
            return glib::ControlFlow::Break;
        }
        let Some(и) = в.модель.iter_nth_child(None, номер.get()) else { return glib::ControlFlow::Break };
        номер.set(номер.get() + 1);
        let путь = в.модель.value(&и, ПУТЬ as i32).get::<String>().unwrap_or_default();
        let байт = в.модель.value(&и, БАЙТ as i32).get::<u64>().unwrap_or_default();
        let тип = угадай_тип(&путь);
        if тип.starts_with("image/") && байт < 30 << 20 && !в.модель.value(&и, ПАПКА as i32).get::<bool>().unwrap_or_default() {
            if let Ok(м) = Pixbuf::from_file_at_scale(&путь, 64, 64, true) {
                в.модель.set_value(&и, ЗНАЧОК, &м.to_value());
            }
        }
        glib::ControlFlow::Continue
    });
}

fn следи(п: &П, в: &Rc<Вкладка>, папка: Option<&Path>) {
    *в.наблюдатель.borrow_mut() = None;
    let Some(папка) = папка else { return };
    let Ok(н) = gio::File::for_path(папка).monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) else { return };
    let (п2, в2) = (Rc::downgrade(п), Rc::downgrade(в));
    let ждём = Rc::new(Cell::new(false));
    н.connect_changed(move |_, _, _, _| {
        if ждём.replace(true) {
            return;
        }
        let (п2, в2, ждём) = (п2.clone(), в2.clone(), ждём.clone());
        glib::timeout_add_local_once(std::time::Duration::from_millis(250), move || {
            ждём.set(false);
            if let (Some(п), Some(в)) = (п2.upgrade(), в2.upgrade()) {
                загрузи(&п, &в, true);
            }
        });
    });
    *в.наблюдатель.borrow_mut() = Some(н);
}

/// Наполнить вкладку тем, что лежит в её месте. `тихо` — перечитываем то же
/// место: выделение и прокрутку не сбрасываем
fn загрузи(п: &П, в: &Rc<Вкладка>, тихо: bool) {
    в.поколение.set(в.поколение.get() + 1);
    let было: Vec<String> = if тихо { выбранные(п, в).into_iter().map(|з| з.имя).collect() } else { Vec::new() };
    let место = в.место.borrow().clone();
    в.модель.clear();
    match &место {
        Место::Папка(папка) => {
            match прочти_папку(папка, п.скрытые.get()) {
                Ok(записи) => записи.iter().for_each(|з| добавь_строку(п, в, з)),
                Err(е) => сообщи(п, "Папку не открыть", &format!("{}: {е}", папка.display())),
            }
            следи(п, в, Some(папка));
        }
        Место::Корзина => {
            прочти_корзину().iter().for_each(|з| добавь_строку(п, в, з));
            let файлы = папка_корзины().join("files");
            let _ = std::fs::create_dir_all(&файлы);
            следи(п, в, Some(&файлы));
        }
        Место::Поиск(папка, что) => {
            следи(п, в, None);
            let (папка, что, скрытые) = (папка.clone(), что.to_lowercase(), п.скрытые.get());
            let (куда, откуда) = async_channel::bounded(1);
            std::thread::spawn(move || {
                let mut итог = Vec::new();
                ищи(&папка, &что, скрытые, &mut итог, 0);
                let _ = куда.send_blocking(итог);
            });
            let (п2, в2, поколение) = (п.clone(), в.clone(), в.поколение.get());
            п.строка.set_text("Ищу…");
            glib::MainContext::default().spawn_local(async move {
                if let Ok(итог) = откуда.recv().await {
                    if в2.поколение.get() != поколение {
                        return;
                    }
                    итог.iter().for_each(|з| добавь_строку(&п2, &в2, з));
                    миниатюры(&в2);
                    обнови_строку(&п2);
                }
            });
        }
    }
    миниатюры(в);
    // Выделить то, что просили, или то, что было выделено
    let нужно: Vec<String> = в.выделить.borrow_mut().take().map(|и| vec![и]).unwrap_or(было);
    if !нужно.is_empty() {
        for н in 0..в.модель.iter_n_children(None) {
            if let Some(и) = в.модель.iter_nth_child(None, н) {
                if нужно.contains(&в.модель.value(&и, ИМЯ as i32).get::<String>().unwrap_or_default()) {
                    let Some(путь) = в.модель.path(&и) else { continue };
                    if п.таблицей.get() {
                        в.таблица.selection().select_path(&путь);
                        в.таблица.scroll_to_cell(Some(&путь), None::<&gtk::TreeViewColumn>, false, 0.0, 0.0);
                    } else {
                        в.сетка.select_path(&путь);
                        в.сетка.scroll_to_path(&путь, false, 0.0, 0.0);
                    }
                }
            }
        }
    }
    в.ярлык.set_text(&имя_места(п, &место));
    обнови_всё(п);
}

fn иди(п: &П, в: &Rc<Вкладка>, куда: Место) {
    if *в.место.borrow() != куда {
        let было = в.место.replace(куда);
        в.назад.borrow_mut().push(было);
        в.вперёд.borrow_mut().clear();
    }
    загрузи(п, в, false);
}

fn назад(п: &П) {
    let Some(в) = текущая(п) else { return };
    let Some(м) = в.назад.borrow_mut().pop() else { return };
    let было = в.место.replace(м);
    в.вперёд.borrow_mut().push(было);
    загрузи(п, &в, false);
}

fn вперёд(п: &П) {
    let Some(в) = текущая(п) else { return };
    let Some(м) = в.вперёд.borrow_mut().pop() else { return };
    let было = в.место.replace(м);
    в.назад.borrow_mut().push(было);
    загрузи(п, &в, false);
}

fn вверх(п: &П) {
    let Some(в) = текущая(п) else { return };
    let м = в.место.borrow().clone();
    let куда = match &м {
        Место::Папка(папка) => папка.parent().map(|р| (р.to_path_buf(), папка.file_name().map(|и| и.to_string_lossy().to_string()))),
        Место::Поиск(папка, _) => Some((папка.clone(), None)),
        Место::Корзина => Some((п.корень.clone(), None)),
    };
    if let Some((папка, имя)) = куда {
        *в.выделить.borrow_mut() = имя;
        иди(п, &в, Место::Папка(папка));
    }
}

/* ---------- вид окна ---------- */

fn обнови_строку(п: &П) {
    let Some(в) = текущая(п) else { return };
    let всего = в.модель.iter_n_children(None);
    let выбрано = выбранные(п, &в);
    let текст = if выбрано.is_empty() {
        format!("Объектов: {всего}")
    } else {
        let байт: u64 = выбрано.iter().filter(|з| !з.папка).map(|з| з.байт).sum();
        let хвост = if байт > 0 { format!(" · {}", размер_текстом(байт)) } else { String::new() };
        format!("Объектов: {всего} · выделено: {}{хвост}", выбрано.len())
    };
    п.строка.set_text(&текст);
}

fn обнови_крошки(п: &П) {
    for р in п.крошки.children() {
        п.крошки.remove(&р);
    }
    let Some(в) = текущая(п) else { return };
    let м = в.место.borrow().clone();
    let кнопка_к = |текст: &str, куда: Option<Место>| {
        let б = gtk::Button::new();
        crate::apps::знак_кнопки(&б, текст);
        б.set_relief(gtk::ReliefStyle::None);
        if let Some(куда) = куда {
            let (п2, в2) = (п.clone(), в.clone());
            б.connect_clicked(move |_| иди(&п2, &в2, куда.clone()));
        }
        п.крошки.add(&б);
    };
    let черта = || {
        let ч = gtk::Label::new(Some("›"));
        ч.style_context().add_class("sep");
        п.крошки.add(&ч);
    };
    match &м {
        Место::Корзина => кнопка_к("🗑 Корзина", None),
        Место::Поиск(папка, что) => {
            кнопка_к(&format!("🔎 «{что}» в «{}»", имя_места(п, &Место::Папка(папка.clone()))), Some(Место::Папка(папка.clone())));
        }
        Место::Папка(папка) => {
            let (начало, хвост) = match папка.strip_prefix(&п.корень) {
                Ok(хвост) => (п.корень.clone(), хвост.to_path_buf()),
                Err(_) => (PathBuf::from("/"), папка.strip_prefix("/").map(|х| х.to_path_buf()).unwrap_or_default()),
            };
            кнопка_к(if начало == п.корень { "💻 Этот компьютер" } else { "🖴 /" }, Some(Место::Папка(начало.clone())));
            let mut путь = начало;
            for часть in хвост.components() {
                путь.push(часть);
                черта();
                кнопка_к(&часть.as_os_str().to_string_lossy(), Some(Место::Папка(путь.clone())));
            }
        }
    }
    п.крошки.show_all();
}

fn обнови_всё(п: &П) {
    let Some(в) = текущая(п) else { return };
    let м = в.место.borrow().clone();
    п.назад.set_sensitive(!в.назад.borrow().is_empty());
    п.вперёд.set_sensitive(!в.вперёд.borrow().is_empty());
    п.вверх.set_sensitive(!matches!(&м, Место::Папка(п2) if п2.parent().is_none()));
    п.окно.set_title(&format!("{} — Проводник", имя_места(п, &м)));
    п.корзина.set_reveal_child(м == Место::Корзина);
    п.новая.set_sensitive(matches!(м, Место::Папка(_)));
    в.стопка.set_visible_child_name(if п.таблицей.get() { "таблица" } else { "сетка" });
    crate::apps::знак_кнопки(&п.вид, if п.таблицей.get() { "▦" } else { "☰" });
    // Сбоку — то место, где мы
    let цели = п.цели.borrow();
    match цели.iter().position(|ц| *ц == м) {
        Some(н) => п.места.select_row(п.места.row_at_index(н as i32).as_ref()),
        None => п.места.unselect_all(),
    }
    drop(цели);
    обнови_крошки(п);
    обнови_строку(п);
}

fn сообщи(п: &П, заголовок: &str, текст: &str) {
    let д = gtk::MessageDialog::new(
        Some(&п.окно),
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        gtk::MessageType::Warning,
        gtk::ButtonsType::Close,
        заголовок,
    );
    д.set_secondary_text(Some(текст));
    д.run();
    unsafe { д.destroy() };
}

fn спроси(п: &П, заголовок: &str, текст: &str, да: &str) -> bool {
    let д = gtk::MessageDialog::new(
        Some(&п.окно),
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        gtk::MessageType::Question,
        gtk::ButtonsType::None,
        заголовок,
    );
    if !текст.is_empty() {
        д.set_secondary_text(Some(текст));
    }
    д.add_button("Отмена", gtk::ResponseType::Cancel);
    д.add_button(да, gtk::ResponseType::Accept);
    д.set_default_response(gtk::ResponseType::Accept);
    let ответ = д.run();
    unsafe { д.destroy() };
    ответ == gtk::ResponseType::Accept
}

/// Спросить имя. Выделено оно без расширения — как во всех проводниках
fn спроси_имя(п: &П, заголовок: &str, было: &str) -> Option<String> {
    let д = gtk::Dialog::with_buttons(
        Some(заголовок),
        Some(&п.окно),
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[("Отмена", gtk::ResponseType::Cancel), ("Готово", gtk::ResponseType::Accept)],
    );
    д.style_context().add_class("glower-app");
    д.set_default_response(gtk::ResponseType::Accept);
    let поле = gtk::Entry::new();
    поле.set_text(было);
    поле.set_activates_default(true);
    поле.set_width_chars(36);
    поле.set_margin(14);
    д.content_area().add(&поле);
    д.show_all();
    let конец = match было.rfind('.') {
        Some(т) if т > 0 => т as i32,
        _ => -1,
    };
    поле.grab_focus();
    // Поле само выделяет всё, получив фокус, — выделяем своё уже после
    let поле2 = поле.clone();
    glib::idle_add_local_once(move || поле2.select_region(0, конец));
    let ответ = д.run();
    let имя = поле.text().trim().to_string();
    unsafe { д.destroy() };
    if ответ != gtk::ResponseType::Accept || имя.is_empty() {
        return None;
    }
    if имя.contains('/') || имя == "." || имя == ".." {
        сообщи(п, "Такое имя нельзя", "В имени не может быть «/».");
        return None;
    }
    Some(имя)
}

/* ---------- дела ---------- */

fn текущая_папка(п: &П) -> Option<PathBuf> {
    match &*текущая(п)?.место.borrow() {
        Место::Папка(папка) => Some(папка.clone()),
        _ => None,
    }
}

fn открой(п: &П, з: &Запись, в_новой: bool) {
    let Some(в) = текущая(п) else { return };
    if *в.место.borrow() == Место::Корзина {
        return свойства(п, std::slice::from_ref(з));
    }
    if з.папка {
        if в_новой {
            новая_вкладка(п, Место::Папка(з.путь.clone()));
        } else {
            иди(п, &в, Место::Папка(з.путь.clone()));
        }
        return;
    }
    let тип = угадай_тип(&з.путь.to_string_lossy());
    // Текст — своим Блокнотом, картинки — своими Фото, остальное — тем,
    // что назначено в системе
    if gio::content_type_is_a(&тип, "text/plain") && std::process::Command::new("glower-notepad").arg(&з.путь).spawn().is_ok() {
        return;
    }
    if тип.starts_with("image/") && std::process::Command::new("glower-photos").arg(&з.путь).spawn().is_ok() {
        return;
    }
    let uri = gio::File::for_path(&з.путь).uri();
    if let Err(е) = gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>) {
        сообщи(п, &format!("Нечем открыть «{}»", з.имя), &е.to_string());
    }
}

fn в_буфер(п: &П, вырезать: bool) {
    let Some(в) = текущая(п) else { return };
    if *в.место.borrow() == Место::Корзина {
        return;
    }
    let пути: Vec<PathBuf> = выбранные(п, &в).into_iter().map(|з| з.путь).collect();
    if пути.is_empty() {
        return;
    }
    // Пути — и в общий буфер обмена: вставятся в терминал или письмо
    let текст = пути.iter().map(|п| п.to_string_lossy().to_string()).collect::<Vec<_>>().join("\n");
    gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD).set_text(&текст);
    *п.буфер.borrow_mut() = Some((пути, вырезать));
}

fn вставь_сюда(п: &П, папка: PathBuf) {
    let Some((пути, вырезать)) = п.буфер.borrow().clone() else { return };
    if вырезать {
        *п.буфер.borrow_mut() = None;
    }
    let (куда, откуда) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = куда.send_blocking(вставь(&пути, &папка, вырезать));
    });
    let п2 = п.clone();
    glib::MainContext::default().spawn_local(async move {
        if let Ok(беды) = откуда.recv().await {
            if !беды.is_empty() {
                сообщи(&п2, "Не всё удалось", &беды.join("\n"));
            }
            if let Some(в) = текущая(&п2) {
                загрузи(&п2, &в, true);
            }
        }
    });
}

fn удали(п: &П, навсегда: bool) {
    let Some(в) = текущая(п) else { return };
    let записи = выбранные(п, &в);
    if записи.is_empty() {
        return;
    }
    let в_корзине = *в.место.borrow() == Место::Корзина;
    let о_чём = if записи.len() == 1 { format!("«{}»", записи[0].имя) } else { format!("{} объектов", записи.len()) };
    if навсегда || в_корзине {
        if !спроси(п, &format!("Удалить {о_чём} навсегда?"), "Вернуть не получится.", "Удалить") {
            return;
        }
    }
    let mut беды = Vec::new();
    for з in &записи {
        let итог = if навсегда || в_корзине {
            let р = if з.папка && !з.путь.symlink_metadata().map(|м| м.file_type().is_symlink()).unwrap_or(false) {
                std::fs::remove_dir_all(&з.путь)
            } else {
                std::fs::remove_file(&з.путь)
            };
            if в_корзине {
                let _ = std::fs::remove_file(папка_корзины().join("info").join(format!("{}.trashinfo", з.имя)));
            }
            р.map_err(|е| е.to_string())
        } else {
            gio::File::for_path(&з.путь).trash(gio::Cancellable::NONE).map_err(|е| е.to_string())
        };
        if let Err(е) = итог {
            беды.push(format!("«{}»: {е}", з.имя));
        }
    }
    if !беды.is_empty() {
        сообщи(п, "Не всё удалось удалить", &беды.join("\n"));
    }
    загрузи(п, &в, true);
}

fn восстанови(п: &П, все: bool) {
    let Some(в) = текущая(п) else { return };
    let записи = if все { прочти_корзину() } else { выбранные(п, &в) };
    let mut беды = Vec::new();
    for з in &записи {
        let Some(откуда) = откуда_в_корзине(&з.имя) else {
            беды.push(format!("«{}»: неизвестно, откуда он", з.имя));
            continue;
        };
        let откуда = PathBuf::from(откуда);
        let папка = откуда.parent().map(Path::to_path_buf).unwrap_or_else(|| п.корень.clone());
        let _ = std::fs::create_dir_all(&папка);
        let имя = откуда.file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_else(|| з.имя.clone());
        match перенеси(&з.путь, &свободное_имя(&папка, &имя)) {
            Ok(()) => {
                let _ = std::fs::remove_file(папка_корзины().join("info").join(format!("{}.trashinfo", з.имя)));
            }
            Err(е) => беды.push(format!("«{}»: {е}", з.имя)),
        }
    }
    if !беды.is_empty() {
        сообщи(п, "Не всё удалось вернуть", &беды.join("\n"));
    }
    загрузи(п, &в, true);
}

fn очисти_корзину(п: &П) {
    if прочти_корзину().is_empty() || !спроси(п, "Очистить корзину?", "Всё в ней удалится навсегда.", "Очистить") {
        return;
    }
    for часть in ["files", "info"] {
        if let Ok(чтение) = std::fs::read_dir(папка_корзины().join(часть)) {
            for з in чтение.filter_map(|з| з.ok()) {
                let п2 = з.path();
                let _ = if з.file_type().map(|т| т.is_dir()).unwrap_or(false) { std::fs::remove_dir_all(&п2) } else { std::fs::remove_file(&п2) };
            }
        }
    }
    if let Some(в) = текущая(п) {
        загрузи(п, &в, true);
    }
}

fn переименуй(п: &П) {
    let Some(в) = текущая(п) else { return };
    if *в.место.borrow() == Место::Корзина {
        return;
    }
    let записи = выбранные(п, &в);
    let [з] = записи.as_slice() else { return };
    let Some(имя) = спроси_имя(п, "Переименовать", &з.имя) else { return };
    if имя == з.имя {
        return;
    }
    let куда = з.путь.with_file_name(&имя);
    if куда.exists() {
        return сообщи(п, "Имя уже занято", &format!("В этой папке уже есть «{имя}»."));
    }
    match std::fs::rename(&з.путь, &куда) {
        Ok(()) => {
            *в.выделить.borrow_mut() = Some(имя);
            загрузи(п, &в, false);
        }
        Err(е) => сообщи(п, "Не удалось переименовать", &е.to_string()),
    }
}

fn создай(п: &П, папку: bool) {
    let (Some(в), Some(папка)) = (текущая(п), текущая_папка(п)) else { return };
    let черновик = if папку { "Новая папка" } else { "Новый документ.txt" };
    let имя = свободное_имя(&папка, черновик).file_name().map(|и| и.to_string_lossy().to_string()).unwrap_or_default();
    let Some(имя) = спроси_имя(п, if папку { "Новая папка" } else { "Новый файл" }, &имя) else { return };
    let путь = папка.join(&имя);
    if путь.exists() {
        return сообщи(п, "Имя уже занято", &format!("В этой папке уже есть «{имя}»."));
    }
    let итог = if папку { std::fs::create_dir(&путь) } else { std::fs::write(&путь, b"") };
    match итог {
        Ok(()) => {
            *в.выделить.borrow_mut() = Some(имя);
            загрузи(п, &в, false);
        }
        Err(е) => сообщи(п, "Не удалось создать", &е.to_string()),
    }
}

fn размер_папки(путь: &Path, глубина: usize) -> (u64, u64) {
    let mut итог = (0, 0);
    if глубина > 32 {
        return итог;
    }
    if let Ok(чтение) = std::fs::read_dir(путь) {
        for з in чтение.filter_map(|з| з.ok()) {
            let Ok(м) = з.path().symlink_metadata() else { continue };
            итог.1 += 1;
            if м.is_dir() {
                let (б, ш) = размер_папки(&з.path(), глубина + 1);
                итог.0 += б;
                итог.1 += ш;
            } else {
                итог.0 += м.len();
            }
        }
    }
    итог
}

fn свойства(п: &П, записи: &[Запись]) {
    let (заголовок, строки): (String, Vec<(String, String)>) = match записи {
        [] => {
            let Some(папка) = текущая_папка(п) else { return };
            let Some(з) = запись(&папка, true) else { return };
            return свойства(п, &[з]);
        }
        [з] => {
            let полная = запись(&з.путь, true).unwrap_or_else(|| з.clone());
            let mut с = vec![
                ("Тип".into(), описание_типа(&полная)),
                ("Где".into(), з.путь.parent().map(|р| р.display().to_string()).unwrap_or_default()),
            ];
            if let Some(о) = откуда_в_корзине(&з.имя).filter(|_| з.путь.starts_with(папка_корзины())) {
                с.push(("Откуда".into(), о));
            }
            if з.папка {
                let (б, ш) = размер_папки(&з.путь, 0);
                с.push(("Размер".into(), format!("{} · объектов: {ш}", размер_текстом(б))));
            } else {
                с.push(("Размер".into(), format!("{} ({} байт)", размер_текстом(полная.байт), полная.байт)));
            }
            с.push(("Изменён".into(), дата_текстом(полная.время)));
            (з.имя.clone(), с)
        }
        много => {
            let mut б = 0;
            for з in много {
                б += if з.папка { размер_папки(&з.путь, 0).0 } else { з.байт };
            }
            (format!("{} объектов", много.len()), vec![("Размер".into(), размер_текстом(б))])
        }
    };
    let д = gtk::Dialog::with_buttons(
        Some(&format!("Свойства: {заголовок}")),
        Some(&п.окно),
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[("Закрыть", gtk::ResponseType::Close)],
    );
    let сетка = gtk::Grid::new();
    сетка.set_row_spacing(8);
    сетка.set_column_spacing(16);
    сетка.set_margin(18);
    let имя = gtk::Label::new(Some(&заголовок));
    имя.set_xalign(0.0);
    имя.set_selectable(true);
    сетка.attach(&имя, 0, 0, 2, 1);
    for (н, (к, з)) in строки.iter().enumerate() {
        let л = gtk::Label::new(Some(к));
        л.set_xalign(0.0);
        л.style_context().add_class("muted");
        let п2 = gtk::Label::new(Some(з));
        п2.set_xalign(0.0);
        п2.set_selectable(true);
        п2.set_line_wrap(true);
        п2.set_max_width_chars(48);
        сетка.attach(&л, 0, н as i32 + 1, 1, 1);
        сетка.attach(&п2, 1, н as i32 + 1, 1, 1);
    }
    д.content_area().add(&сетка);
    д.show_all();
    д.run();
    unsafe { д.destroy() };
}

/* ---------- меню ---------- */

fn пункт(меню: &gtk::Menu, текст: &str, можно: bool, дело: impl Fn() + 'static) {
    let п = crate::apps::пункт_меню(&текст);
    п.set_sensitive(можно);
    п.connect_activate(move |_| дело());
    меню.append(&п);
}

fn черта(меню: &gtk::Menu) {
    меню.append(&gtk::SeparatorMenuItem::new());
}

fn меню_записей(п: &П, событие: &gtk::gdk::EventButton) {
    let Some(в) = текущая(п) else { return };
    let записи = выбранные(п, &в);
    let меню = gtk::Menu::new();
    let п_ = || п.clone();
    if *в.место.borrow() == Место::Корзина {
        let п1 = п_();
        пункт(&меню, "Восстановить", true, move || восстанови(&п1, false));
        let п1 = п_();
        пункт(&меню, "Удалить навсегда", true, move || удали(&п1, true));
    } else {
        let одна = записи.len() == 1;
        let первая = записи.first().cloned();
        if let Some(з) = первая.clone() {
            let п1 = п_();
            let з1 = з.clone();
            пункт(&меню, "Открыть", true, move || открой(&п1, &з1, false));
            if з.папка && одна {
                let п1 = п_();
                пункт(&меню, "Открыть в новой вкладке", true, move || открой(&п1, &з, true));
            }
            if matches!(&*в.место.borrow(), Место::Поиск(..)) {
                let п1 = п_();
                let з1 = записи[0].clone();
                пункт(&меню, "Показать в папке", true, move || {
                    if let (Some(в), Some(р)) = (текущая(&п1), з1.путь.parent()) {
                        *в.выделить.borrow_mut() = Some(з1.имя.clone());
                        иди(&п1, &в, Место::Папка(р.to_path_buf()));
                    }
                });
            }
        }
        черта(&меню);
        let п1 = п_();
        пункт(&меню, "Вырезать", true, move || в_буфер(&п1, true));
        let п1 = п_();
        пункт(&меню, "Копировать", true, move || в_буфер(&п1, false));
        if let Some(з) = первая.clone().filter(|з| з.папка && одна) {
            let п1 = п_();
            пункт(&меню, "Вставить в папку", п.буфер.borrow().is_some(), move || вставь_сюда(&п1, з.путь.clone()));
        }
        let пути = записи.iter().map(|з| з.путь.to_string_lossy().to_string()).collect::<Vec<_>>().join("\n");
        пункт(&меню, "Копировать путь", true, move || gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD).set_text(&пути));
        черта(&меню);
        let п1 = п_();
        пункт(&меню, "Переименовать", одна, move || переименуй(&п1));
        let п1 = п_();
        пункт(&меню, "Удалить", true, move || удали(&п1, false));
    }
    черта(&меню);
    let п1 = п_();
    пункт(&меню, "Свойства", true, move || свойства(&п1, &записи));
    меню.show_all();
    меню.popup_at_pointer(Some(событие));
}

fn меню_папки(п: &П, событие: &gtk::gdk::EventButton) {
    let Some(в) = текущая(п) else { return };
    let меню = gtk::Menu::new();
    let в_папке = текущая_папка(п).is_some();
    if *в.место.borrow() == Место::Корзина {
        let п1 = п.clone();
        пункт(&меню, "Восстановить всё", true, move || восстанови(&п1, true));
        let п1 = п.clone();
        пункт(&меню, "Очистить корзину", true, move || очисти_корзину(&п1));
        черта(&меню);
    } else if в_папке {
        let п1 = п.clone();
        пункт(&меню, "Создать папку", true, move || создай(&п1, true));
        let п1 = п.clone();
        пункт(&меню, "Создать текстовый файл", true, move || создай(&п1, false));
        let п1 = п.clone();
        let папка = текущая_папка(п).unwrap();
        пункт(&меню, "Вставить", п.буфер.borrow().is_some(), move || вставь_сюда(&п1, папка.clone()));
        черта(&меню);
    }
    let порядок = gtk::Menu::new();
    for (текст, столбец) in [("По имени", ИМЯ), ("По размеру", БАЙТ), ("По дате", ВРЕМЯ), ("По типу", ТИП)] {
        let м = в.модель.clone();
        пункт(&порядок, текст, true, move || м.set_sort_column_id(gtk::SortColumn::Index(столбец), gtk::SortType::Ascending));
    }
    let упорядочить = gtk::MenuItem::with_label("Упорядочить");
    упорядочить.set_submenu(Some(&порядок));
    меню.append(&упорядочить);
    let скрытые = gtk::CheckMenuItem::with_label("Скрытые файлы");
    скрытые.set_active(п.скрытые.get());
    let п1 = п.clone();
    скрытые.connect_toggled(move |_| переключи_скрытые(&п1));
    меню.append(&скрытые);
    let п1 = п.clone();
    пункт(&меню, "Обновить", true, move || {
        if let Some(в) = текущая(&п1) {
            загрузи(&п1, &в, true);
        }
    });
    if в_папке {
        черта(&меню);
        let п1 = п.clone();
        пункт(&меню, "Свойства", true, move || свойства(&п1, &[]));
    }
    меню.show_all();
    меню.popup_at_pointer(Some(событие));
}

fn переключи_скрытые(п: &П) {
    п.скрытые.set(!п.скрытые.get());
    for в in п.список.borrow().iter() {
        загрузи(п, в, true);
    }
}

fn переключи_вид(п: &П) {
    let таблицей = !п.таблицей.get();
    п.таблицей.set(таблицей);
    let _ = std::fs::create_dir_all(glib::user_config_dir().join("glower"));
    let _ = std::fs::write(glib::user_config_dir().join("glower").join("files-view"), if таблицей { "таблица" } else { "сетка" });
    for в in п.список.borrow().iter() {
        в.стопка.set_visible_child_name(if таблицей { "таблица" } else { "сетка" });
    }
    обнови_всё(п);
}

/* ---------- перетаскивание ---------- */

/// Таскать файлы списком ссылок — так их понимают и другие программы.
/// В gtk-rs этих вызовов нет, зовём GTK напрямую; цели он копирует себе
fn включи_перенос(сетка: &gtk::IconView, таблица: &gtk::TreeView) {
    use glib::translate::{IntoGlib, ToGlibPtr};
    let имя = std::ffi::CString::new("text/uri-list").unwrap();
    let цель = gtk::ffi::GtkTargetEntry { target: имя.as_ptr() as *mut _, flags: 0, info: 0 };
    let действия = (gtk::gdk::DragAction::COPY | gtk::gdk::DragAction::MOVE).into_glib();
    let кнопка = gtk::gdk::ModifierType::BUTTON1_MASK.into_glib();
    unsafe {
        gtk::ffi::gtk_icon_view_enable_model_drag_source(сетка.to_glib_none().0, кнопка, &цель, 1, действия);
        gtk::ffi::gtk_icon_view_enable_model_drag_dest(сетка.to_glib_none().0, &цель, 1, действия);
        gtk::ffi::gtk_tree_view_enable_model_drag_source(таблица.to_glib_none().0, кнопка, &цель, 1, действия);
        gtk::ffi::gtk_tree_view_enable_model_drag_dest(таблица.to_glib_none().0, &цель, 1, действия);
    }
}

/// Бросили файлы: в папку под курсором, а мимо папок — в текущую.
/// В пределах одного диска — переносим, с другого — копируем; с Ctrl — копия
fn брошено(п: &П, uris: Vec<glib::GString>, папка_под: Option<PathBuf>, копия: bool) -> bool {
    let Some(папка) = папка_под.or_else(|| текущая_папка(п)) else { return false };
    let пути: Vec<PathBuf> = uris.iter().filter_map(|u| gio::File::for_uri(u).path()).filter(|п2| *п2 != папка).collect();
    if пути.is_empty() {
        return false;
    }
    use std::os::unix::fs::MetadataExt;
    let диск = |п2: &Path| std::fs::metadata(п2).map(|м| м.dev()).ok();
    let вырезать = !копия && пути.iter().all(|п2| диск(п2) == диск(&папка));
    *п.буфер.borrow_mut() = Some((пути, вырезать));
    вставь_сюда(п, папка);
    true
}

fn подключи_перетаскивание<W: IsA<gtk::Widget>>(п: &П, в: &Rc<Вкладка>, вид: &W) {
    let пв = Rc::downgrade(п);
    let вв = Rc::downgrade(в);
    вид.connect_drag_data_get(move |w, _, данные, _, _| {
        let (Some(п), Some(в)) = (пв.upgrade(), вв.upgrade()) else { return };
        let uris: Vec<String> = выбранные(&п, &в).iter().map(|з| gio::File::for_path(&з.путь).uri().to_string()).collect();
        let ссылки: Vec<&str> = uris.iter().map(String::as_str).collect();
        данные.set_uris(&ссылки);
        w.stop_signal_emission_by_name("drag-data-get");
    });
    вид.connect_drag_data_delete(|w, _| w.stop_signal_emission_by_name("drag-data-delete"));
    let пв = Rc::downgrade(п);
    let вв = Rc::downgrade(в);
    вид.connect_drag_data_received(move |w, ctx, x, y, данные, _, время| {
        w.stop_signal_emission_by_name("drag-data-received");
        let (Some(п), Some(в)) = (пв.upgrade(), вв.upgrade()) else { return };
        let папка_под = путь_под(&п, &в, x as f64, y as f64)
            .and_then(|путь| в.модель.iter(&путь))
            .map(|и| запись_из(&в.модель, &и))
            .filter(|з| з.папка)
            .map(|з| з.путь);
        let копия = gtk::current_event_state().is_some_and(|с| с.contains(gtk::gdk::ModifierType::CONTROL_MASK));
        let ok = брошено(&п, данные.uris(), папка_под, копия);
        ctx.drag_finish(ok, false, время);
    });
}

/* ---------- окно ---------- */

fn новая_вкладка(п: &П, место: Место) -> Rc<Вкладка> {
    let модель = gtk::ListStore::new(&[
        Pixbuf::static_type(),
        Pixbuf::static_type(),
        String::static_type(),
        String::static_type(),
        String::static_type(),
        String::static_type(),
        String::static_type(),
        bool::static_type(),
        u64::static_type(),
        i64::static_type(),
    ]);
    // Папки — всегда первыми, дальше — по выбранному столбцу
    for столбец in [ИМЯ, БАЙТ, ВРЕМЯ, ТИП] {
        модель.set_sort_func(gtk::SortColumn::Index(столбец), move |м, а, б| {
            let (па, пб) = (м.value(а, ПАПКА as i32).get::<bool>().unwrap_or_default(), м.value(б, ПАПКА as i32).get::<bool>().unwrap_or_default());
            if па != пб {
                return if па { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater };
            }
            let имя = || м.value(а, ИМЯ as i32).get::<String>().unwrap_or_default().to_lowercase().cmp(&м.value(б, ИМЯ as i32).get::<String>().unwrap_or_default().to_lowercase());
            let первое = match столбец {
                БАЙТ => м.value(б, БАЙТ as i32).get::<u64>().unwrap_or_default().cmp(&м.value(а, БАЙТ as i32).get::<u64>().unwrap_or_default()),
                ВРЕМЯ => м.value(б, ВРЕМЯ as i32).get::<i64>().unwrap_or_default().cmp(&м.value(а, ВРЕМЯ as i32).get::<i64>().unwrap_or_default()),
                ТИП => м.value(а, ТИП as i32).get::<String>().unwrap_or_default().cmp(&м.value(б, ТИП as i32).get::<String>().unwrap_or_default()),
                _ => std::cmp::Ordering::Equal,
            };
            первое.then_with(имя)
        });
    }
    модель.set_sort_column_id(gtk::SortColumn::Index(ИМЯ), gtk::SortType::Ascending);

    let сетка = gtk::IconView::with_model(&модель);
    сетка.set_pixbuf_column(ЗНАЧОК as i32);
    сетка.set_text_column(ИМЯ as i32);
    сетка.set_selection_mode(gtk::SelectionMode::Multiple);
    сетка.set_item_width(92);
    сетка.set_column_spacing(6);
    сетка.set_row_spacing(6);
    IconViewExt::set_margin(&сетка, 10);
    сетка.set_activate_on_single_click(false);

    let таблица = gtk::TreeView::with_model(&модель);
    таблица.selection().set_mode(gtk::SelectionMode::Multiple);
    таблица.set_rubber_banding(true);
    таблица.set_enable_search(false);
    let столбец = |заголовок: &str, данные: u32, сорт: u32, растянуть: bool| {
        let с = gtk::TreeViewColumn::new();
        с.set_title(заголовок);
        с.set_resizable(true);
        с.set_expand(растянуть);
        с.set_sort_column_id(сорт as i32);
        if данные == ИМЯ {
            let з = gtk::CellRendererPixbuf::new();
            TreeViewColumnExt::pack_start(&с, &з, false);
            TreeViewColumnExt::add_attribute(&с, &з, "pixbuf", МАЛЫЙ as i32);
        }
        let т = gtk::CellRendererText::new();
        if данные == ИМЯ {
            т.set_property("ellipsize", gtk::pango::EllipsizeMode::End);
        }
        TreeViewColumnExt::pack_start(&с, &т, true);
        TreeViewColumnExt::add_attribute(&с, &т, "text", данные as i32);
        таблица.append_column(&с);
    };
    столбец("Имя", ИМЯ, ИМЯ, true);
    столбец("Изменён", ДАТА, ВРЕМЯ, false);
    столбец("Тип", ТИП, ТИП, false);
    столбец("Размер", РАЗМЕР, БАЙТ, false);

    let стопка = gtk::Stack::new();
    for (имя, вид) in [("сетка", сетка.clone().upcast::<gtk::Widget>()), ("таблица", таблица.clone().upcast())] {
        let прокрутка = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        прокрутка.add(&вид);
        стопка.add_named(&прокрутка, имя);
    }
    let в = Rc::new(Вкладка {
        место: RefCell::new(место),
        назад: RefCell::new(Vec::new()),
        вперёд: RefCell::new(Vec::new()),
        модель,
        сетка,
        таблица,
        стопка,
        ярлык: gtk::Label::new(None),
        наблюдатель: RefCell::new(None),
        поколение: Cell::new(0),
        выделить: RefCell::new(None),
    });
    в.ярлык.set_max_width_chars(22);
    в.ярлык.set_ellipsize(gtk::pango::EllipsizeMode::End);

    // Открыть двойным щелчком или Enter
    {
        let пв = Rc::downgrade(п);
        let вв = Rc::downgrade(&в);
        в.сетка.connect_item_activated(move |_, путь| {
            let (Some(п), Some(в)) = (пв.upgrade(), вв.upgrade()) else { return };
            if let Some(и) = в.модель.iter(путь) {
                открой(&п, &запись_из(&в.модель, &и), false);
            }
        });
        let пв = Rc::downgrade(п);
        let вв = Rc::downgrade(&в);
        в.таблица.connect_row_activated(move |_, путь, _| {
            let (Some(п), Some(в)) = (пв.upgrade(), вв.upgrade()) else { return };
            if let Some(и) = в.модель.iter(путь) {
                открой(&п, &запись_из(&в.модель, &и), false);
            }
        });
    }
    // Выделение — в строку состояния
    {
        let пв = Rc::downgrade(п);
        в.сетка.connect_selection_changed(move |_| {
            if let Some(п) = пв.upgrade() {
                обнови_строку(&п);
            }
        });
        let пв = Rc::downgrade(п);
        в.таблица.selection().connect_changed(move |_| {
            if let Some(п) = пв.upgrade() {
                обнови_строку(&п);
            }
        });
    }
    // Правая кнопка — меню; средняя по папке — новая вкладка
    for вид in [в.сетка.clone().upcast::<gtk::Widget>(), в.таблица.clone().upcast()] {
        let пв = Rc::downgrade(п);
        let вв = Rc::downgrade(&в);
        вид.connect_button_press_event(move |_, е| {
            let (Some(п), Some(в)) = (пв.upgrade(), вв.upgrade()) else { return glib::Propagation::Proceed };
            if е.event_type() != gtk::gdk::EventType::ButtonPress {
                return glib::Propagation::Proceed;
            }
            let (x, y) = е.position();
            let под = путь_под(&п, &в, x, y);
            match (е.button(), под) {
                (3, Some(путь)) => {
                    if !выбрано_ли(&п, &в, &путь) {
                        выбери_одну(&п, &в, &путь);
                    }
                    меню_записей(&п, е);
                    glib::Propagation::Stop
                }
                (3, None) => {
                    в.сетка.unselect_all();
                    в.таблица.selection().unselect_all();
                    меню_папки(&п, е);
                    glib::Propagation::Stop
                }
                (2, Some(путь)) => {
                    if let Some(и) = в.модель.iter(&путь) {
                        let з = запись_из(&в.модель, &и);
                        if з.папка {
                            новая_вкладка(&п, Место::Папка(з.путь));
                        }
                    }
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
    }
    // Перетаскивание
    включи_перенос(&в.сетка, &в.таблица);
    подключи_перетаскивание(п, &в, &в.сетка);
    подключи_перетаскивание(п, &в, &в.таблица);

    // Ярлык вкладки с крестиком
    let шапка = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    шапка.add(&в.ярлык);
    let крест = crate::apps::значок_кнопка("×");
    крест.style_context().add_class("tab-close");
    {
        let пв = Rc::downgrade(п);
        let вв = Rc::downgrade(&в);
        крест.connect_clicked(move |_| {
            if let (Some(п), Some(в)) = (пв.upgrade(), вв.upgrade()) {
                закрой_вкладку(&п, &в);
            }
        });
    }
    шапка.add(&крест);
    шапка.show_all();
    в.стопка.show_all();
    в.стопка.set_visible_child_name(if п.таблицей.get() { "таблица" } else { "сетка" });
    п.список.borrow_mut().push(в.clone());
    let н = п.вкладки.append_page(&в.стопка, Some(&шапка));
    п.вкладки.set_tab_reorderable(&в.стопка, false);
    п.вкладки.set_show_tabs(п.список.borrow().len() > 1);
    загрузи(п, &в, false);
    п.вкладки.set_current_page(Some(н));
    в
}

fn закрой_вкладку(п: &П, в: &Rc<Вкладка>) {
    if п.список.borrow().len() <= 1 {
        п.окно.close();
        return;
    }
    let Some(н) = п.список.borrow().iter().position(|х| Rc::ptr_eq(х, в)) else { return };
    в.поколение.set(в.поколение.get() + 1);
    *в.наблюдатель.borrow_mut() = None;
    п.список.borrow_mut().remove(н);
    п.вкладки.remove_page(Some(н as u32));
    п.вкладки.set_show_tabs(п.список.borrow().len() > 1);
    обнови_всё(п);
}

fn собери_места(п: &П) {
    for р in п.места.children() {
        п.места.remove(&р);
    }
    let mut цели = Vec::new();
    let mut ряд = |знак: &str, текст: &str, цель: Место| {
        let р = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        р.add(&crate::apps::значок_эмодзи(знак, 16));
        let л = gtk::Label::new(Some(текст));
        л.set_ellipsize(gtk::pango::EllipsizeMode::End);
        л.set_xalign(0.0);
        р.add(&л);
        п.места.add(&р);
        цели.push(цель);
    };
    ряд("💻", "Этот компьютер", Место::Папка(п.корень.clone()));
    for (знак, имя) in [("🖥️", "Рабочий стол"), ("📄", "Документы"), ("🖼️", "Изображения"), ("🎵", "Музыка"), ("⬇️", "Загрузки")] {
        let путь = п.корень.join(имя);
        if путь.is_dir() {
            ряд(знак, имя, Место::Папка(путь));
        }
    }
    let дом = glib::home_dir();
    if дом != п.корень {
        ряд("🏠", "Домашняя папка", Место::Папка(дом));
    }
    ряд("🗑️", "Корзина", Место::Корзина);
    // Флешки и диски
    for м in gio::VolumeMonitor::get().mounts() {
        if let Some(корень) = м.root().path() {
            ряд("💾", &м.name(), Место::Папка(корень));
        }
    }
    ряд("🖴", "Файловая система", Место::Папка(PathBuf::from("/")));
    *п.цели.borrow_mut() = цели;
    п.места.show_all();
}

fn латинская(е: &gtk::gdk::EventKey) -> String {
    crate::notepad::латинская(е)
}

pub fn запусти(доводы: Vec<String>) -> ! {
    подготовь("glower-files", "Проводник");
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль проводника");
    gtk::StyleContext::add_provider_for_screen(&gtk::gdk::Screen::default().expect("экран"), &стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);

    // Корень — папка системы: та же, что у агента и страничного Проводника
    let корень = std::env::var("GLOWER_ROOT")
        .map(PathBuf::from)
        .ok()
        .filter(|п| п.is_dir())
        .or_else(|| Some(glib::home_dir().join("GlowerOS")).filter(|п| п.is_dir()))
        .unwrap_or_else(glib::home_dir);

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_default_size(980, 620);
    окно.set_icon_name(Some("system-file-manager"));
    окно.style_context().add_class("glower-app");
    crate::apps::строка_заголовка(&окно);
    окно.style_context().add_class("files");

    let колонка = gtk::Box::new(gtk::Orientation::Vertical, 0);
    окно.add(&колонка);

    // Панель: назад, вперёд, вверх, адрес, вид, поиск
    let панель = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    панель.style_context().add_class("toolbar");
    let к_назад = кнопка("‹", "Назад (Alt+←)");
    let к_вперёд = кнопка("›", "Вперёд (Alt+→)");
    let к_вверх = кнопка("↑", "Вверх (Alt+↑)");
    панель.add(&к_назад);
    панель.add(&к_вперёд);
    панель.add(&к_вверх);
    let крошки = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    крошки.style_context().add_class("crumbs");
    let адрес = gtk::Entry::new();
    let адресная = gtk::Stack::new();
    let крошки_место = gtk::EventBox::new();
    крошки_место.add(&крошки);
    адресная.add_named(&крошки_место, "крошки");
    адресная.add_named(&адрес, "адрес");
    адресная.set_hexpand(true);
    панель.add(&адресная);
    let к_папка = кнопка("＋ Папка", "Создать папку (Ctrl+Shift+N)");
    панель.add(&к_папка);
    let к_вид = кнопка("☰", "Сетка или таблица");
    панель.add(&к_вид);
    let поиск = gtk::Entry::new();
    поиск.set_placeholder_text(Some("Поиск"));
    поиск.set_icon_from_icon_name(gtk::EntryIconPosition::Primary, Some("edit-find-symbolic"));
    поиск.set_width_chars(20);
    панель.add(&поиск);
    колонка.add(&панель);

    // Корзина: свои кнопки
    let корзина = gtk::Revealer::new();
    let полоса = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    полоса.style_context().add_class("trashbar");
    let к_вернуть = кнопка("↩ Восстановить всё", "Вернуть всё на свои места");
    let к_очистить = кнопка("🧹 Очистить корзину", "Удалить всё навсегда");
    полоса.add(&к_вернуть);
    полоса.add(&к_очистить);
    корзина.add(&полоса);
    колонка.add(&корзина);

    let середина = gtk::Paned::new(gtk::Orientation::Horizontal);
    let места = gtk::ListBox::new();
    места.set_selection_mode(gtk::SelectionMode::Single);
    let бок = gtk::Box::new(gtk::Orientation::Vertical, 0);
    бок.style_context().add_class("sidebar");
    let прокрутка = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    прокрутка.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    прокрутка.add(&места);
    прокрутка.set_vexpand(true);
    бок.add(&прокрутка);
    бок.set_size_request(190, -1);
    середина.pack1(&бок, false, false);
    let вкладки = gtk::Notebook::new();
    вкладки.set_scrollable(true);
    вкладки.set_show_border(false);
    середина.pack2(&вкладки, true, false);
    середина.set_vexpand(true);
    колонка.add(&середина);

    let строка = gtk::Label::new(None);
    строка.set_xalign(0.0);
    строка.style_context().add_class("statusbar");
    колонка.add(&строка);

    let таблицей = std::fs::read_to_string(glib::user_config_dir().join("glower").join("files-view")).map(|т| т.trim() == "таблица").unwrap_or(false);
    let п: П = Rc::new(Проводник {
        окно: окно.clone(),
        вкладки: вкладки.clone(),
        список: RefCell::new(Vec::new()),
        назад: к_назад.clone(),
        вперёд: к_вперёд.clone(),
        вверх: к_вверх.clone(),
        крошки,
        адрес: адрес.clone(),
        адресная: адресная.clone(),
        поиск: поиск.clone(),
        строка,
        корзина,
        вид: к_вид.clone(),
        новая: к_папка.clone(),
        места: места.clone(),
        цели: RefCell::new(Vec::new()),
        таблицей: Cell::new(таблицей),
        скрытые: Cell::new(false),
        буфер: RefCell::new(None),
        корень: корень.clone(),
        значки: RefCell::new(HashMap::new()),
    });
    собери_места(&п);
    {
        let монитор = gio::VolumeMonitor::get();
        let пв = Rc::downgrade(&п);
        монитор.connect_mount_added(move |_, _| {
            if let Some(п) = пв.upgrade() {
                собери_места(&п);
                обнови_всё(&п);
            }
        });
        let пв = Rc::downgrade(&п);
        монитор.connect_mount_removed(move |_, _| {
            if let Some(п) = пв.upgrade() {
                собери_места(&п);
                обнови_всё(&п);
            }
        });
        // Монитор живёт, пока живёт окно
        unsafe { окно.set_data("монитор", монитор) };
    }

    {
        let п1 = п.clone();
        места.connect_row_activated(move |_, р| {
            let цель = п1.цели.borrow().get(р.index() as usize).cloned();
            if let (Some(цель), Some(в)) = (цель, текущая(&п1)) {
                if *в.место.borrow() != цель {
                    иди(&п1, &в, цель);
                }
            }
        });
        let п1 = п.clone();
        к_назад.connect_clicked(move |_| назад(&п1));
        let п1 = п.clone();
        к_вперёд.connect_clicked(move |_| вперёд(&п1));
        let п1 = п.clone();
        к_вверх.connect_clicked(move |_| вверх(&п1));
        let п1 = п.clone();
        к_папка.connect_clicked(move |_| создай(&п1, true));
        let п1 = п.clone();
        к_вид.connect_clicked(move |_| переключи_вид(&п1));
        let п1 = п.clone();
        к_вернуть.connect_clicked(move |_| восстанови(&п1, true));
        let п1 = п.clone();
        к_очистить.connect_clicked(move |_| очисти_корзину(&п1));
        let п1 = п.clone();
        вкладки.connect_switch_page(move |_, _, _| {
            let п2 = п1.clone();
            glib::idle_add_local_once(move || обнови_всё(&п2));
        });
    }

    // Адресная строка: щелчок по пустому месту крошек или Ctrl+L
    let покажи_адрес = {
        let п1 = п.clone();
        move || {
            let путь = match текущая(&п1).map(|в| в.место.borrow().clone()) {
                Some(Место::Папка(папка)) | Some(Место::Поиск(папка, _)) => папка.display().to_string(),
                _ => String::new(),
            };
            п1.адрес.set_text(&путь);
            п1.адрес.style_context().remove_class("error");
            п1.адресная.set_visible_child_name("адрес");
            п1.адрес.grab_focus();
        }
    };
    {
        let покажи = покажи_адрес.clone();
        крошки_место.connect_button_press_event(move |_, е| {
            if е.button() == 1 {
                покажи();
            }
            glib::Propagation::Proceed
        });
        let п1 = п.clone();
        адрес.connect_activate(move |поле| {
            let текст = поле.text().trim().to_string();
            let путь = match текст.strip_prefix('~') {
                Some(хвост) => PathBuf::from(format!("{}{хвост}", glib::home_dir().display())),
                None => PathBuf::from(&текст),
            };
            let Some(в) = текущая(&п1) else { return };
            if путь.is_dir() {
                п1.адресная.set_visible_child_name("крошки");
                иди(&п1, &в, Место::Папка(путь));
            } else if путь.is_file() {
                п1.адресная.set_visible_child_name("крошки");
                *в.выделить.borrow_mut() = путь.file_name().map(|и| и.to_string_lossy().to_string());
                иди(&п1, &в, Место::Папка(путь.parent().map(Path::to_path_buf).unwrap_or_default()));
            } else {
                поле.style_context().add_class("error");
            }
        });
        let п1 = п.clone();
        адрес.connect_focus_out_event(move |_, _| {
            п1.адресная.set_visible_child_name("крошки");
            glib::Propagation::Proceed
        });
    }
    // Поиск: набираешь — отбирает по имени в этой папке и глубже
    {
        let п1 = п.clone();
        let ждём: Rc<Cell<u32>> = Rc::new(Cell::new(0));
        поиск.connect_changed(move |поле| {
            let номер = ждём.get() + 1;
            ждём.set(номер);
            let (п2, ждём2, текст) = (п1.clone(), ждём.clone(), поле.text().trim().to_string());
            glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                if ждём2.get() != номер {
                    return;
                }
                let Some(в) = текущая(&п2) else { return };
                let м = в.место.borrow().clone();
                let где = match &м {
                    Место::Папка(папка) | Место::Поиск(папка, _) => папка.clone(),
                    Место::Корзина => return,
                };
                if текст.is_empty() {
                    if matches!(м, Место::Поиск(..)) {
                        в.место.replace(Место::Папка(где));
                        загрузи(&п2, &в, false);
                    }
                } else {
                    // Поиск не множит историю: «назад» ведёт в папку, а не по буквам
                    if matches!(м, Место::Папка(_)) {
                        в.назад.borrow_mut().push(м.clone());
                    }
                    в.место.replace(Место::Поиск(где, текст));
                    загрузи(&п2, &в, false);
                }
            });
        });
    }

    // Клавиши — по латинской букве клавиши, при любой раскладке
    {
        let п1 = п.clone();
        let покажи = покажи_адрес.clone();
        окно.connect_key_press_event(move |окно, е| {
            use gtk::gdk::keys::constants as к;
            use gtk::gdk::ModifierType as М;
            let ключ = е.keyval();
            let с = е.state();
            let ctrl = с.contains(М::CONTROL_MASK);
            let shift = с.contains(М::SHIFT_MASK);
            let alt = с.contains(М::MOD1_MASK);
            let буква = латинская(е);
            let в_поле = окно.focused_widget().map(|ф| ф.is::<gtk::Entry>()).unwrap_or(false);
            if ключ == к::Escape {
                if п1.адресная.visible_child_name().as_deref() == Some("адрес") {
                    п1.адресная.set_visible_child_name("крошки");
                } else if !п1.поиск.text().is_empty() {
                    п1.поиск.set_text("");
                }
                if let Some(в) = текущая(&п1) {
                    if п1.таблицей.get() { в.таблица.grab_focus() } else { в.сетка.grab_focus() }
                }
                return glib::Propagation::Stop;
            }
            let сделано = if ctrl && буква == "l" {
                покажи();
                true
            } else if ctrl && буква == "f" {
                п1.поиск.grab_focus();
                true
            } else if ctrl && буква == "t" {
                let где = текущая_папка(&п1).unwrap_or_else(|| п1.корень.clone());
                новая_вкладка(&п1, Место::Папка(где));
                true
            } else if ctrl && буква == "w" {
                if let Some(в) = текущая(&п1) {
                    закрой_вкладку(&п1, &в);
                }
                true
            } else if ctrl && буква == "h" {
                переключи_скрытые(&п1);
                true
            } else if ctrl && shift && буква == "n" {
                создай(&п1, true);
                true
            } else if alt && ключ == к::Left {
                назад(&п1);
                true
            } else if alt && ключ == к::Right {
                вперёд(&п1);
                true
            } else if alt && ключ == к::Up {
                вверх(&п1);
                true
            } else if ключ == к::F5 {
                if let Some(в) = текущая(&п1) {
                    загрузи(&п1, &в, true);
                }
                true
            } else if в_поле {
                false
            } else if ctrl && буква == "c" {
                в_буфер(&п1, false);
                true
            } else if ctrl && буква == "x" {
                в_буфер(&п1, true);
                true
            } else if ctrl && буква == "v" {
                if let Some(папка) = текущая_папка(&п1) {
                    вставь_сюда(&п1, папка);
                }
                true
            } else if ctrl && буква == "a" {
                if let Some(в) = текущая(&п1) {
                    if п1.таблицей.get() { в.таблица.selection().select_all() } else { в.сетка.select_all() }
                }
                true
            } else if ключ == к::Delete || ключ == к::KP_Delete {
                удали(&п1, shift);
                true
            } else if ключ == к::F2 {
                переименуй(&п1);
                true
            } else if ключ == к::BackSpace {
                вверх(&п1);
                true
            } else if alt && ключ == к::Return {
                if let Some(в) = текущая(&п1) {
                    свойства(&п1, &выбранные(&п1, &в));
                }
                true
            } else {
                false
            };
            if сделано { glib::Propagation::Stop } else { glib::Propagation::Proceed }
        });
    }
    окно.connect_delete_event(|_, _| {
        gtk::main_quit();
        glib::Propagation::Proceed
    });

    // Что открыть: папку из доводов, а файл — его папку, с ним выделенным
    let mut первая = Место::Папка(корень.clone());
    let mut выделить = None;
    if доводы.first().is_some_and(|д| д == "--trash") {
        первая = Место::Корзина;
    } else if let Some(д) = доводы.first() {
        let путь = PathBuf::from(д);
        if путь.is_dir() {
            первая = Место::Папка(путь);
        } else if путь.exists() {
            if let Some(р) = путь.parent() {
                первая = Место::Папка(р.to_path_buf());
                выделить = путь.file_name().map(|и| и.to_string_lossy().to_string());
            }
        }
    }
    окно.show_all();
    п.адресная.set_visible_child_name("крошки");
    п.корзина.set_reveal_child(false);
    let в = новая_вкладка(&п, первая);
    if выделить.is_some() {
        *в.выделить.borrow_mut() = выделить;
        загрузи(&п, &в, false);
    }
    if п.таблицей.get() { в.таблица.grab_focus() } else { в.сетка.grab_focus() }
    gtk::main();
    std::process::exit(0);
}

#[cfg(test)]
mod проверки {
    use super::*;

    #[test]
    fn размеры_по_русски() {
        assert_eq!(размер_текстом(512), "512 Б");
        assert_eq!(размер_текстом(1536), "1,5 КБ");
        assert_eq!(размер_текстом(20 << 20), "20 МБ");
    }

    #[test]
    fn раскодирует_путь_корзины() {
        assert_eq!(раскодируй("/home/%D0%94%D0%BE%D0%BA/a%20b.txt"), "/home/Док/a b.txt");
        assert_eq!(раскодируй("100%"), "100%");
    }

    #[test]
    fn копирует_переносит_и_не_затирает() {
        let корень = std::env::temp_dir().join(format!("glower-files-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&корень);
        let (а, б) = (корень.join("а"), корень.join("б"));
        std::fs::create_dir_all(а.join("внутри")).unwrap();
        std::fs::create_dir_all(&б).unwrap();
        std::fs::write(а.join("внутри").join("x.txt"), "привет").unwrap();
        std::fs::write(б.join("x.txt"), "старое").unwrap();
        // Копия папки — со всем, что внутри
        assert!(вставь(&[а.join("внутри")], &б, false).is_empty());
        assert_eq!(std::fs::read_to_string(б.join("внутри").join("x.txt")).unwrap(), "привет");
        // Занятое имя не затираем — рядом встаёт «x (2).txt»
        assert!(вставь(&[а.join("внутри").join("x.txt")], &б, true).is_empty());
        assert_eq!(std::fs::read_to_string(б.join("x.txt")).unwrap(), "старое");
        assert_eq!(std::fs::read_to_string(б.join("x (2).txt")).unwrap(), "привет");
        assert!(!а.join("внутри").join("x.txt").exists());
        // Папку в саму себя — нельзя
        assert_eq!(вставь(&[б.clone()], &б.join("внутри"), false).len(), 1);
        let _ = std::fs::remove_dir_all(&корень);
    }
}
