//! Диспетчер задач — нативный, настоящим окном (glower-taskmgr).
//!
//! Процессы читает сам, из /proc: программе на этой же машине агент не
//! нужен. Программы человека и внутренности системы — врозь, как и было:
//! человек приходит закрыть зависшую программу, а не искать её среди
//! systemd и dbus. Сверху — загрузка процессора и памяти с историей.

use crate::apps::{кнопка, подготовь};
use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::time::{Duration, Instant};

const СТИЛЬ: &str = r#"
.taskmgr .cards { padding: 12px; }
.taskmgr .tm-card { background-color: alpha(@glower_fg, 0.05); border-radius: 12px; padding: 10px 14px; }
.taskmgr .tm-k { font-size: 12px; opacity: 0.6; }
.taskmgr .tm-v { font-size: 20px; font-weight: 600; }
.taskmgr treeview { background-color: @glower_bg2; }
.taskmgr treeview:selected { background-color: alpha(@glower_accent, 0.35); }
.taskmgr treeview header button { background-color: @glower_bg; border: none; border-bottom: 1px solid alpha(@glower_fg, 0.07); padding: 4px 8px; }
"#;

/// Наши части и службы системы: человеку они нужны редко
const СИСТЕМНОЕ: &[&str] = &[
    "glower-", "server.mjs", "labwc", "glower-wm", "Xwayland", "greetd", "systemd", "dbus", "polkit", "udisks", "upower",
    "NetworkManager", "wpa_supplicant", "ModemManager", "pipewire", "wireplumber", "avahi", "bluetoothd", "cups",
    "rsyslog", "cron", "agetty", "plymouth", "accounts-daemon", "gvfs", "at-spi", "xdg-desktop-portal", "swayidle", "wlsunset",
];

#[derive(Clone, Debug, PartialEq)]
pub struct Процесс {
    pub pid: i32,
    pub имя: String,
    pub строка: String,
    pub такты: u64,
    pub память: u64,
    pub свой: bool,
}

pub fn системный(имя: &str, строка: &str) -> bool {
    // Потоки ядра строки запуска не имеют вовсе
    if строка.is_empty() {
        return true;
    }
    let где = format!("{строка} {имя}");
    СИСТЕМНОЕ.iter().any(|с| где.contains(с))
}

/// Разобрать /proc/<pid>/stat: имя в скобках, такты и память
pub fn разбери_stat(stat: &str) -> Option<(String, u64, u64)> {
    let открыта = stat.find('(')?;
    let закрыта = stat.rfind(')')?;
    let имя = stat[открыта + 1..закрыта].to_string();
    let хвост: Vec<&str> = stat[закрыта + 2..].split(' ').collect();
    let utime: u64 = хвост.get(11)?.parse().ok()?;
    let stime: u64 = хвост.get(12)?.parse().ok()?;
    let rss: u64 = хвост.get(21)?.parse().ok()?;
    Some((имя, utime + stime, rss * 4096))
}

fn процессы() -> Vec<Процесс> {
    let мой = unsafe { getuid() };
    let mut итог = Vec::new();
    let Ok(чтение) = std::fs::read_dir("/proc") else { return итог };
    for з in чтение.filter_map(|з| з.ok()) {
        let Some(pid) = з.file_name().to_str().and_then(|с| с.parse::<i32>().ok()) else { continue };
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else { continue };
        let Some((имя, такты, память)) = разбери_stat(&stat) else { continue };
        let строка = std::fs::read(format!("/proc/{pid}/cmdline"))
            .map(|б| String::from_utf8_lossy(&б).replace('\0', " ").trim().to_string())
            .unwrap_or_default();
        use std::os::unix::fs::MetadataExt;
        let свой = std::fs::metadata(format!("/proc/{pid}")).map(|м| м.uid() == мой).unwrap_or(false);
        итог.push(Процесс { pid, имя, строка, такты, память, свой });
    }
    итог
}

extern "C" {
    fn getuid() -> u32;
    fn kill(pid: i32, sig: i32) -> i32;
}

/// Такты процессора всего: для доли загрузки
fn такты_всего() -> (u64, u64) {
    let т = std::fs::read_to_string("/proc/stat").unwrap_or_default();
    let ч: Vec<u64> = т.lines().next().unwrap_or("").split_whitespace().skip(1).filter_map(|с| с.parse().ok()).collect();
    let всего: u64 = ч.iter().sum();
    let простой = ч.get(3).copied().unwrap_or(0) + ч.get(4).copied().unwrap_or(0);
    (всего, простой)
}

