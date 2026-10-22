//! Блокнот — нативный, настоящим окном (glower-notepad).
//!
//! Прежде Блокнот был окном внутри страницы рабочего стола: для оконного
//! сервера его не существовало, Alt+Tab его не видел, а Alt+F4 приходилось
//! передавать странице окольным путём. Здесь он — обычная программа.
//!
//! Умеет то же, что умел страничный: вкладки, открыть и сохранить, найти и
//! заменить, перенос строк, размер шрифта, строка состояния. Файлы —
//! настоящие, с диска; что передали при запуске, то и открываем.

use crate::apps::{подготовь, кнопка, папка_документов};
use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

struct Вкладка {
    путь: RefCell<Option<PathBuf>>,
    буфер: gtk::TextBuffer,
    вид: gtk::TextView,
    ярлык: gtk::Label,
    изменён: Cell<bool>,
    страница: gtk::ScrolledWindow,
}

impl Вкладка {
    fn имя(&self) -> String {
        self.путь
            .borrow()
            .as_ref()
            .and_then(|п| п.file_name().map(|и| и.to_string_lossy().to_string()))
            .unwrap_or_else(|| "Безымянный.txt".into())
    }
    fn текст(&self) -> String {
        let (н, к) = self.буфер.bounds();
        self.буфер.text(&н, &к, false).map(|т| т.to_string()).unwrap_or_default()
    }
}

struct Блокнот {
    окно: gtk::Window,
    вкладки: gtk::Notebook,
    список: RefCell<Vec<Rc<Вкладка>>>,
    строка: gtk::Label,
    поиск: gtk::Revealer,
    найти: gtk::Entry,
    заменить: gtk::Entry,
    счёт: gtk::Label,
    перенос: Cell<bool>,
    размер: Cell<i32>,
    шрифт: gtk::CssProvider,
}

type Б = Rc<Блокнот>;

fn текущая(б: &Б) -> Option<Rc<Вкладка>> {
    let н = б.вкладки.current_page()? as usize;
    б.список.borrow().get(н).cloned()
}

fn обнови_заголовок(б: &Б) {
    if let Some(в) = текущая(б) {
        let метка = if в.изменён.get() { "• " } else { "" };
        б.окно.set_title(&format!("{метка}{} — Блокнот", в.имя()));
    }
    for в in б.список.borrow().iter() {
        в.ярлык.set_text(&format!("{}{}", if в.изменён.get() { "• " } else { "" }, в.имя()));
    }
}

fn обнови_строку(б: &Б) {
    let Some(в) = текущая(б) else { return };
    let итер = в.буфер.iter_at_offset(в.буфер.cursor_position());
    let текст = в.текст();
    let слов = текст.split_whitespace().count();
    б.строка.set_text(&format!(
        "Стр {}, столб {}   ·   Символов: {}   ·   Слов: {}   ·   UTF-8",
        итер.line() + 1,
        итер.line_offset() + 1,
        текст.chars().count(),
        слов
    ));
}

fn примени_шрифт(б: &Б) {
    let css = format!("textview {{ font-family: monospace; font-size: {}px; }}", б.размер.get());
    let _ = б.шрифт.load_from_data(css.as_bytes());
}

