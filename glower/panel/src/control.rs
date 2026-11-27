//! Панель управления (glower-control) — как в Windows: всё, чем
//! настраивается машина, в одном окне, по категориям или значками.
//!
//! Сама она ничего не настраивает — каждый пункт открывает настоящее место
//! системы: раздел Параметров, Диспетчер задач, Магазин с установленными
//! программами, журнал системы в терминале. Так у настройки одно место, а
//! дорог к нему — сколько удобно.

use crate::apps::подготовь;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

const СТИЛЬ: &str = r#"
.control .top { padding: 14px 22px; border-bottom: 1px solid alpha(@glower_fg, 0.07); }
.control .top .title { font-size: 20px; font-weight: 700; }
.control .body { padding: 18px 22px 30px; }
.control .cat { padding: 12px; border-radius: 14px; }
.control .cat:hover { background-color: alpha(@glower_fg, 0.04); }
.control button.link { background: none; border: none; box-shadow: none; padding: 1px 2px; }
.control button.link label { color: @glower_accent2; }
.control button.link:hover label { text-decoration-line: underline; }
.control button.link.head label { font-size: 15px; font-weight: 700; color: @glower_accent; }
.control button.item { border-radius: 12px; padding: 10px 12px; background: none; border: 1px solid transparent; box-shadow: none; }
.control button.item:hover { background-color: alpha(@glower_fg, 0.06); border-color: alpha(@glower_fg, 0.08); }
.control .h { font-size: 17px; font-weight: 700; margin-bottom: 10px; }
.control .note { opacity: 0.65; }
"#;

pub struct Категория {
    pub id: &'static str,
    pub имя: &'static str,
    pub знак: &'static str,
}

pub const КАТЕГОРИИ: &[Категория] = &[
    Категория { id: "система", имя: "Система и безопасность", знак: "🛡" },
    Категория { id: "сеть", имя: "Сеть и Интернет", знак: "📶" },
    Категория { id: "оборудование", имя: "Оборудование и звук", знак: "🖨" },
    Категория { id: "программы", имя: "Программы", знак: "📦" },
    Категория { id: "люди", имя: "Учётные записи", знак: "👥" },
    Категория { id: "оформление", имя: "Оформление и персонализация", знак: "🎨" },
    Категория { id: "часы", имя: "Часы и регион", знак: "🌍" },
    Категория { id: "администрирование", имя: "Администрирование", знак: "🛠" },
];

pub struct Пункт {
    pub имя: &'static str,
    pub знак: &'static str,
    pub где: &'static str,
    /// Команда: программа и доводы
    pub дело: &'static [&'static str],
    pub слова: &'static str,
    /// Показывать ссылкой под заголовком категории
    pub главный: bool,
}