fn память() -> (u64, u64) {
    let т = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let поле = |имя: &str| {
        т.lines().find(|с| с.starts_with(имя)).and_then(|с| с.split_whitespace().nth(1)).and_then(|ч| ч.parse::<u64>().ok()).unwrap_or(0) * 1024
    };
    (поле("MemTotal:"), поле("MemAvailable:"))
}

fn гб(б: u64) -> String {
    format!("{:.1} ГБ", б as f64 / 1073741824.0).replace('.', ",")
}

fn мб(б: u64) -> String {
    if б < 1048576 {
        format!("{} КБ", б / 1024)
    } else {
        format!("{:.1} МБ", б as f64 / 1048576.0).replace('.', ",")
    }
}

struct Диспетчер {
    окно: gtk::Window,
    модель: gtk::TreeStore,
    вид: gtk::TreeView,
    цп: gtk::Label,
    пам: gtk::Label,
    всего: gtk::Label,
    нагрузка: gtk::Label,
    история_цп: RefCell<VecDeque<f64>>,
    история_пам: RefCell<VecDeque<f64>>,
    графики: (gtk::DrawingArea, gtk::DrawingArea),
    прошлое: RefCell<HashMap<i32, u64>>,
    прошлое_всего: Cell<(u64, u64)>,
    когда: Cell<Option<Instant>>,
    раскрыто: Cell<bool>,
    ядер: f64,
}

type Д = Rc<Диспетчер>;

const PID: u32 = 0;
const ИМЯ: u32 = 1;
const ЦП: u32 = 2;
const ЦП_ЧИСЛО: u32 = 3;
const ПАМЯТЬ: u32 = 4;
const ПАМЯТЬ_ЧИСЛО: u32 = 5;
const ЖИРНО: u32 = 6;