fn новая_вкладка(б: &Б, путь: Option<PathBuf>, текст: &str) -> Rc<Вкладка> {
    let буфер = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
    буфер.set_text(текст);
    let вид = gtk::TextView::with_buffer(&буфер);
    вид.set_left_margin(14);
    вид.set_right_margin(14);
    вид.set_top_margin(10);
    вид.set_bottom_margin(10);
    вид.set_wrap_mode(if б.перенос.get() { gtk::WrapMode::WordChar } else { gtk::WrapMode::None });
    вид.style_context().add_provider(&б.шрифт, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
    let страница = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    страница.add(&вид);

    let ярлык = gtk::Label::new(None);
    let закрыть = gtk::Button::from_icon_name(Some("window-close-symbolic"), gtk::IconSize::Menu);
    закрыть.set_relief(gtk::ReliefStyle::None);
    закрыть.style_context().add_class("tab-close");
    let шапка = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    шапка.add(&ярлык);
    шапка.add(&закрыть);
    шапка.show_all();

    let в = Rc::new(Вкладка {
        путь: RefCell::new(путь),
        буфер: буфер.clone(),
        вид: вид.clone(),
        ярлык,
        изменён: Cell::new(false),
        страница: страница.clone(),
    });
    {
        let (б, в_) = (б.clone(), Rc::downgrade(&в));
        буфер.connect_changed(move |_| {
            if let Some(в) = в_.upgrade() {
                if !в.изменён.get() {
                    в.изменён.set(true);
                    обнови_заголовок(&б);
                }
            }
            обнови_строку(&б);
        });
    }
    {
        let б = б.clone();
        буфер.connect_mark_set(move |_, _, метка| {
            if метка.name().as_deref() == Some("insert") {
                обнови_строку(&б);
            }
        });
    }
    {
        let (б, в_) = (б.clone(), Rc::downgrade(&в));
        закрыть.connect_clicked(move |_| {
            if let Some(в) = в_.upgrade() {
                закрой_вкладку(&б, &в);
            }
        });
    }
    б.список.borrow_mut().push(в.clone());
    let н = б.вкладки.append_page(&страница, Some(&шапка));
    б.вкладки.set_tab_reorderable(&страница, false);
    страница.show_all();
    б.вкладки.set_current_page(Some(н));
    // Курсор — в начало: открыли файл, чтобы читать его с начала
    буфер.place_cursor(&буфер.start_iter());
    вид.grab_focus();
    обнови_заголовок(б);
    обнови_строку(б);
    в
}

fn открой_файл(б: &Б, путь: PathBuf) {
    // Этот файл уже открыт — просто переходим к нему
    let уже = б.список.borrow().iter().position(|в| в.путь.borrow().as_ref() == Some(&путь));
    if let Some(н) = уже {
        б.вкладки.set_current_page(Some(н as u32));
        return;
    }
    match std::fs::read(&путь) {
        Ok(байты) => {
            let текст = String::from_utf8_lossy(&байты).to_string();
            // Пустую безымянную вкладку, с которой Блокнот открылся,
            // заменяем открытым файлом — а не держим рядом
            let пустая = {
                let список = б.список.borrow();
                (список.len() == 1 && список[0].путь.borrow().is_none() && !список[0].изменён.get() && список[0].текст().is_empty())
                    .then(|| список[0].clone())
            };
            let в = новая_вкладка(б, Some(путь), &текст);
            в.изменён.set(false);
            if let Some(п) = пустая {
                убери_вкладку(б, &п);
            }
            обнови_заголовок(б);
        }
        Err(е) => сообщи(б, "Не удалось открыть файл", &format!("{}: {е}", путь.display())),
    }
}

fn сообщи(б: &Б, заголовок: &str, текст: &str) {
    let д = gtk::MessageDialog::new(
        Some(&б.окно),
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        gtk::MessageType::Warning,
        gtk::ButtonsType::Close,
        заголовок,
    );
    д.set_secondary_text(Some(текст));
    д.run();
    unsafe { д.destroy() };
}

/// Сохранить вкладку. Без пути — спросить, куда. Верно ли сохранили.
fn сохрани(б: &Б, в: &Rc<Вкладка>, спросить: bool) -> bool {
    let путь = if спросить || в.путь.borrow().is_none() {
        let д = gtk::FileChooserDialog::with_buttons(
            Some("Сохранить как"),
            Some(&б.окно),
            gtk::FileChooserAction::Save,
            &[("Отмена", gtk::ResponseType::Cancel), ("Сохранить", gtk::ResponseType::Accept)],
        );
        д.set_do_overwrite_confirmation(true);
        match в.путь.borrow().as_ref() {
            Some(п) => {
                let _ = д.set_filename(п);
            }
            None => {
                д.set_current_folder(папка_документов());
                д.set_current_name(&в.имя());
            }
        }
        let ответ = д.run();
        let п = д.filename();
        unsafe { д.destroy() };
        if ответ != gtk::ResponseType::Accept {
            return false;
        }
        match п {
            Some(п) => п,
            None => return false,
        }
    } else {
        в.путь.borrow().clone().unwrap()
    };
    match std::fs::write(&путь, в.текст()) {
        Ok(()) => {
            *в.путь.borrow_mut() = Some(путь);
            в.изменён.set(false);
            обнови_заголовок(б);
            true
        }
        Err(е) => {
            сообщи(б, "Не удалось сохранить", &format!("{}: {е}", путь.display()));
            false
        }
    }
}

/// Несохранённое — спросить. Можно ли закрывать.
fn можно_закрыть(б: &Б, в: &Rc<Вкладка>) -> bool {
    if !в.изменён.get() {
        return true;
    }
    if let Some(н) = б.список.borrow().iter().position(|х| Rc::ptr_eq(х, в)) {
        б.вкладки.set_current_page(Some(н as u32));
    }
    let д = gtk::MessageDialog::new(
        Some(&б.окно),
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        gtk::MessageType::Question,
        gtk::ButtonsType::None,
        &format!("Сохранить изменения в «{}»?", в.имя()),
    );
    д.set_secondary_text(Some("Если не сохранить, изменения пропадут."));
    д.add_button("Не сохранять", gtk::ResponseType::No);
    д.add_button("Отмена", gtk::ResponseType::Cancel);
    д.add_button("Сохранить", gtk::ResponseType::Yes);
    д.set_default_response(gtk::ResponseType::Yes);
    let ответ = д.run();
    unsafe { д.destroy() };
    match ответ {
        gtk::ResponseType::Yes => сохрани(б, в, false),
        gtk::ResponseType::No => true,
        _ => false,
    }
}

fn убери_вкладку(б: &Б, в: &Rc<Вкладка>) {
    if let Some(н) = б.вкладки.page_num(&в.страница) {
        б.вкладки.remove_page(Some(н));
    }
    б.список.borrow_mut().retain(|х| !Rc::ptr_eq(х, в));
}

fn закрой_вкладку(б: &Б, в: &Rc<Вкладка>) {
    if !можно_закрыть(б, в) {
        return;
    }
    убери_вкладку(б, в);
    if б.список.borrow().is_empty() {
        // Последнюю вкладку закрыли — закрываем и Блокнот, как в любом
        // редакторе с вкладками
        gtk::main_quit();
        return;
    }
    обнови_заголовок(б);
    обнови_строку(б);
}

fn открой_диалогом(б: &Б) {
    let д = gtk::FileChooserDialog::with_buttons(
        Some("Открыть"),
        Some(&б.окно),
        gtk::FileChooserAction::Open,
        &[("Отмена", gtk::ResponseType::Cancel), ("Открыть", gtk::ResponseType::Accept)],
    );
    д.set_select_multiple(true);
    д.set_current_folder(папка_документов());
    let ответ = д.run();
    let файлы = д.filenames();
    unsafe { д.destroy() };
    if ответ == gtk::ResponseType::Accept {
        for ф in файлы {
            открой_файл(б, ф);
        }
    }
}

// ------------------------------------------------------------- поиск

fn сосчитай(б: &Б) {
    let что = б.найти.text().to_string();
    let Some(в) = текущая(б) else { return };
    if что.is_empty() {
        б.счёт.set_text("");
        return;
    }
    let н = в.текст().matches(что.as_str()).count();
    б.счёт.set_text(&if н > 0 { format!("совпадений: {н}") } else { "не найдено".into() });
}

fn найди_дальше(б: &Б) {
    let что = б.найти.text().to_string();
    let Some(в) = текущая(б) else { return };
    if что.is_empty() {
        return;
    }
    let буфер = &в.буфер;
    let от = буфер.iter_at_offset(буфер.cursor_position());
    let найдено = от
        .forward_search(&что, gtk::TextSearchFlags::CASE_INSENSITIVE, None)
        .or_else(|| буфер.start_iter().forward_search(&что, gtk::TextSearchFlags::CASE_INSENSITIVE, None));
    if let Some((н, к)) = найдено {
        буфер.select_range(&к, &н);
        буфер.select_range(&н, &к);
        let mut н2 = н.clone();
        в.вид.scroll_to_iter(&mut н2, 0.2, false, 0.0, 0.0);
    }
}

fn замени_всё(б: &Б) {
    let что = б.найти.text().to_string();
    let на = б.заменить.text().to_string();
    let Some(в) = текущая(б) else { return };
    if что.is_empty() {
        return;
    }
    let текст = в.текст();
    let н = текст.matches(что.as_str()).count();
    if н == 0 {
        б.счёт.set_text("не найдено");
        return;
    }
    в.буфер.set_text(&текст.replace(что.as_str(), &на));
    б.счёт.set_text(&format!("заменено: {н}"));
}

// ------------------------------------------------------------- окно

pub fn запусти(файлы: Vec<String>) -> ! {
    подготовь("glower-notepad", "Блокнот");

    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_title("Блокнот");
    окно.set_default_size(760, 560);
    окно.set_icon_name(Some("accessories-text-editor"));
    окно.style_context().add_class("glower-app");

    let колонка = gtk::Box::new(gtk::Orientation::Vertical, 0);
    окно.add(&колонка);

    // Панель с кнопками — как у страничного Блокнота
    let панель = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    панель.style_context().add_class("toolbar");
    let новый = кнопка("＋ Новый", "Новая вкладка (Ctrl+N)");
    let открыть = кнопка("📂 Открыть", "Открыть файл (Ctrl+O)");
    let сохранить = кнопка("💾 Сохранить", "Сохранить (Ctrl+S)");
    сохранить.style_context().add_class("accent");
    let как = кнопка("Сохранить как…", "Сохранить в другой файл (Ctrl+Shift+S)");
    let найти_кн = кнопка("🔎 Найти", "Найти и заменить (Ctrl+F)");
    for к in [&новый, &открыть, &сохранить, &как, &найти_кн] {
        панель.add(к);
    }
    let пусто = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    пусто.set_hexpand(true);
    панель.add(&пусто);
    let меньше = кнопка("A−", "Мельче (Ctrl+−)");
    let больше = кнопка("A+", "Крупнее (Ctrl+=)");
    let перенос_ярлык = gtk::Label::new(Some("Перенос"));
    перенос_ярлык.style_context().add_class("muted");
    let перенос = gtk::Switch::new();
    перенос.set_active(true);
    перенос.set_valign(gtk::Align::Center);
    for в in [меньше.upcast_ref::<gtk::Widget>(), больше.upcast_ref(), перенос_ярлык.upcast_ref(), перенос.upcast_ref()] {
        панель.add(в);
    }
    колонка.pack_start(&панель, false, false, 0);

    // Найти и заменить — выезжает под панелью
    let поиск = gtk::Revealer::new();
    let строка_поиска = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    строка_поиска.style_context().add_class("findbar");
    let найти = gtk::Entry::new();
    найти.set_placeholder_text(Some("Найти"));
    let заменить = gtk::Entry::new();
    заменить.set_placeholder_text(Some("Заменить на"));
    let дальше = кнопка("↓", "Следующее (Enter)");
    let всё = кнопка("Заменить всё", "Заменить все совпадения");
    let счёт = gtk::Label::new(None);
    счёт.style_context().add_class("muted");
    let закрыть_поиск = кнопка("×", "Закрыть (Esc)");
    for в in [найти.upcast_ref::<gtk::Widget>(), заменить.upcast_ref(), дальше.upcast_ref(), всё.upcast_ref(), счёт.upcast_ref()] {
        строка_поиска.add(в);
    }
    строка_поиска.pack_end(&закрыть_поиск, false, false, 0);
    поиск.add(&строка_поиска);
    колонка.pack_start(&поиск, false, false, 0);

    let вкладки = gtk::Notebook::new();
    вкладки.set_scrollable(true);
    вкладки.set_show_border(false);
    колонка.pack_start(&вкладки, true, true, 0);

    let строка = gtk::Label::new(None);
    строка.set_xalign(0.0);
    строка.style_context().add_class("statusbar");
    колонка.pack_start(&строка, false, false, 0);

    let б: Б = Rc::new(Блокнот {
        окно: окно.clone(),
        вкладки: вкладки.clone(),
        список: RefCell::new(Vec::new()),
        строка,
        поиск: поиск.clone(),
        найти: найти.clone(),
        заменить: заменить.clone(),
        счёт,
        перенос: Cell::new(true),
        размер: Cell::new(14),
        шрифт: gtk::CssProvider::new(),
    });
    примени_шрифт(&б);

    // Действия
    {
        let б = б.clone();
        новый.connect_clicked(move |_| {
            новая_вкладка(&б, None, "");
        });
    }
    {
        let б = б.clone();
        открыть.connect_clicked(move |_| открой_диалогом(&б));
    }
    {
        let б = б.clone();
        сохранить.connect_clicked(move |_| {
            if let Some(в) = текущая(&б) {
                сохрани(&б, &в, false);
            }
        });
    }
    {
        let б = б.clone();
        как.connect_clicked(move |_| {
            if let Some(в) = текущая(&б) {
                сохрани(&б, &в, true);
            }
        });
    }
    let покажи_поиск = {
        let б = б.clone();
        move || {
            б.поиск.set_reveal_child(true);
            if let Some(в) = текущая(&б) {
                // Выделенное — сразу в поле поиска
                if let Some((н, к)) = в.буфер.selection_bounds() {
                    б.найти.set_text(&в.буфер.text(&н, &к, false).map(|т| т.to_string()).unwrap_or_default());
                }
            }
            б.найти.grab_focus();
            сосчитай(&б);
        }
    };
    let спрячь_поиск = {
        let б = б.clone();
        move || {
            б.поиск.set_reveal_child(false);
            if let Some(в) = текущая(&б) {
                в.вид.grab_focus();
            }
        }
    };
    {
        let п = покажи_поиск.clone();
        найти_кн.connect_clicked(move |_| п());
    }
    {
        let с = спрячь_поиск.clone();
        закрыть_поиск.connect_clicked(move |_| с());
    }
    {
        let б = б.clone();
        найти.connect_changed(move |_| сосчитай(&б));
    }
    {
        let б = б.clone();
        найти.connect_activate(move |_| найди_дальше(&б));
    }
    {
        let б = б.clone();
        дальше.connect_clicked(move |_| найди_дальше(&б));
    }
    {
        let б = б.clone();
        всё.connect_clicked(move |_| замени_всё(&б));
    }
    {
        let б = б.clone();
        перенос.connect_state_set(move |_, вкл| {
            б.перенос.set(вкл);
            for в in б.список.borrow().iter() {
                в.вид.set_wrap_mode(if вкл { gtk::WrapMode::WordChar } else { gtk::WrapMode::None });
            }
            glib::Propagation::Proceed
        });
    }
    let шаг = {
        let б = б.clone();
        move |на: i32| {
            б.размер.set((б.размер.get() + на).clamp(8, 40));
            примени_шрифт(&б);
        }
    };
    {
        let ш = шаг.clone();
        меньше.connect_clicked(move |_| ш(-1));
    }
    {
        let ш = шаг.clone();
        больше.connect_clicked(move |_| ш(1));
    }
    {
        let б = б.clone();
        вкладки.connect_switch_page(move |_, _, _| {
            let б = б.clone();
            // Номер вкладки меняется после сигнала — смотрим следующим ходом
            glib::idle_add_local_once(move || {
                обнови_заголовок(&б);
                обнови_строку(&б);
                сосчитай(&б);
            });
        });
    }

    // Клавиши
    {
        let б = б.clone();
        let (п, с) = (покажи_поиск.clone(), спрячь_поиск.clone());
        окно.connect_key_press_event(move |_, е| {
            use gtk::gdk::keys::constants as к;
            let ctrl = е.state().contains(gtk::gdk::ModifierType::CONTROL_MASK);
            let shift = е.state().contains(gtk::gdk::ModifierType::SHIFT_MASK);
            let клавиша = е.keyval().to_lower();
            if клавиша == к::Escape && б.поиск.reveals_child() {
                с();
                return glib::Propagation::Stop;
            }
            if !ctrl {
                return glib::Propagation::Proceed;
            }
            // Сочетания ищем по физической клавише: при русской раскладке
            // Ctrl+S приходит как «Ctrl+ы»
            let латиница = латинская(е);
            match латиница.as_str() {
                "n" | "t" => {
                    новая_вкладка(&б, None, "");
                }
                "o" => открой_диалогом(&б),
                "s" => {
                    if let Some(в) = текущая(&б) {
                        сохрани(&б, &в, shift);
                    }
                }
                "f" | "h" => п(),
                "w" => {
                    if let Some(в) = текущая(&б) {
                        закрой_вкладку(&б, &в);
                    }
                }
                "=" | "+" => шаг(1),
                "-" => шаг(-1),
                _ => {
                    if клавиша == к::plus || клавиша == к::KP_Add {
                        шаг(1)
                    } else if клавиша == к::minus || клавиша == к::KP_Subtract {
                        шаг(-1)
                    } else {
                        return glib::Propagation::Proceed;
                    }
                }
            }
            glib::Propagation::Stop
        });
    }

    // Закрыть окно — спросить про каждую несохранённую вкладку
    {
        let б = б.clone();
        окно.connect_delete_event(move |_, _| {
            let вкладки: Vec<Rc<Вкладка>> = б.список.borrow().clone();
            for в in вкладки {
                if !можно_закрыть(&б, &в) {
                    return glib::Propagation::Stop;
                }
            }
            gtk::main_quit();
            glib::Propagation::Proceed
        });
    }

    окно.show_all();
    if файлы.is_empty() {
        новая_вкладка(&б, None, "");
    } else {
        for ф in файлы {
            let п = PathBuf::from(&ф);
            if п.exists() {
                открой_файл(&б, п);
            } else {
                // Файла ещё нет — откроем пустую вкладку с этим именем: при
                // сохранении он и появится
                let в = новая_вкладка(&б, Some(п), "");
                в.изменён.set(false);
                обнови_заголовок(&б);
            }
        }
    }
    gtk::main();
    std::process::exit(0);
}

/// Латинская буква физической клавиши — для сочетаний при любой раскладке.
pub(crate) fn латинская(е: &gtk::gdk::EventKey) -> String {
    let карта = gtk::gdk::Keymap::for_display(&gtk::gdk::Display::default().expect("экран"));
    if let Some(карта) = карта {
        for (место, значение) in карта.entries_for_keycode(е.hardware_keycode() as u32) {
            if место.level() != 0 {
                continue;
            }
            if let Some(б) = gtk::gdk::keys::Key::from(значение).to_unicode() {
                if б.is_ascii_graphic() {
                    return б.to_ascii_lowercase().to_string();
                }
            }
        }
    }
    е.keyval().to_unicode().map(|б| б.to_lowercase().to_string()).unwrap_or_default()
}