const fn п(имя: &'static str, знак: &'static str, где: &'static str, дело: &'static [&'static str], слова: &'static str, главный: bool) -> Пункт {
    Пункт { имя, знак, где, дело, слова, главный }
}

pub const ПУНКТЫ: &[Пункт] = &[
    // Система и безопасность
    п("Система", "ℹ", "система", &["glower-settings", "about"], "о системе версия процессор память компьютер", true),
    п("Электропитание", "🔋", "система", &["glower-settings", "system"], "питание батарея сон спящий режим", true),
    п("Брандмауэр", "🔐", "система", &["glower-settings", "privacy"], "стена сеть защита порты", false),
    п("Защита от вирусов", "🛡", "система", &["glower-scan"], "антивирус clamav карантин проверка", true),
    п("Центр обновлений", "⬆", "система", &["glower-settings", "update"], "обновления версия прошивка", true),
    п("Резервная копия на Ключ Glower", "🔑", "система", &["glower-settings", "key"], "копия файлов флешка резервная", false),
    // Сеть и Интернет
    п("Сеть и Интернет", "📶", "сеть", &["glower-settings", "net"], "wi-fi вайфай кабель ethernet ip dns", true),
    п("Сетевые подключения", "📡", "сеть", &["foot", "-e", "nmtui"], "nmtui подключения адаптеры vpn", true),
    // Оборудование и звук
    п("Устройства и принтеры", "🖨", "оборудование", &["glower-settings", "print"], "принтер сканер печать", true),
    п("Bluetooth и устройства", "🔵", "оборудование", &["glower-settings", "bt"], "bluetooth блютуз наушники", true),
    п("Звук", "🔊", "оборудование", &["glower-settings", "sound"], "громкость динамики микрофон", true),
    п("Дисплей", "💡", "оборудование", &["glower-settings", "display"], "экран разрешение яркость частота", false),
    п("Мышь и тачпад", "🖱", "оборудование", &["glower-settings", "mouse"], "мышь курсор тачпад", false),
    п("Клавиатура", "⌨", "оборудование", &["glower-settings", "keys"], "сочетания клавиши", false),
    п("Носители", "💾", "оборудование", &["glower-settings", "drives"], "флешка диск usb карта памяти", false),
    п("Игры", "🎮", "оборудование", &["glower-settings", "games"], "steam игровой режим видеокарта драйвер", false),
    // Программы
    п("Программы и компоненты", "📦", "программы", &["glower-store", "installed"], "удалить программу установленные", true),
    п("Магазин", "🛍", "программы", &["glower-store"], "установить программу flathub", true),
    п("Автозапуск и программы Windows", "🚀", "программы", &["glower-settings", "apps"], "автозапуск wine exe по умолчанию", false),
    п("Обновления программ", "⬇", "программы", &["glower-store", "updates"], "обновить программы flathub", false),
    // Учётные записи
    п("Учётные записи", "👤", "люди", &["glower-settings", "acc"], "пароль пользователь люди блокировка", true),
    п("Ключ Glower", "🔑", "люди", &["glower-settings", "key"], "вход без пароля флешка перенос стола", true),
    п("Пароли и хранилище", "🔒", "люди", &["glower-settings", "privacy"], "пароли хранилище ключи", false),
    // Оформление
    п("Персонализация", "🎨", "оформление", &["glower-settings", "person"], "обои тема цвет акцент шрифт прозрачность", true),
    п("Рабочий стол и док", "🧱", "оформление", &["glower-settings", "dock"], "док панель задач значки виджеты", true),
    п("Уведомления и фокус", "🔔", "оформление", &["glower-settings", "notif"], "не беспокоить фокусировка", false),
    // Часы и регион
    п("Дата и время", "🕐", "часы", &["glower-settings", "time"], "часы пояс время синхронизация", true),
    п("Язык и раскладки", "🗣", "часы", &["glower-settings", "time"], "язык раскладка клавиатуры регион", true),
    п("Часы и будильники", "⏰", "часы", &["glower-clock"], "будильник таймер секундомер", false),
    // Администрирование
    п("Диспетчер задач", "📊", "администрирование", &["glower-taskmgr"], "процессы память нагрузка", true),
    п("Терминал", "🧑\u{200d}💻", "администрирование", &["foot"], "консоль командная строка cmd", true),
    п("Выполнить", "🚀", "администрирование", &["glower-run"], "win+r команда запуск", false),
    п("Журнал событий", "📄", "администрирование", &["foot", "-e", "journalctl", "-b", "-e"], "журнал события ошибки логи", true),
    п("Службы", "⚙", "администрирование", &["foot", "-e", "systemctl", "list-units", "--type=service"], "службы сервисы systemd", false),
    п("Управление дисками", "🖴", "администрирование", &["foot", "-e", "sh", "-c", "lsblk -o NAME,SIZE,FSTYPE,MOUNTPOINTS,LABEL; df -h; echo; read -p 'Enter — закрыть' x"], "диски разделы место", false),
    п("Сведения о неполадках", "🧪", "администрирование", &["foot", "-e", "sh", "-c", "врач; echo; read -p 'Enter — закрыть' x"], "врач диагностика неполадки", false),
];

#[derive(Clone, PartialEq)]
enum Вид {
    Категории,
    Значки,
    Категория(&'static str),
    Поиск(String),
}

struct Окно {
    окно: gtk::Window,
    тело: gtk::Box,
    вид: RefCell<Вид>,
    выбор: gtk::ComboBoxText,
}

type П = Rc<Окно>;

fn значок(знак: &str, размер: i32) -> gtk::Widget {
    crate::apps::значок_эмодзи(знак, размер)
}

fn открой(п: &П, пункт: &Пункт) {
    let mut к = std::process::Command::new(пункт.дело[0]);
    к.args(&пункт.дело[1..]);
    match к.spawn() {
        Ok(mut ребёнок) => {
            std::thread::spawn(move || ребёнок.wait());
        }
        Err(е) => {
            let д = gtk::MessageDialog::new(Some(&п.окно), gtk::DialogFlags::MODAL, gtk::MessageType::Error, gtk::ButtonsType::Ok, &format!("Не открылось: {}", пункт.имя));
            д.set_secondary_text(Some(&е.to_string()));
            д.run();
            unsafe { д.destroy() };
        }
    }
}

fn ссылка(текст: &str, заголовок: bool) -> gtk::Button {
    let б = gtk::Button::with_label(текст);
    б.style_context().add_class("link");
    if заголовок {
        б.style_context().add_class("head");
    }
    б.set_halign(gtk::Align::Start);
    if let Some(л) = б.child().and_then(|ч| ч.downcast::<gtk::Label>().ok()) {
        л.set_xalign(0.0);
        л.set_line_wrap(true);
    }
    б
}

fn плитка(п: &П, пункт: &'static Пункт) -> gtk::Button {
    let б = gtk::Button::new();
    б.style_context().add_class("item");
    let р = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    р.add(&значок(пункт.знак, 32));
    let л = gtk::Label::new(Some(пункт.имя));
    л.set_xalign(0.0);
    л.set_line_wrap(true);
    л.set_max_width_chars(22);
    р.add(&л);
    б.add(&р);
    let п2 = п.clone();
    б.connect_clicked(move |_| открой(&п2, пункт));
    б
}

fn очисти(к: &gtk::Box) {
    for р in к.children() {
        к.remove(&р);
    }
}

fn иди(п: &П, вид: Вид) {
    *п.вид.borrow_mut() = вид;
    рисуй(п);
}

fn рисуй(п: &П) {
    очисти(&п.тело);
    let вид = п.вид.borrow().clone();
    п.выбор.set_active_id(Some(if вид == Вид::Значки { "значки" } else { "категории" }));
    match вид {
        Вид::Категории => категории(п),
        Вид::Значки => {
            let mut все: Vec<&'static Пункт> = ПУНКТЫ.iter().collect();
            все.sort_by_key(|п| п.имя.to_lowercase());
            все.dedup_by_key(|п| п.имя);
            let с = сетка();
            for пункт in все {
                с.add(&плитка(п, пункт));
            }
            п.тело.add(&с);
        }
        Вид::Категория(id) => {
            let назад = gtk::Button::with_label("‹ Все категории");
            назад.set_halign(gtk::Align::Start);
            let п2 = п.clone();
            назад.connect_clicked(move |_| иди(&п2, Вид::Категории));
            п.тело.add(&назад);
            let к = КАТЕГОРИИ.iter().find(|к| к.id == id);
            let з = gtk::Label::new(к.map(|к| к.имя));
            з.style_context().add_class("h");
            з.set_xalign(0.0);
            з.set_margin_top(12);
            п.тело.add(&з);
            let с = сетка();
            for пункт in ПУНКТЫ.iter().filter(|х| х.где == id) {
                с.add(&плитка(п, пункт));
            }
            п.тело.add(&с);
        }
        Вид::Поиск(что) => {
            let найдено = найди(&что);
            let з = gtk::Label::new(Some(&format!("Найдено: {}", найдено.len())));
            з.style_context().add_class("h");
            з.set_xalign(0.0);
            п.тело.add(&з);
            if найдено.is_empty() {
                let н = gtk::Label::new(Some("Ничего не нашлось. Попробуйте другое слово — например, «звук» или «пароль»."));
                н.style_context().add_class("note");
                н.set_xalign(0.0);
                п.тело.add(&н);
            }
            let с = сетка();
            for пункт in найдено {
                с.add(&плитка(п, пункт));
            }
            п.тело.add(&с);
        }
    }
    п.тело.show_all();
}

fn сетка() -> gtk::FlowBox {
    let с = gtk::FlowBox::new();
    с.set_selection_mode(gtk::SelectionMode::None);
    с.set_homogeneous(true);
    с.set_max_children_per_line(4);
    с.set_row_spacing(6);
    с.set_column_spacing(6);
    с
}

/// Поиск — по имени и по словам: «пароль» находит учётные записи и Ключ
pub fn найди(что: &str) -> Vec<&'static Пункт> {
    let что = что.trim().to_lowercase();
    if что.is_empty() {
        return Vec::new();
    }
    let mut итог: Vec<&'static Пункт> = ПУНКТЫ.iter().filter(|п| п.имя.to_lowercase().contains(&что) || п.слова.contains(&что)).collect();
    итог.dedup_by_key(|п| п.имя);
    итог
}

fn категории(п: &П) {
    let с = gtk::FlowBox::new();
    с.set_selection_mode(gtk::SelectionMode::None);
    с.set_homogeneous(true);
    с.set_max_children_per_line(2);
    с.set_min_children_per_line(1);
    с.set_row_spacing(8);
    с.set_column_spacing(16);
    for к in КАТЕГОРИИ {
        let блок = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        блок.style_context().add_class("cat");
        let з = значок(к.знак, 48);
        з.set_valign(gtk::Align::Start);
        блок.add(&з);
        let т = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let г = ссылка(к.имя, true);
        let (п2, id) = (п.clone(), к.id);
        г.connect_clicked(move |_| иди(&п2, Вид::Категория(id)));
        т.add(&г);
        for пункт in ПУНКТЫ.iter().filter(|х| х.где == к.id && х.главный) {
            let л = ссылка(пункт.имя, false);
            let п2 = п.clone();
            л.connect_clicked(move |_| открой(&п2, пункт));
            т.add(&л);
        }
        блок.add(&т);
        с.add(&блок);
    }
    п.тело.add(&с);
}

pub fn запусти(доводы: Vec<String>) -> ! {
    подготовь("glower-control", "Панель управления");
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль");
    gtk::StyleContext::add_provider_for_screen(&gtk::gdk::Screen::default().expect("экран"), &стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_title("Панель управления");
    окно.set_icon_name(Some("preferences-system"));
    окно.set_default_size(1000, 680);
    окно.style_context().add_class("glower-app");
    crate::apps::строка_заголовка(&окно);
    окно.style_context().add_class("control");

    let всё = gtk::Box::new(gtk::Orientation::Vertical, 0);
    окно.add(&всё);
    let верх = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    верх.style_context().add_class("top");
    let з = gtk::Label::new(Some("Настройка параметров компьютера"));
    з.style_context().add_class("title");
    з.set_xalign(0.0);
    з.set_hexpand(true);
    верх.add(&з);
    верх.add(&gtk::Label::new(Some("Просмотр:")));
    let выбор = gtk::ComboBoxText::new();
    выбор.append(Some("категории"), &crate::i18n::tr("Категория"));
    выбор.append(Some("значки"), &crate::i18n::tr("Значки"));
    верх.add(&выбор);
    let поиск = gtk::SearchEntry::new();
    поиск.set_placeholder_text(Some("Поиск в панели управления"));
    поиск.set_width_chars(26);
    верх.add(&поиск);
    всё.add(&верх);
    let прокрутка = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    прокрутка.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    прокрутка.set_vexpand(true);
    let тело = gtk::Box::new(gtk::Orientation::Vertical, 6);
    тело.style_context().add_class("body");
    прокрутка.add(&тело);
    всё.add(&прокрутка);

    let первый = match доводы.first().map(String::as_str) {
        Some("значки" | "icons") => Вид::Значки,
        Some(к) if КАТЕГОРИИ.iter().any(|х| х.id == к) => Вид::Категория(КАТЕГОРИИ.iter().find(|х| х.id == к).map(|х| х.id).unwrap_or("система")),
        _ => Вид::Категории,
    };
    let п: П = Rc::new(Окно { окно: окно.clone(), тело, вид: RefCell::new(первый), выбор: выбор.clone() });
    {
        let п2 = п.clone();
        выбор.connect_changed(move |в| {
            let нужен = if в.active_id().as_deref() == Some("значки") { Вид::Значки } else { Вид::Категории };
            let сейчас = п2.вид.borrow().clone();
            let совпадает = matches!((&сейчас, &нужен), (Вид::Значки, Вид::Значки)) || (нужен == Вид::Категории && !matches!(сейчас, Вид::Значки));
            if !совпадает {
                иди(&п2, нужен);
            }
        });
        let п2 = п.clone();
        поиск.connect_search_changed(move |п_| {
            let т = п_.text().to_string();
            иди(&п2, if т.trim().is_empty() { Вид::Категории } else { Вид::Поиск(т) });
        });
    }
    окно.connect_key_press_event({
        let поиск = поиск.clone();
        move |_, е| {
            if е.state().contains(gtk::gdk::ModifierType::CONTROL_MASK) && crate::notepad::латинская(е) == "f" {
                поиск.grab_focus();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        }
    });
    окно.connect_delete_event(|_, _| {
        gtk::main_quit();
        glib::Propagation::Proceed
    });
    рисуй(&п);
    окно.show_all();
    gtk::main();
    std::process::exit(0);
}

#[cfg(test)]
mod проверки {
    use super::*;

    #[test]
    fn у_каждого_пункта_есть_категория_и_дело() {
        for п in ПУНКТЫ {
            assert!(КАТЕГОРИИ.iter().any(|к| к.id == п.где), "{}", п.имя);
            assert!(!п.дело.is_empty(), "{}", п.имя);
        }
        // В каждой категории есть что показать под заголовком
        for к in КАТЕГОРИИ {
            assert!(ПУНКТЫ.iter().any(|п| п.где == к.id && п.главный), "{}", к.имя);
        }
    }

    #[test]
    fn разделы_параметров_существуют() {
        let раздел_есть = |id: &str| crate::settings::раздел_есть(id);
        for п in ПУНКТЫ.iter().filter(|п| п.дело[0] == "glower-settings") {
            assert!(раздел_есть(п.дело[1]), "{} → {}", п.имя, п.дело[1]);
        }
    }

    #[test]
    fn поиск_по_словам() {
        assert!(найди("пароль").iter().any(|п| п.имя == "Учётные записи"));
        assert!(найди("звук").iter().any(|п| п.имя == "Звук"));
        assert!(найди("   ").is_empty());
    }
}