fn обнови(д: &Д) {
    let сейчас = Instant::now();
    let прошло = д.когда.get().map(|к| сейчас.duration_since(к).as_secs_f64()).unwrap_or(0.0);
    д.когда.set(Some(сейчас));
    let список = процессы();
    let mut новое = HashMap::new();
    // Доля процессора — за время между замерами, а не за всю жизнь
    let доля = |п: &Процесс, прошлое: &HashMap<i32, u64>| -> f64 {
        match прошлое.get(&п.pid) {
            Some(было) if прошло > 0.2 => (((п.такты.saturating_sub(*было)) as f64 / 100.0) / прошло * 100.0 / д.ядер).clamp(0.0, 100.0),
            _ => 0.0,
        }
    };
    let прошлое = д.прошлое.borrow().clone();
    let mut свои: Vec<(Процесс, f64)> = Vec::new();
    let mut системные: Vec<(Процесс, f64)> = Vec::new();
    for п in список {
        новое.insert(п.pid, п.такты);
        let д_ = доля(&п, &прошлое);
        if п.свой && !системный(&п.имя, &п.строка) {
            свои.push((п, д_));
        } else {
            системные.push((п, д_));
        }
    }
    let всего_процессов = свои.len() + системные.len();
    *д.прошлое.borrow_mut() = новое;

    // Общая загрузка
    let (всего, простой) = такты_всего();
    let (было_всего, было_простой) = д.прошлое_всего.replace((всего, простой));
    let загрузка = if было_всего > 0 && всего > было_всего {
        (1.0 - (простой - было_простой) as f64 / (всего - было_всего) as f64).clamp(0.0, 1.0) * 100.0
    } else {
        0.0
    };
    let (всего_пам, свободно) = память();
    let занято = всего_пам.saturating_sub(свободно);
    д.цп.set_text(&format!("{загрузка:.0}%"));
    д.пам.set_text(&format!("{} из {}", гб(занято), гб(всего_пам)));
    д.всего.set_text(&всего_процессов.to_string());
    let la = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    д.нагрузка.set_text(&la.split_whitespace().take(3).collect::<Vec<_>>().join(" · ").replace('.', ","));
    for (история, v) in [(&д.история_цп, загрузка), (&д.история_пам, if всего_пам > 0 { занято as f64 / всего_пам as f64 * 100.0 } else { 0.0 })] {
        let mut и = история.borrow_mut();
        и.push_back(v);
        while и.len() > 60 {
            и.pop_front();
        }
    }
    д.графики.0.queue_draw();
    д.графики.1.queue_draw();

    // Список: выделение и раскрытие сохраняем
    let выбран = д.вид.selection().selected().map(|(м, и)| м.value(&и, PID as i32).get::<i32>().unwrap_or(0));
    let раскрыто = д.раскрыто.get();
    д.модель.clear();
    // Группы держатся своего порядка при любой сортировке: программы — первыми
    let группа = |имя: &str, сколько: usize, место: u64| {
        д.модель.insert_with_values(None, None, &[(PID, &0i32), (ИМЯ, &format!("{имя} · {сколько}")), (ЖИРНО, &700i32), (ЦП_ЧИСЛО, &(1e9 + место as f64)), (ПАМЯТЬ_ЧИСЛО, &(u64::MAX - 1 + место))])
    };
    let добавь = |родитель: &gtk::TreeIter, (п, доля): &(Процесс, f64)| {
        д.модель.insert_with_values(
            Some(родитель),
            None,
            &[
                (PID, &п.pid),
                (ИМЯ, &format!("{}   {}", п.имя, п.pid)),
                (ЦП, &format!("{доля:.1}%")),
                (ЦП_ЧИСЛО, &доля),
                (ПАМЯТЬ, &мб(п.память)),
                (ПАМЯТЬ_ЧИСЛО, &п.память),
                (ЖИРНО, &400i32),
            ],
        );
    };
    let п1 = группа("Программы", свои.len(), 1);
    for з in &свои {
        добавь(&п1, з);
    }
    let п2 = группа("Система и оболочка", системные.len(), 0);
    for з in &системные {
        добавь(&п2, з);
    }
    if let Some(путь) = д.модель.path(&п1) {
        д.вид.expand_row(&путь, false);
    }
    if раскрыто {
        if let Some(путь) = д.модель.path(&п2) {
            д.вид.expand_row(&путь, false);
        }
    }
    if let Some(pid) = выбран.filter(|п| *п > 0) {
        д.модель.foreach(|м, путь, и| {
            if м.value(и, PID as i32).get::<i32>().unwrap_or(0) == pid {
                д.вид.selection().select_path(путь);
                return true;
            }
            false
        });
    }
    д.окно.set_title(&format!("Диспетчер задач — {всего_процессов} процессов"));
}

fn сними(д: &Д, силой: bool) {
    let Some((м, и)) = д.вид.selection().selected() else { return };
    let pid = м.value(&и, PID as i32).get::<i32>().unwrap_or(0);
    if pid <= 0 {
        return;
    }
    let имя = м.value(&и, ИМЯ as i32).get::<String>().unwrap_or_default();
    let вопрос = gtk::MessageDialog::new(
        Some(&д.окно),
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Question,
        gtk::ButtonsType::None,
        if силой { "Завершить принудительно?" } else { "Снять задачу?" },
    );
    вопрос.set_secondary_text(Some(&format!(
        "«{}» {}. Несохранённое в нём может пропасть.",
        имя.split_whitespace().next().unwrap_or(&имя),
        if силой { "будет остановлен сразу, без спроса" } else { "получит просьбу закончить работу" }
    )));
    вопрос.add_button("Отмена", gtk::ResponseType::Cancel);
    вопрос.add_button(if силой { "Завершить" } else { "Снять" }, gtk::ResponseType::Accept);
    let ответ = вопрос.run();
    unsafe { вопрос.destroy() };
    if ответ != gtk::ResponseType::Accept {
        return;
    }
    if unsafe { kill(pid, if силой { 9 } else { 15 }) } != 0 {
        let е = std::io::Error::last_os_error();
        let д2 = gtk::MessageDialog::new(Some(&д.окно), gtk::DialogFlags::MODAL, gtk::MessageType::Warning, gtk::ButtonsType::Close, "Не вышло снять задачу");
        д2.set_secondary_text(Some(&е.to_string()));
        д2.run();
        unsafe { д2.destroy() };
    }
    let д3 = д.clone();
    glib::timeout_add_local_once(Duration::from_millis(600), move || обнови(&д3));
}

