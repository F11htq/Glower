//! Меню программ во весь экран — нативное, в той же программе, что панель.
//!
//! Прежде меню рисовала страница рабочего стола. Здесь — то же меню:
//! рабочие столы наверху, поиск, открытые окна системы и все программы
//! разом. Что лежит на столах и в окнах, знает только страница: мы
//! спрашиваем её по шине («обзор-дай») и ей же отдаём то, что сделал
//! человек («обзор-дело»). Список программ стол и так рассказывает всем
//! («программы»).

use crate::agent::скажи;
use crate::popup;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::rc::Rc;

struct Меню {
    окно: gtk::Window,
    поиск: gtk::Entry,
    столы: gtk::Box,
    окна: gtk::Box,
    сетка: gtk::FlowBox,
    листание: gtk::ScrolledWindow,
    /// Просмотр задач (Win+Tab): только столы и крупные карточки окон
    карточки: gtk::FlowBox,
    задачи: bool,
    программы: Vec<Value>,
    слепок_программ: String,
    данные: Value,
}

thread_local! {
    static МЕНЮ: RefCell<Option<Rc<RefCell<Меню>>>> = const { RefCell::new(None) };
    static СПИСОК: RefCell<Vec<Value>> = const { RefCell::new(Vec::new()) };
    static ДАННЫЕ: RefCell<Value> = const { RefCell::new(Value::Null) };
    /// Насколько меню проявилось: 0 — нет, 1 — целиком. Фон и содержимое
    /// рисуются с этой долей, пока идёт появление или уход
    static ПОКАЗ: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
    /// Куда идёт показ: true — появляется, false — уходит. Нажали снова
    /// посреди анимации — она просто разворачивается с того же места
    static КУДА: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ИДЁТ: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Длительности: появление чуть дольше ухода — так меню кажется лёгким
const ПОЯВЛЕНИЕ_МС: f64 = 200.0;
const УХОД_МС: f64 = 140.0;
/// На сколько точек сетка съезжает сверху, пока проявляется
const СДВИГ: f64 = 26.0;

fn плавно(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

/// Применить долю показа: прозрачность содержимого, сдвиг и фон
fn примени_показ(м: &Меню, п: f64) {
    if let Some(колонка) = м.окно.child().and_then(|ф| ф.downcast::<gtk::Bin>().ok()).and_then(|ф| ф.child()) {
        колонка.set_opacity(п);
        колонка.set_margin_top(52 + (СДВИГ * (1.0 - плавно(п))) as i32);
    }
    м.окно.queue_draw();
}

/// Повести показ к открытому или закрытому виду
fn веди_показ(м: &Rc<RefCell<Меню>>, открыть: bool) {
    КУДА.with(|к| к.set(открыть));
    if std::env::var_os("GLOWER_WM_NO_ANIM").is_some() {
        ПОКАЗ.with(|п| п.set(if открыть { 1.0 } else { 0.0 }));
        примени_показ(&м.borrow(), if открыть { 1.0 } else { 0.0 });
        if !открыть {
            м.borrow().окно.hide();
        }
        return;
    }
    if ИДЁТ.with(|и| и.replace(true)) {
        return;
    }
    let окно = м.borrow().окно.clone();
    let м = м.clone();
    let прошлое = Rc::new(std::cell::Cell::new(0i64));
    окно.add_tick_callback(move |_, часы| {
        let сейчас = часы.frame_time();
        let было = прошлое.replace(сейчас);
        let шаг = if было == 0 { 16.0 } else { ((сейчас - было) as f64 / 1000.0).clamp(1.0, 50.0) };
        let открыть = КУДА.with(|к| к.get());
        let п = ПОКАЗ.with(|п| {
            let н = if открыть { (п.get() + шаг / ПОЯВЛЕНИЕ_МС).min(1.0) } else { (п.get() - шаг / УХОД_МС).max(0.0) };
            п.set(н);
            н
        });
        примени_показ(&м.borrow(), п);
        let конец = (открыть && п >= 1.0) || (!открыть && п <= 0.0);
        if конец {
            ИДЁТ.with(|и| и.set(false));
            if !открыть {
                м.borrow().окно.hide();
            }
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    });
}

/// Стол рассказал о своих программах.
pub fn программы(список: &Value) {
    СПИСОК.with(|с| *с.borrow_mut() = список.as_array().cloned().unwrap_or_default());
    if let Some(м) = меню_если_есть() {
        if м.borrow().окно.is_visible() {
            наполни_сетку(&м);
        }
    }
}

/// Стол ответил, что у него на столах и в окнах.
pub fn данные(что: &Value) {
    ДАННЫЕ.with(|д| *д.borrow_mut() = что.clone());
    if let Some(м) = меню_если_есть() {
        м.borrow_mut().данные = что.clone();
        if м.borrow().окно.is_visible() {
            наполни_столы(&м);
        }
    }
}

fn меню_если_есть() -> Option<Rc<RefCell<Меню>>> {
    МЕНЮ.with(|м| м.borrow().clone())
}

/// Открыто — и не уходит: уходящее меню второе нажатие должно вернуть
pub fn открыто() -> bool {
    меню_если_есть().is_some_and(|м| м.borrow().окно.is_visible()) && КУДА.with(|к| к.get())
}

pub fn закрой() {
    if let Some(м) = меню_если_есть() {
        if м.borrow().окно.is_visible() {
            веди_показ(&м, false);
        }
    }
}

/// «Переключи» — от клавиши Win и кнопки «Программы»: второе нажатие
/// закрывает. true / false — открыть или закрыть явно. «задачи» — Win+Tab:
/// тот же обзор, но без программ, с окнами во всю ширину
pub fn переключи(что: &Value) {
    let задачи = что.as_str() == Some("задачи");
    let было_задачи = меню_если_есть().is_some_and(|м| м.borrow().задачи);
    let открыть = match что {
        Value::Bool(б) => *б,
        // Из меню в задачи и обратно — переключаем вид, а не закрываем
        _ if открыто() && задачи != было_задачи => true,
        _ => !открыто(),
    };
    if !открыть {
        закрой();
        return;
    }
    popup::закрой();
    let м = меню_если_есть().unwrap_or_else(создай);
    м.borrow_mut().задачи = задачи;
    {
        let м_ = м.borrow();
        м_.поиск.set_text("");
        м_.листание.vadjustment().set_value(0.0);
    }
    м.borrow_mut().данные = ДАННЫЕ.with(|д| д.borrow().clone());
    наполни_сетку(&м);
    наполни_столы(&м);
    // Свежее о столах и окнах — у стола: ответ придёт следом и дорисует
    скажи("обзор-дай", json!(true));
    let окно = м.borrow().окно.clone();
    let уже_видно = окно.is_visible();
    if !уже_видно {
        ПОКАЗ.with(|п| п.set(0.0));
        примени_показ(&м.borrow(), 0.0);
    }
    окно.show_all();
    веди_показ(&м, true);
    let м_ = м.borrow();
    м_.поиск.set_visible(!задачи);
    м_.окна.set_visible(!задачи);
    м_.листание.set_visible(!задачи);
    м_.карточки.set_visible(задачи);
    if задачи {
        окно.grab_focus();
    } else {
        м_.поиск.grab_focus();
    }
}

fn дело(что: &str, доводы: Value) {
    let mut тело = доводы;
    тело["что"] = json!(что);
    скажи("обзор-дело", тело);
}

// ------------------------------------------------------------- окно меню

fn создай() -> Rc<RefCell<Меню>> {
    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_decorated(false);
    окно.set_app_paintable(true);
    if let Some(в) = gtk::prelude::GtkWindowExt::screen(&окно).and_then(|э| э.rgba_visual()) {
        окно.set_visual(Some(&в));
    }
    окно.style_context().add_class("overview");
    окно.init_layer_shell();
    // Поверх всего, вместе с панелью — как Launchpad в macOS: меню
    // занимает экран целиком, и ничто не спорит с ним за внимание.
    // Закрывают его Win, Escape или щелчок по пустому месту.
    окно.set_layer(Layer::Overlay);
    окно.set_namespace("glower-overview");
    for край in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        окно.set_anchor(край, true);
    }
    // −1: не уступать места ничьей полосе, в том числе нашей же панели
    окно.set_exclusive_zone(-1);
    окно.set_keyboard_mode(KeyboardMode::Exclusive);
    // Фон заливаем сами. Окно объявлено самоотрисовываемым (иначе нет
    // прозрачности), и GTK его фон не рисует вовсе: меню выходило
    // совсем прозрачным, и значки терялись на пёстрых обоях
    // Под glower-wm фон за меню размыт, и плотная заливка его бы спрятала —
    // поэтому там она прозрачнее. Доля показа — от анимации появления
    let плотность = if std::env::var_os("GLOWER_WM_BLUR").is_some() { 0.55 } else { 0.86 };
    окно.connect_draw(move |_, к| {
        // Прозрачность выключена в «Персонализации» — фон плотный
        let плотность = if crate::theme::прозрачно() { плотность } else { 0.97 };
        к.set_operator(gtk::cairo::Operator::Source);
        к.set_source_rgba(0.04, 0.047, 0.08, плотность * ПОКАЗ.with(|п| п.get()));
        let _ = к.paint();
        glib::Propagation::Proceed
    });
    окно.connect_key_press_event(|_, с| {
        if с.keyval() == gtk::gdk::keys::constants::Escape {
            закрой();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });

    // Щелчок по пустому месту закрывает — как в GNOME
    let фон = gtk::EventBox::new();
    фон.set_visible_window(false);
    фон.connect_button_press_event(|_, с| {
        if с.button() == 1 {
            закрой();
        }
        glib::Propagation::Stop
    });

    let колонка = gtk::Box::new(gtk::Orientation::Vertical, 16);
    колонка.set_margin_top(52);
    колонка.set_margin_bottom(18);
    колонка.set_margin_start(40);
    колонка.set_margin_end(40);

    let столы = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    столы.set_halign(gtk::Align::Center);

    let поиск = gtk::Entry::new();
    поиск.set_placeholder_text(Some("Поиск программ и файлов"));
    поиск.set_width_chars(36);
    поиск.set_halign(gtk::Align::Center);
    поиск.style_context().add_class("search");

    let окна = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    окна.set_halign(gtk::Align::Center);

    let сетка = gtk::FlowBox::new();
    сетка.set_selection_mode(gtk::SelectionMode::None);
    сетка.set_homogeneous(true);
    сетка.set_max_children_per_line(9);
    сетка.set_min_children_per_line(3);
    сетка.set_row_spacing(6);
    сетка.set_column_spacing(6);
    сетка.set_valign(gtk::Align::Start);
    сетка.set_halign(gtk::Align::Center);

    let листание = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    листание.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    листание.set_vexpand(true);
    листание.add(&сетка);

    let карточки = gtk::FlowBox::new();
    карточки.set_selection_mode(gtk::SelectionMode::None);
    карточки.set_homogeneous(true);
    карточки.set_max_children_per_line(4);
    карточки.set_min_children_per_line(1);
    карточки.set_row_spacing(18);
    карточки.set_column_spacing(18);
    карточки.set_valign(gtk::Align::Center);
    карточки.set_halign(gtk::Align::Center);
    карточки.set_vexpand(true);

    колонка.pack_start(&столы, false, false, 0);
    колонка.pack_start(&поиск, false, false, 0);
    // Найденные файлы — над программами, как «Документы» в поиске Windows
    let файлы = gtk::ListBox::new();
    файлы.set_selection_mode(gtk::SelectionMode::None);
    файлы.style_context().add_class("files-found");
    файлы.set_halign(gtk::Align::Center);
    файлы.set_size_request(560, -1);
    файлы.set_no_show_all(true);
    // Нажатие забираем себе: иначе оно уходит фону меню, а тот закрывает
    // меню раньше, чем строка успевает открыть файл
    файлы.connect_button_press_event(|с, е| {
        if е.button() == 1 {
            if let Some(р) = с.row_at_y(е.position().1 as i32) {
                р.activate();
            }
        }
        glib::Propagation::Stop
    });
    колонка.pack_start(&файлы, false, false, 0);
    колонка.pack_start(&окна, false, false, 0);
    колонка.pack_start(&листание, true, true, 0);
    колонка.pack_start(&карточки, true, true, 0);
    фон.add(&колонка);
    окно.add(&фон);

    // Поиск: фильтр по имени, Ввод открывает первое найденное
    let строка = Rc::new(RefCell::new(String::new()));
    {
        let строка = строка.clone();
        сетка.set_filter_func(Some(Box::new(move |ребёнок| {
            let что = строка.borrow();
            что.is_empty() || ребёнок.widget_name().as_str().contains(что.as_str())
        })));
    }
    {
        let (строка, сетка) = (строка.clone(), сетка.clone());
        let файлы = файлы.clone();
        поиск.connect_changed(move |п| {
            *строка.borrow_mut() = п.text().trim().to_lowercase();
            сетка.invalidate_filter();
            ищи_файлы(&файлы, п.text().trim());
        });
    }
    {
        let (сетка, файлы) = (сетка.clone(), файлы.clone());
        поиск.connect_activate(move |_| {
            // Программ не нашлось — Ввод открывает первый найденный файл
            let есть_программы = сетка
                .children()
                .into_iter()
                .filter_map(|р| р.downcast::<gtk::FlowBoxChild>().ok())
                .any(|р| р.is_child_visible());
            if !есть_программы {
                if let Some(р) = файлы.row_at_index(0) {
                    р.activate();
                    return;
                }
            }
            let первый = сетка
                .children()
                .into_iter()
                .filter_map(|р| р.downcast::<gtk::FlowBoxChild>().ok())
                // Отсеянного поиском GTK не прячет, а лишь снимает с показа
                .find(|р| р.is_child_visible());
            if let Some(р) = первый {
                if let Some(б) = р.child().and_then(|б| б.downcast::<gtk::Button>().ok()) {
                    б.clicked();
                }
            }
        });
    }

    let м = Rc::new(RefCell::new(Меню {
        окно,
        поиск,
        столы,
        окна,
        сетка,
        листание,
        карточки,
        задачи: false,
        программы: Vec::new(),
        слепок_программ: String::new(),
        данные: Value::Null,
    }));
    МЕНЮ.with(|я| *я.borrow_mut() = Some(м.clone()));
    м
}


// ------------------------------------------------------------- поиск файлов

thread_local! {
    /// Номер поиска: ответ старого поиска, пришедший после нового, — мимо
    static ПОИСК_ФАЙЛОВ: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Найти файлы по имени в папках человека. Обходим в отдельном потоке и с
/// пределом — меню не должно замирать даже на огромной домашней папке
fn найди_файлы(что: &str, сколько: usize) -> Vec<std::path::PathBuf> {
    let что = что.to_lowercase();
    let mut корни = vec![crate::core::корень()];
    let дом = glib::home_dir();
    if !корни.contains(&дом) {
        корни.push(дом);
    }
    let mut найдено: Vec<(usize, std::path::PathBuf)> = Vec::new();
    let mut очередь: std::collections::VecDeque<(std::path::PathBuf, usize)> = корни.into_iter().map(|к| (к, 0)).collect();
    let mut просмотрено = 0usize;
    let mut виденные = std::collections::HashSet::new();
    while let Some((папка, глубина)) = очередь.pop_front() {
        if !виденные.insert(папка.clone()) {
            continue;
        }
        let Ok(чтение) = std::fs::read_dir(&папка) else { continue };
        for з in чтение.filter_map(|з| з.ok()) {
            просмотрено += 1;
            if просмотрено > 40_000 {
                break;
            }
            let имя = з.file_name().to_string_lossy().to_string();
            // Скрытое и служебное не ищем: там тысячи файлов программ
            if имя.starts_with('.') || имя == "node_modules" || имя == "snap" {
                continue;
            }
            let путь = з.path();
            let папка_ли = з.file_type().map(|т| т.is_dir()).unwrap_or(false);
            let малое = имя.to_lowercase();
            if let Some(где) = малое.find(&что) {
                // Совпало с начала имени — выше
                найдено.push((if где == 0 { 0 } else { 1 } + глубина, путь.clone()));
            }
            if папка_ли && глубина < 8 {
                очередь.push_back((путь, глубина + 1));
            }
        }
    }
    найдено.sort_by(|а, б| а.0.cmp(&б.0).then_with(|| а.1.cmp(&б.1)));
    найдено.into_iter().take(сколько).map(|(_, п)| п).collect()
}

fn ищи_файлы(список: &gtk::ListBox, что: &str) {
    let номер = ПОИСК_ФАЙЛОВ.with(|н| {
        н.set(н.get() + 1);
        н.get()
    });
    if что.chars().count() < 2 {
        for р in список.children() {
            список.remove(&р);
        }
        список.hide();
        return;
    }
    let что = что.to_string();
    let список = список.clone();
    // Пауза — пока печатают, не обходить диск на каждую букву
    glib::timeout_add_local_once(std::time::Duration::from_millis(220), move || {
        if ПОИСК_ФАЙЛОВ.with(|н| н.get()) != номер {
            return;
        }
        let (куда, откуда) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let _ = куда.send_blocking(найди_файлы(&что, 6));
        });
        glib::MainContext::default().spawn_local(async move {
            let Ok(пути) = откуда.recv().await else { return };
            if ПОИСК_ФАЙЛОВ.with(|н| н.get()) != номер {
                return;
            }
            for р in список.children() {
                список.remove(&р);
            }
            for п in пути {
                // Строки показываем сами: у списка стоит no_show_all, и его
                // show_all не трогает детей
                let р = строка_файла(&п);
                р.show_all();
                список.add(&р);
            }
            список.set_visible(!список.children().is_empty());
        });
    });
}

fn строка_файла(путь: &std::path::Path) -> gtk::ListBoxRow {
    let р = gtk::ListBoxRow::new();
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    ряд.set_margin_start(10);
    ряд.set_margin_end(10);
    ряд.set_margin_top(5);
    ряд.set_margin_bottom(5);
    let значок = if путь.is_dir() {
        gio::ThemedIcon::new("folder").upcast::<gio::Icon>()
    } else {
        gio::content_type_get_icon(&crate::core::тип_по_имени(&путь.to_string_lossy()))
    };
    let к = gtk::Image::from_gicon(&значок, gtk::IconSize::LargeToolbar);
    к.set_pixel_size(24);
    ряд.add(&к);
    let столбик = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let имя = gtk::Label::new(путь.file_name().map(|и| и.to_string_lossy().to_string()).as_deref());
    имя.set_xalign(0.0);
    имя.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    имя.set_max_width_chars(50);
    имя.style_context().add_class("no-i18n");
    let где = путь.parent().map(|п| п.to_string_lossy().replace(&glib::home_dir().to_string_lossy().to_string(), "~")).unwrap_or_default();
    let папка = gtk::Label::new(Some(&где));
    папка.set_xalign(0.0);
    папка.set_ellipsize(gtk::pango::EllipsizeMode::Start);
    папка.set_max_width_chars(60);
    папка.set_width_chars(60);
    папка.style_context().add_class("dim");
    папка.style_context().add_class("no-i18n");
    столбик.add(&имя);
    столбик.add(&папка);
    ряд.pack_start(&столбик, true, true, 0);
    р.add(&ряд);
    let путь = путь.to_path_buf();
    р.connect_activate(move |_| {
        закрой();
        crate::core::открой_путь(&путь);
    });
    р
}

// ------------------------------------------------------------- значки

/// Значок нашей программы: знак на цветной плашке — тот же, что у
/// страницы. Фон приходит готовой строкой CSS, её и отдаём GTK.
pub(crate) fn наш_значок(знак: &str, фон: &str) -> gtk::Widget {
    // Белый значок темы на цветной плашке — как у программ в Windows 11 и
    // macOS. Раньше на плашке был эмодзи, и каждый шрифт рисовал его
    // по-своему. Размер значка — по размеру плашки: в доке, в меню
    // программ и мелкий на карточке окна
    let з: gtk::Widget = match crate::apps::имя_значка(if знак.is_empty() { "▢" } else { знак }) {
        Some(имя) => {
            let к = gtk::Image::from_icon_name(Some(имя), gtk::IconSize::Dialog);
            к.set_pixel_size(28);
            к.connect_realize(|к| {
                let мелкий = к.style_context().has_class("small");
                let в_доке = к.toplevel().is_some_and(|в| в.style_context().has_class("dock"));
                к.set_pixel_size(if мелкий { 14 } else if в_доке { 22 } else { 28 });
            });
            к.upcast()
        }
        None => gtk::Label::new(Some(знак)).upcast(),
    };
    з.style_context().add_class("glyph");
    let стиль = gtk::CssProvider::new();
    let css = if фон.trim_start().starts_with("linear-gradient") || фон.trim_start().starts_with("radial-gradient") {
        format!("* {{ background-image: {фон}; }}")
    } else if !фон.is_empty() {
        format!("* {{ background-color: {фон}; }}")
    } else {
        String::new()
    };
    if !css.is_empty() && стиль.load_from_data(css.as_bytes()).is_ok() {
        з.style_context().add_provider(&стиль, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
    }
    з.upcast()
}

/// Значок программы машины — её собственный, из темы значков системы.
fn значок_машины(id: &str, знак: &str) -> gtk::Widget {
    if let Some(ярлык) = gio::DesktopAppInfo::new(id) {
        if let Some(з) = ярлык.icon() {
            let к = gtk::Image::from_gicon(&з, gtk::IconSize::Dialog);
            к.set_pixel_size(52);
            return к.upcast();
        }
    }
    наш_значок(if знак.is_empty() { "🐧" } else { знак }, "rgba(255,255,255,0.12)")
}

// ------------------------------------------------------------- перетаскивание
//
// Программу из сетки можно бросить на рабочий стол наверху — она там и
// откроется; на «+» — заведётся новый стол, и откроется на нём. Открытое
// окно переезжает на другой стол так же. Так это устроено в GNOME, и так
// было в страничном меню: открыть программу не там, где стоишь, — одно
// движение вместо трёх (уйти на стол, открыть, вернуться).

const ГРУЗ: &str = "application/x-glower-drag";

fn цель() -> gtk::TargetEntry {
    // Только внутри своей программы: чужим окнам наш груз ни к чему
    gtk::TargetEntry::new(ГРУЗ, gtk::TargetFlags::SAME_APP, 0)
}

/// Сделать виджет источником груза. `картинка` — что тянется за курсором:
/// значок программы, а не безликий значок GTK.
fn тащи(кто: &gtk::Button, груз: Value, картинка: &gtk::Widget) {
    кто.drag_source_set(gtk::gdk::ModifierType::BUTTON1_MASK, &[цель()], gtk::gdk::DragAction::MOVE);
    let картинка = картинка.clone();
    кто.connect_drag_begin(move |_, ход| {
        let (ш, в) = (картинка.allocated_width(), картинка.allocated_height());
        if ш > 1 && в > 1 {
            if let Ok(холст) = gtk::cairo::ImageSurface::create(gtk::cairo::Format::ARgb32, ш, в) {
                if let Ok(кисть) = gtk::cairo::Context::new(&холст) {
                    картинка.draw(&кисть);
                }
                холст.set_device_offset(-(ш as f64) / 2.0, -(в as f64) / 2.0);
                ход.drag_set_icon_surface(&холст);
            }
        }
    });
    let строка = груз.to_string();
    кто.connect_drag_data_get(move |_, _, данные, _, _| {
        данные.set(&данные.target(), 8, строка.as_bytes());
    });
}

/// Сделать виджет местом, куда бросают. `дело` получает груз.
fn прими<F: Fn(Value) + 'static>(куда: &gtk::Button, дело_: F) {
    куда.drag_dest_set(gtk::DestDefaults::ALL, &[цель()], gtk::gdk::DragAction::MOVE);
    // Стол под курсором подсвечиваем — видно, куда упадёт
    куда.connect_drag_motion(|к, _, _, _, _| {
        к.style_context().add_class("drop");
        false
    });
    куда.connect_drag_leave(|к, _, _| к.style_context().remove_class("drop"));
    куда.connect_drag_data_received(move |к, _, _, _, данные, _, _| {
        к.style_context().remove_class("drop");
        if let Ok(груз) = serde_json::from_slice::<Value>(&данные.data()) {
            дело_(груз);
        }
    });
}

/// Что сделать, когда груз упал на стол `n` (None — на «+», новый стол).
fn брошено(груз: Value, n: Option<u64>) {
    if let Some(окно) = груз["окно"].as_str() {
        // Окно переезжает, меню остаётся: человек видит, что оно уехало
        match n {
            Some(n) => дело("окно-на-стол", json!({ "id": окно, "n": n })),
            None => дело("окно-на-новый-стол", json!({ "id": окно })),
        }
        return;
    }
    let (id, вид, имя) = (груз["id"].clone(), груз["вид"].clone(), груз["имя"].clone());
    закрой();
    match n {
        Some(n) => дело("на-стол", json!({ "id": id, "вид": вид, "имя": имя, "n": n })),
        None => дело("на-новый-стол", json!({ "id": id, "вид": вид, "имя": имя })),
    }
}

// ------------------------------------------------------------- сетка

fn наполни_сетку(м: &Rc<RefCell<Меню>>) {
    let список = СПИСОК.with(|с| с.borrow().clone());
    let слепок = serde_json::to_string(&список).unwrap_or_default();
    if м.borrow().слепок_программ == слепок {
        м.borrow().сетка.invalidate_filter();
        return;
    }
    let сетка = м.borrow().сетка.clone();
    for р in сетка.children() {
        сетка.remove(&р);
    }
    for п in &список {
        let id = п["id"].as_str().unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let имя = п["имя"].as_str().unwrap_or(&id).to_string();
        let вид = п["вид"].as_str().unwrap_or("приложение").to_string();
        let знак = п["знак"].as_str().unwrap_or("");
        let фон = п["фон"].as_str().unwrap_or("");

        let б = gtk::Button::new();
        б.style_context().add_class("app");
        let ящик = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let значок = if вид == "машина" { значок_машины(&id, знак) } else { наш_значок(знак, фон) };
        значок.set_halign(gtk::Align::Center);
        ящик.add(&значок);
        let подпись = gtk::Label::new(Some(&имя));
        подпись.style_context().add_class("app-name");
        подпись.set_max_width_chars(12);
        подпись.set_ellipsize(gtk::pango::EllipsizeMode::End);
        подпись.set_justify(gtk::Justification::Center);
        ящик.add(&подпись);
        б.add(&ящик);
        б.set_tooltip_text(Some(&имя));
        тащи(&б, json!({ "id": id, "вид": вид, "имя": имя }), &значок);

        {
            let (id, вид) = (id.clone(), вид.clone());
            б.connect_clicked(move |_| {
                закрой();
                дело("запусти", json!({ "id": id, "вид": вид }));
            });
        }
        {
            let (id, вид, имя) = (id.clone(), вид.clone(), имя.clone());
            б.connect_button_press_event(move |к, с| {
                if с.button() == 3 {
                    меню_программы(к, &id, &вид, &имя, с);
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            });
        }

        let ребёнок = gtk::FlowBoxChild::new();
        ребёнок.set_widget_name(&имя.to_lowercase());
        ребёнок.add(&б);
        сетка.add(&ребёнок);
    }
    if список.is_empty() {
        let п = gtk::Label::new(Some("Список программ ещё не пришёл"));
        п.style_context().add_class("empty");
        сетка.add(&п);
    }
    let mut м_ = м.borrow_mut();
    м_.программы = список;
    м_.слепок_программ = слепок;
    сетка.show_all();
}

/// Меню программы по правой кнопке — те же пункты, что у страничного.
fn меню_программы(кнопка: &gtk::Button, id: &str, вид: &str, имя: &str, событие: &gtk::gdk::EventButton) {
    let машина = вид == "машина";
    let данные = ДАННЫЕ.with(|д| д.borrow().clone());
    let есть = |поле: &str| данные[поле].as_array().is_some_and(|с| с.iter().any(|x| x.as_str() == Some(id)));

    let меню = gtk::Menu::new();
    let пункт = |текст: &str, что: &'static str, закрыть: bool| {
        let п = crate::apps::пункт_меню(&текст);
        let доводы = json!({ "id": id, "вид": вид, "имя": имя });
        п.connect_activate(move |_| {
            if закрыть {
                закрой();
            }
            дело(что, доводы.clone());
        });
        меню.append(&п);
    };
    пункт("Открыть", "запусти", true);
    if !машина || данные["столы"].as_u64().unwrap_or(1) > 1 {
        пункт("Открыть на новом рабочем столе", "на-новый-стол", true);
    }
    меню.append(&gtk::SeparatorMenuItem::new());
    if машина {
        пункт(if есть("на-столе") { "Убрать с рабочего стола" } else { "Вынести на рабочий стол" }, "вынеси", false);
    } else {
        пункт("Вынести на рабочий стол", "вынеси", false);
        пункт(if есть("в-доке") { "Убрать из дока" } else { "Закрепить в доке" }, "док", false);
    }
    if машина {
        пункт("Найти в Магазине", "магазин", true);
        меню.append(&gtk::SeparatorMenuItem::new());
        пункт("Удалить", "удали", true);
    }
    меню.show_all();
    let событие: &gtk::gdk::Event = событие;
    меню.popup_at_pointer(Some(событие));
    let _ = кнопка;
}

// ------------------------------------------------------------- столы и окна

fn наполни_столы(м: &Rc<RefCell<Меню>>) {
    let (столы, окна, карточки, д) = {
        let м_ = м.borrow();
        (м_.столы.clone(), м_.окна.clone(), м_.карточки.clone(), м_.данные.clone())
    };
    for р in карточки.children() {
        карточки.remove(&р);
    }
    for р in столы.children() {
        столы.remove(&р);
    }
    for р in окна.children() {
        окна.remove(&р);
    }
    let сколько = д["столы"].as_u64().unwrap_or(1).max(1);
    let текущий = д["текущий"].as_u64().unwrap_or(0);
    let на_столах: Vec<u64> = д["на-столах"]
        .as_array()
        .map(|с| с.iter().map(|x| x.as_u64().unwrap_or(0)).collect())
        .unwrap_or_default();

    for i in 0..сколько {
        let окон = на_столах.get(i as usize).copied().unwrap_or(0);
        let стол = gtk::Overlay::new();
        let б = gtk::Button::with_label(&if окон > 0 { format!("{} · {}", i + 1, окон) } else { format!("{}", i + 1) });
        б.style_context().add_class("desk");
        if i == текущий {
            б.style_context().add_class("on");
        }
        б.connect_clicked(move |_| дело("стол", json!({ "n": i })));
        прими(&б, move |груз| брошено(груз, Some(i)));
        стол.add(&б);
        // Убрать можно любой стол, кроме последнего: окна с него переедут
        if сколько > 1 && д["можно-убрать"].as_bool() != Some(false) {
            let крестик = crate::apps::значок_кнопка("×");
            крестик.style_context().add_class("desk-x");
            крестик.set_halign(gtk::Align::End);
            крестик.set_valign(gtk::Align::Start);
            крестик.set_tooltip_text(Some(if окон > 0 { "Убрать стол — окна переедут на соседний" } else { "Убрать этот рабочий стол" }));
            крестик.connect_clicked(move |_| дело("стол-убери", json!({ "n": i })));
            стол.add_overlay(&крестик);
        }
        столы.add(&стол);
    }
    if д["можно-ещё"].as_bool() != Some(false) && сколько < 8 {
        let плюс = crate::apps::значок_кнопка("+");
        плюс.style_context().add_class("desk");
        плюс.style_context().add_class("desk-plus");
        плюс.set_tooltip_text(Some("Новый рабочий стол"));
        плюс.connect_clicked(|_| дело("стол-добавь", json!({})));
        прими(&плюс, |груз| брошено(груз, None));
        столы.add(&плюс);
    }

    for о in д["окна"].as_array().cloned().unwrap_or_default() {
        let id = о["id"].as_str().unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let б = gtk::Button::new();
        б.style_context().add_class("win");
        let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let з = наш_значок(о["знак"].as_str().unwrap_or(""), о["фон"].as_str().unwrap_or(""));
        з.style_context().add_class("small");
        ряд.add(&з);
        let имя = gtk::Label::new(о["имя"].as_str());
        имя.set_max_width_chars(22);
        имя.set_ellipsize(gtk::pango::EllipsizeMode::End);
        ряд.add(&имя);
        б.add(&ряд);
        тащи(&б, json!({ "окно": id }), &з);
        б.connect_clicked(move |_| {
            закрой();
            дело("окно", json!({ "id": id }));
        });
        окна.add(&б);
        карточки.add(&карточка_окна(&о));
    }
    let всего = карточки.children().len() as u32;
    карточки.set_max_children_per_line(всего.clamp(1, 4));
    if всего == 0 {
        let пусто = gtk::Label::new(Some("Открытых окон нет"));
        пусто.style_context().add_class("task-empty");
        карточки.add(&пусто);
    }
    столы.show_all();
    окна.show_all();
    карточки.show_all();
    let задачи = м.borrow().задачи;
    окна.set_visible(!задачи);
    карточки.set_visible(задачи);
}

/// Карточка окна в просмотре задач: значок, имя, крестик «закрыть»
fn карточка_окна(о: &Value) -> gtk::Widget {
    let id = о["id"].as_str().unwrap_or("").to_string();
    let слой = gtk::Overlay::new();
    let б = gtk::Button::new();
    б.style_context().add_class("task");
    if о["свёрнуто"].as_bool() == Some(true) {
        б.style_context().add_class("min");
    }
    let столбик = gtk::Box::new(gtk::Orientation::Vertical, 12);
    столбик.set_valign(gtk::Align::Center);
    let з: gtk::Widget = if о["знак"].as_str().unwrap_or("").is_empty() {
        let имя_значка = gio::DesktopAppInfo::new(&format!("{}.desktop", о["приложение"].as_str().unwrap_or("")))
            .and_then(|д| д.icon())
            .and_then(|и| и.downcast::<gio::ThemedIcon>().ok())
            .and_then(|т| т.names().first().map(|н| н.to_string()))
            .unwrap_or_else(|| "application-x-executable".into());
        gtk::Image::from_icon_name(Some(&имя_значка), gtk::IconSize::Dialog).upcast()
    } else {
        наш_значок(о["знак"].as_str().unwrap_or(""), о["фон"].as_str().unwrap_or(""))
    };
    з.set_halign(gtk::Align::Center);
    столбик.add(&з);
    let имя = gtk::Label::new(о["имя"].as_str());
    имя.set_max_width_chars(26);
    имя.set_ellipsize(gtk::pango::EllipsizeMode::End);
    столбик.add(&имя);
    б.add(&столбик);
    тащи(&б, json!({ "окно": id }), &з);
    {
        let id = id.clone();
        б.connect_clicked(move |_| {
            закрой();
            дело("окно", json!({ "id": id }));
        });
    }
    слой.add(&б);
    let крестик = crate::apps::значок_кнопка("×");
    крестик.style_context().add_class("desk-x");
    крестик.set_halign(gtk::Align::End);
    крестик.set_valign(gtk::Align::Start);
    крестик.set_tooltip_text(Some("Закрыть окно"));
    крестик.connect_clicked(move |к| {
        дело("окно-закрой", json!({ "id": id }));
        // Карточку убираем сразу, не дожидаясь опроса окон
        if let Some(р) = к.parent().and_then(|с| с.parent()) {
            р.hide();
        }
    });
    слой.add_overlay(&крестик);
    слой.upcast()
}