fn график(история: &RefCell<VecDeque<f64>>, к: &gtk::cairo::Context, ш: f64, в: f64, цвет: (f64, f64, f64)) {
    let и = история.borrow();
    к.set_source_rgba(1.0, 1.0, 1.0, 0.04);
    к.rectangle(0.0, 0.0, ш, в);
    let _ = к.fill();
    if и.len() < 2 {
        return;
    }
    let шаг = ш / 59.0;
    let старт = ш - шаг * (и.len() - 1) as f64;
    к.move_to(старт, в);
    for (i, v) in и.iter().enumerate() {
        к.line_to(старт + шаг * i as f64, в - в * v / 100.0);
    }
    к.line_to(ш, в);
    к.close_path();
    к.set_source_rgba(цвет.0, цвет.1, цвет.2, 0.25);
    let _ = к.fill_preserve();
    к.new_path();
    for (i, v) in и.iter().enumerate() {
        let x = старт + шаг * i as f64;
        let y = в - в * v / 100.0;
        if i == 0 { к.move_to(x, y) } else { к.line_to(x, y) }
    }
    к.set_source_rgb(цвет.0, цвет.1, цвет.2);
    к.set_line_width(1.5);
    let _ = к.stroke();
}

pub fn запусти() -> ! {
    подготовь("glower-taskmgr", "Диспетчер задач");
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(СТИЛЬ.as_bytes()).expect("стиль диспетчера");
    gtk::StyleContext::add_provider_for_screen(&gtk::gdk::Screen::default().expect("экран"), &стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_default_size(760, 600);
    окно.set_icon_name(Some("utilities-system-monitor"));
    окно.style_context().add_class("glower-app");
    окно.style_context().add_class("taskmgr");
    let колонка = gtk::Box::new(gtk::Orientation::Vertical, 0);
    окно.add(&колонка);

    let карточки = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    карточки.style_context().add_class("cards");
    карточки.set_homogeneous(true);
    let карточка = |заголовок: &str, с_графиком: bool| -> (gtk::Label, Option<gtk::DrawingArea>) {
        let к = gtk::Box::new(gtk::Orientation::Vertical, 4);
        к.style_context().add_class("tm-card");
        let з = gtk::Label::new(Some(заголовок));
        з.set_xalign(0.0);
        з.style_context().add_class("tm-k");
        к.add(&з);
        let в = gtk::Label::new(Some("—"));
        в.set_xalign(0.0);
        в.style_context().add_class("tm-v");
        к.add(&в);
        let г = с_графиком.then(|| {
            let г = gtk::DrawingArea::new();
            г.set_size_request(-1, 46);
            к.add(&г);
            г
        });
        карточки.add(&к);
        (в, г)
    };
    let (цп, гц) = карточка("Процессор", true);
    let (пам, гп) = карточка("Память", true);
    let (всего, _) = карточка("Процессов", false);
    let (нагрузка, _) = карточка("Средняя нагрузка · 1, 5, 15 мин", false);
    колонка.add(&карточки);

    let модель = gtk::TreeStore::new(&[i32::static_type(), String::static_type(), String::static_type(), f64::static_type(), String::static_type(), u64::static_type(), i32::static_type()]);
    let вид = gtk::TreeView::with_model(&модель);
    let столбец = |заголовок: &str, данные: u32, сорт: u32, растянуть: bool, справа: bool| {
        let с = gtk::TreeViewColumn::new();
        с.set_title(заголовок);
        с.set_resizable(true);
        с.set_expand(растянуть);
        с.set_sort_column_id(сорт as i32);
        let т = gtk::CellRendererText::new();
        if справа {
            т.set_xalign(1.0);
        }
        TreeViewColumnExt::pack_start(&с, &т, true);
        TreeViewColumnExt::add_attribute(&с, &т, "text", данные as i32);
        TreeViewColumnExt::add_attribute(&с, &т, "weight", ЖИРНО as i32);
        вид.append_column(&с);
    };
    столбец("Процесс", ИМЯ, ИМЯ, true, false);
    столбец("ЦП", ЦП, ЦП_ЧИСЛО, false, true);
    столбец("Память", ПАМЯТЬ, ПАМЯТЬ_ЧИСЛО, false, true);
    модель.set_sort_column_id(gtk::SortColumn::Index(ЦП_ЧИСЛО), gtk::SortType::Descending);
    let прокрутка = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    прокрутка.set_vexpand(true);
    прокрутка.add(&вид);
    колонка.add(&прокрутка);

    let низ = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    низ.style_context().add_class("toolbar");
    let пустота = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    пустота.set_hexpand(true);
    низ.add(&пустота);
    let к_сила = кнопка("Завершить принудительно", "Остановить сразу (Shift+Delete)");
    let к_снять = кнопка("Снять задачу", "Попросить закончить работу (Delete)");
    к_снять.style_context().add_class("accent");
    низ.add(&к_сила);
    низ.add(&к_снять);
    колонка.add(&низ);

    let ядер = std::thread::available_parallelism().map(|н| н.get()).unwrap_or(1) as f64;
    let д: Д = Rc::new(Диспетчер {
        окно: окно.clone(),
        модель,
        вид: вид.clone(),
        цп,
        пам,
        всего,
        нагрузка,
        история_цп: RefCell::new(VecDeque::new()),
        история_пам: RefCell::new(VecDeque::new()),
        графики: (гц.unwrap(), гп.unwrap()),
        прошлое: RefCell::new(HashMap::new()),
        прошлое_всего: Cell::new((0, 0)),
        когда: Cell::new(None),
        раскрыто: Cell::new(false),
        ядер,
    });
    {
        let д1 = д.clone();
        д.графики.0.connect_draw(move |г, к| {
            график(&д1.история_цп, к, г.allocated_width() as f64, г.allocated_height() as f64, (0.36, 0.55, 1.0));
            glib::Propagation::Stop
        });
        let д1 = д.clone();
        д.графики.1.connect_draw(move |г, к| {
            график(&д1.история_пам, к, г.allocated_width() as f64, г.allocated_height() as f64, (0.55, 0.36, 0.96));
            glib::Propagation::Stop
        });
        // Раскрыли «Систему» — помним до конца
        let д1 = д.clone();
        вид.connect_row_expanded(move |_, _, путь| {
            if путь.indices().first() == Some(&1) {
                д1.раскрыто.set(true);
            }
        });
        let д1 = д.clone();
        вид.connect_row_collapsed(move |_, _, путь| {
            if путь.indices().first() == Some(&1) {
                д1.раскрыто.set(false);
            }
        });
        let д1 = д.clone();
        к_снять.connect_clicked(move |_| сними(&д1, false));
        let д1 = д.clone();
        к_сила.connect_clicked(move |_| сними(&д1, true));
        let д1 = д.clone();
        окно.connect_key_press_event(move |_, е| {
            use gtk::gdk::keys::constants as к;
            if е.keyval() == к::Delete {
                сними(&д1, е.state().contains(gtk::gdk::ModifierType::SHIFT_MASK));
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        let д1 = Rc::downgrade(&д);
        glib::timeout_add_local(Duration::from_secs(2), move || match д1.upgrade() {
            Some(д) => {
                обнови(&д);
                glib::ControlFlow::Continue
            }
            None => glib::ControlFlow::Break,
        });
    }
    окно.connect_delete_event(|_, _| {
        gtk::main_quit();
        glib::Propagation::Proceed
    });
    обнови(&д);
    // Второй замер — через полсекунды: первому не с чем сравнить
    let д1 = д.clone();
    glib::timeout_add_local_once(Duration::from_millis(500), move || обнови(&д1));
    окно.show_all();
    gtk::main();
    std::process::exit(0);
}

#[cfg(test)]
mod проверки {
    use super::*;

    #[test]
    fn разбирает_stat_со_скобками_в_имени() {
        let stat = "1234 (Web Content (x)) S 1 2 3 4 5 6 7 8 9 10 120 30 0 0 20 0 1 0 500 0 2560 18446744073709551615";
        let (имя, такты, память) = разбери_stat(stat).unwrap();
        assert_eq!(имя, "Web Content (x)");
        assert_eq!(такты, 150);
        assert_eq!(память, 2560 * 4096);
    }

    #[test]
    fn отличает_программы_от_системы() {
        assert!(системный("kworker/0:1", ""));
        assert!(системный("node", "node agent/server.mjs --port 8123"));
        assert!(!системный("telegram-deskto", "/app/bin/telegram-desktop"));
    }

    #[test]
    fn видит_себя_в_списке() {
        let мой = std::process::id() as i32;
        assert!(процессы().iter().any(|п| п.pid == мой));
    }
}
