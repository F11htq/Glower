//! Плеер на верхней полосе — что играет сейчас, у кого угодно: песня в
//! Музыке, ролик в браузере, голосовое в Telegram. Всё — по MPRIS.
//!
//! На полосе — «пилюля»: маленькая обложка, название и кнопки «назад»,
//! «пауза», «вперёд». Щелчок по обложке или названию открывает карточку:
//! большая обложка, фон в её цветах, ход трека, крупные кнопки и «Открыть»
//! — к окну проигрывателя. Нет обложки (видео в браузере) — вместо неё
//! значок программы на цветной плашке.

use crate::media::{self, Играет};
use crate::popup;
use gtk::gdk_pixbuf::Pixbuf;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::rc::Rc;
use std::time::Duration;

struct Плеер {
    пилюля: gtk::Box,
    обложка: gtk::DrawingArea,
    название: gtk::Label,
    назад: gtk::Button,
    пауза: gtk::Button,
    вперёд: gtk::Button,
    открыть: gtk::Button,
    /// Что показано сейчас
    сейчас: Option<Играет>,
    /// Все проигрыватели, что есть на шине
    все: Vec<Играет>,
    /// Выбран человеком в карточке: показываем его, пока он жив
    выбран: Option<String>,
    /// Обложки по адресу; None — не загрузилась
    обложки: HashMap<String, Option<Pixbuf>>,
    /// Открытая карточка: её надо обновлять вместе с полосой
    карточка: Option<gtk::Box>,
}

thread_local! {
    static П: RefCell<Option<Плеер>> = const { RefCell::new(None) };
}

fn с_плеером<R>(дело: impl FnOnce(&mut Плеер) -> R) -> Option<R> {
    П.with(|п| п.borrow_mut().as_mut().map(дело))
}

/// Пилюля для полосы. Сама прячется, когда играть нечему
pub fn создай() -> gtk::Widget {
    let пилюля = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    пилюля.style_context().add_class("np");
    пилюля.set_no_show_all(true);
    пилюля.set_valign(gtk::Align::Center);

    let открыть = gtk::Button::new();
    открыть.style_context().add_class("np-main");
    let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let обложка = gtk::DrawingArea::new();
    обложка.set_size_request(20, 20);
    обложка.set_valign(gtk::Align::Center);
    обложка.connect_draw(|о, к| {
        let (обл, запас) = картинки();
        рисуй_обложку(к, о.allocated_width() as f64, о.allocated_height() as f64, 5.0, обл.as_ref(), запас.as_ref());
        glib::Propagation::Stop
    });
    let название = gtk::Label::new(None);
    название.set_ellipsize(gtk::pango::EllipsizeMode::End);
    название.set_max_width_chars(26);
    название.set_use_markup(true);
    // Название песни и ролика — не надпись системы
    название.style_context().add_class("no-i18n");
    ряд.add(&обложка);
    ряд.add(&название);
    открыть.add(&ряд);
    открыть.set_tooltip_text(Some("Что играет"));
    открыть.connect_clicked(|к| покажи_карточку(к));
    пилюля.add(&открыть);

    let кнопка = |значок: &str, подсказка: &str, метод: &'static str| {
        let б = gtk::Button::from_icon_name(Some(значок), gtk::IconSize::Menu);
        б.style_context().add_class("np-btn");
        б.set_tooltip_text(Some(подсказка));
        б.connect_clicked(move |_| нажми(метод));
        б
    };
    let назад = кнопка("media-skip-backward-symbolic", "Предыдущий", "Previous");
    let пауза = кнопка("media-playback-start-symbolic", "Играть / пауза", "PlayPause");
    let вперёд = кнопка("media-skip-forward-symbolic", "Следующий", "Next");
    for б in [&назад, &пауза, &вперёд] {
        пилюля.add(б);
    }

    П.with(|п| {
        *п.borrow_mut() = Some(Плеер {
            пилюля: пилюля.clone(),
            обложка,
            название,
            назад,
            пауза,
            вперёд,
            открыть,
            сейчас: None,
            все: Vec::new(),
            выбран: None,
            обложки: HashMap::new(),
            карточка: None,
        })
    });

    media::следи(спроси);
    // Не все проигрыватели шлют весточки исправно — переспрашиваем и сами
    glib::timeout_add_local(Duration::from_secs(3), || {
        спроси();
        glib::ControlFlow::Continue
    });
    спроси();
    пилюля.upcast()
}

/// Спросить шину в отдельном потоке: зависший проигрыватель не должен
/// подвешивать полосу
fn спроси() {
    let (куда, откуда) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = куда.send_blocking(media::все());
    });
    glib::MainContext::default().spawn_local(async move {
        if let Ok(все) = откуда.recv().await {
            прими(все);
        }
    });
}

fn прими(все: Vec<Играет>) {
    let Some((сейчас, было)) = с_плеером(|п| {
        // Выбранный в карточке — пока жив; иначе играющий, затем на паузе;
        // при равных — тот, что показан сейчас (не прыгать между вкладками)
        let был = п.сейчас.as_ref().map(|и| и.кто.clone());
        let выбран = п.выбран.clone().filter(|в| все.iter().any(|и| &и.кто == в));
        п.выбран = выбран.clone();
        let mut ряд = все.clone();
        ряд.sort_by_key(|и| {
            (
                выбран.as_ref() != Some(&и.кто),
                match и.статус.as_str() {
                    "Playing" => 0,
                    "Paused" => 1,
                    _ => 2,
                },
                был.as_ref() != Some(&и.кто),
            )
        });
        п.все = все;
        let было = п.сейчас.clone();
        п.сейчас = ряд.into_iter().next();
        (п.сейчас.clone(), было)
    }) else {
        return;
    };
    if сейчас == было {
        return;
    }
    if let Some(и) = &сейчас {
        загрузи_обложку(&и.обложка);
    }
    нарисуй();
}

/// Картинки для текущего: обложка и запасная — значок программы
fn картинки() -> (Option<Pixbuf>, Option<Pixbuf>) {
    с_плеером(|п| {
        let Some(и) = &п.сейчас else { return (None, None) };
        let обл = п.обложки.get(&и.обложка).cloned().flatten();
        (обл, значок_программы(и))
    })
    .unwrap_or((None, None))
}

fn значок_программы(и: &Играет) -> Option<Pixbuf> {
    let тема = gtk::IconTheme::default()?;
    let ярлык = gio::DesktopAppInfo::new(&format!("{}.desktop", и.ярлык)).or_else(|| gio::DesktopAppInfo::new(&и.ярлык));
    if let Some(з) = ярлык.and_then(|я| я.icon()) {
        if let Some(п) = тема.lookup_by_gicon(&з, 128, gtk::IconLookupFlags::FORCE_SIZE).and_then(|и| и.load_icon().ok()) {
            return Some(п);
        }
    }
    let запас = if и.ярлык.contains("glower-music") { "audio-x-generic" } else { "multimedia-player" };
    тема.load_icon(запас, 96, gtk::IconLookupFlags::FORCE_SIZE).ok().flatten()
}

/// Обложку тянем в отдельном потоке: она может быть и в интернете
fn загрузи_обложку(адрес: &str) {
    if адрес.is_empty() || с_плеером(|п| п.обложки.contains_key(адрес)).unwrap_or(true) {
        return;
    }
    с_плеером(|п| п.обложки.insert(адрес.to_string(), None));
    let адрес = адрес.to_string();
    let (куда, откуда) = async_channel::bounded(1);
    {
        let адрес = адрес.clone();
        std::thread::spawn(move || {
            let байты: Option<Vec<u8>> = if let Some(данные) = адрес.strip_prefix("data:") {
                use base64::Engine;
                данные.split_once(";base64,").and_then(|(_, б)| base64::engine::general_purpose::STANDARD.decode(б).ok())
            } else {
                // file:// читается и так, https:// — через gvfs
                gio::File::for_uri(&адрес).load_contents(gio::Cancellable::NONE).ok().map(|(б, _)| б.to_vec())
            };
            let _ = куда.send_blocking(байты);
        });
    }
    glib::MainContext::default().spawn_local(async move {
        let Ok(Some(байты)) = откуда.recv().await else { return };
        let загрузчик = gtk::gdk_pixbuf::PixbufLoader::new();
        if загрузчик.write(&байты).is_err() || загрузчик.close().is_err() {
            return;
        }
        let Some(к) = загрузчик.pixbuf() else { return };
        // Больше карточки не нужно: огромную обложку ужимаем сразу
        let м = (360.0 / к.width().max(к.height()) as f64).min(1.0);
        let к = if м < 1.0 {
            к.scale_simple((к.width() as f64 * м) as i32, (к.height() as f64 * м) as i32, gtk::gdk_pixbuf::InterpType::Bilinear).unwrap_or(к)
        } else {
            к
        };
        с_плеером(|п| {
            // Кэш — небольшой: держим последние обложки, остальное выкидываем
            if п.обложки.len() > 30 {
                п.обложки.clear();
            }
            п.обложки.insert(адрес, Some(к))
        });
        нарисуй();
    });
}

/// Обложка со скруглёнными углами, «заполнить» по месту. Нет обложки —
/// цветная плашка и значок программы посередине
fn рисуй_обложку(к: &gtk::cairo::Context, ш: f64, в: f64, р: f64, обл: Option<&Pixbuf>, запас: Option<&Pixbuf>) {
    к.new_sub_path();
    к.arc(ш - р, р, р, -PI / 2.0, 0.0);
    к.arc(ш - р, в - р, р, 0.0, PI / 2.0);
    к.arc(р, в - р, р, PI / 2.0, PI);
    к.arc(р, р, р, PI, 1.5 * PI);
    к.close_path();
    к.clip();
    match обл {
        Some(о) => {
            let м = (ш / о.width() as f64).max(в / о.height() as f64);
            к.save().ok();
            к.translate((ш - о.width() as f64 * м) / 2.0, (в - о.height() as f64 * м) / 2.0);
            к.scale(м, м);
            к.set_source_pixbuf(о, 0.0, 0.0);
            let _ = к.paint();
            к.restore().ok();
        }
        None => {
            let г = gtk::cairo::LinearGradient::new(0.0, 0.0, ш, в);
            г.add_color_stop_rgb(0.0, 0.49, 0.36, 0.98);
            г.add_color_stop_rgb(1.0, 0.15, 0.55, 0.96);
            let _ = к.set_source(&г);
            let _ = к.paint();
            if let Some(з) = запас {
                let размер = ш.min(в) * 0.62;
                let м = размер / з.width().max(з.height()) as f64;
                к.save().ok();
                к.translate((ш - з.width() as f64 * м) / 2.0, (в - з.height() as f64 * м) / 2.0);
                к.scale(м, м);
                к.set_source_pixbuf(з, 0.0, 0.0);
                let _ = к.paint();
                к.restore().ok();
            }
        }
    }
}

/// Средний цвет обложки — фон карточки в её тонах
fn средний_цвет(о: &Pixbuf) -> (f64, f64, f64) {
    let Some(т) = о.scale_simple(1, 1, gtk::gdk_pixbuf::InterpType::Bilinear) else { return (0.3, 0.3, 0.5) };
    let б = т.read_pixel_bytes();
    if б.len() < 3 {
        return (0.3, 0.3, 0.5);
    }
    (б[0] as f64 / 255.0, б[1] as f64 / 255.0, б[2] as f64 / 255.0)
}

fn экранируй(т: &str) -> String {
    glib::markup_escape_text(т).to_string()
}

fn нарисуй() {
    let Some(()) = с_плеером(|п| {
        let Some(и) = п.сейчас.clone() else {
            п.пилюля.hide();
            return;
        };
        let имя = if и.название.is_empty() { и.программа.clone() } else { и.название.clone() };
        let подпись = if и.исполнитель.is_empty() {
            format!("<b>{}</b>", экранируй(&имя))
        } else {
            format!("<b>{}</b>  <span alpha=\"60%\">{}</span>", экранируй(&имя), экранируй(&и.исполнитель))
        };
        п.название.set_markup(&подпись);
        п.открыть.set_tooltip_text(Some(&format!("{} · {}", имя, и.программа)));
        п.пауза.set_image(Some(&gtk::Image::from_icon_name(Some(if и.играет() { "media-playback-pause-symbolic" } else { "media-playback-start-symbolic" }), gtk::IconSize::Menu)));
        п.обложка.queue_draw();
        // У пилюли no_show_all (иначе show_all полосы показал бы пустую) —
        // детей показываем сами, а «назад»/«вперёд» — только если можно
        п.пилюля.show();
        п.открыть.show_all();
        п.пауза.show_all();
        for (б, можно) in [(&п.назад, и.назад), (&п.вперёд, и.вперёд)] {
            б.show_all();
            б.set_visible(можно);
        }
    }) else {
        return;
    };
    // Карточка открыта — перерисовать и её
    if let Some(к) = с_плеером(|п| п.карточка.clone()).flatten() {
        if к.toplevel().is_some_and(|т| т.is_visible()) {
            наполни_карточку(&к);
        } else {
            с_плеером(|п| п.карточка = None);
        }
    }
}

/// Нажали «пауза» / «дальше». Значок меняем сразу, не дожидаясь ответа
fn нажми(метод: &'static str) {
    let Some(кто) = с_плеером(|п| {
        if метод == "PlayPause" {
            if let Some(и) = п.сейчас.as_mut() {
                и.статус = if и.играет() { "Paused".into() } else { "Playing".into() };
            }
        }
        п.сейчас.as_ref().map(|и| и.кто.clone())
    })
    .flatten() else {
        return;
    };
    if метод == "PlayPause" {
        нарисуй();
    }
    std::thread::spawn(move || media::нажми_у(&кто, метод));
    glib::timeout_add_local_once(Duration::from_millis(350), спроси);
}

/* ---------- карточка ---------- */

fn покажи_карточку(кнопка: &gtk::Button) {
    popup::открой(кнопка, "плеер", popup::Где::ПодКнопкой, 340, |меню| {
        меню.style_context().add_class("np-card");
        с_плеером(|п| п.карточка = Some(меню.clone()));
        наполни_карточку(меню);
        // Ход трека — раз в секунду, пока карточка открыта
        let м = меню.downgrade();
        glib::timeout_add_local(Duration::from_secs(1), move || {
            let Some(м) = м.upgrade() else { return glib::ControlFlow::Break };
            if !м.toplevel().is_some_and(|т| т.is_visible()) {
                return glib::ControlFlow::Break;
            }
            обнови_ход(&м);
            glib::ControlFlow::Continue
        });
    });
}

thread_local! {
    /// Полоса хода, подписи времени и стиль фона открытой карточки
    static ХОД: RefCell<Option<(gtk::DrawingArea, gtk::Label, gtk::Label, Rc<std::cell::Cell<f64>>)>> = const { RefCell::new(None) };
    /// Стиль фона — свой у каждой открытой карточки
    static ФОН: RefCell<Option<(glib::WeakRef<gtk::Box>, gtk::CssProvider)>> = const { RefCell::new(None) };
}

fn наполни_карточку(меню: &gtk::Box) {
    popup::очисти(меню);
    let Some((и, обл, запас, другие)) = с_плеером(|п| {
        let и = п.сейчас.clone()?;
        let обл = п.обложки.get(&и.обложка).cloned().flatten();
        let другие: Vec<Играет> = п.все.iter().filter(|д| д.кто != и.кто).cloned().collect();
        let запас = значок_программы(&и);
        Some((и, обл, запас, другие))
    })
    .flatten() else {
        popup::тихо(меню, "Сейчас ничего не играет");
        меню.show_all();
        return;
    };

    // Фон — в цветах обложки, книзу темнее
    let (r, g, b) = обл.as_ref().map(средний_цвет).unwrap_or((0.36, 0.30, 0.75));
    let стиль = ФОН.with(|ф| {
        let mut ф = ф.borrow_mut();
        match ф.as_ref() {
            Some((м, с)) if м.upgrade().as_ref() == Some(меню) => с.clone(),
            _ => {
                let с = gtk::CssProvider::new();
                меню.style_context().add_provider(&с, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 3);
                *ф = Some((меню.downgrade(), с.clone()));
                с
            }
        }
    });
    let css = format!(
        ".np-card {{ background-image: linear-gradient(170deg, rgba({},{},{},0.92) 0%, rgba(18,20,28,0.97) 72%); }}",
        (r * 200.0) as u8, (g * 200.0) as u8, (b * 200.0) as u8
    );
    let _ = стиль.load_from_data(css.as_bytes());

    let верх = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    верх.set_margin_top(6);
    let большая = gtk::DrawingArea::new();
    большая.set_size_request(112, 112);
    {
        let (обл, запас) = (обл.clone(), запас.clone());
        большая.connect_draw(move |о, к| {
            рисуй_обложку(к, о.allocated_width() as f64, о.allocated_height() as f64, 12.0, обл.as_ref(), запас.as_ref());
            glib::Propagation::Stop
        });
    }
    верх.add(&большая);
    let текст = gtk::Box::new(gtk::Orientation::Vertical, 4);
    текст.set_valign(gtk::Align::Center);
    let н = gtk::Label::new(Some(if и.название.is_empty() { &и.программа } else { &и.название }));
    н.style_context().add_class("np-title");
    н.set_xalign(0.0);
    н.set_line_wrap(true);
    н.set_lines(3);
    н.set_ellipsize(gtk::pango::EllipsizeMode::End);
    н.set_max_width_chars(20);
    н.style_context().add_class("no-i18n");
    текст.add(&н);
    if !и.исполнитель.is_empty() {
        let а = gtk::Label::new(Some(&и.исполнитель));
        а.style_context().add_class("np-artist");
        а.set_xalign(0.0);
        а.set_ellipsize(gtk::pango::EllipsizeMode::End);
        а.set_max_width_chars(22);
        а.style_context().add_class("no-i18n");
        текст.add(&а);
    }
    // Откуда звук: значок и имя программы
    let откуда = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    откуда.style_context().add_class("np-app");
    if let Some(з) = &запас {
        if let Some(м) = з.scale_simple(14, 14, gtk::gdk_pixbuf::InterpType::Bilinear) {
            откуда.add(&gtk::Image::from_pixbuf(Some(&м)));
        }
    }
    let пр = gtk::Label::new(Some(&и.программа));
    пр.style_context().add_class("no-i18n");
    откуда.add(&пр);
    текст.add(&откуда);
    верх.pack_start(&текст, true, true, 0);
    меню.add(&верх);

    // Ход трека: полоса и время. У прямого эфира длины нет — полосы нет
    if и.длина > 0 {
        let доля = Rc::new(std::cell::Cell::new(0.0f64));
        let полоса = gtk::DrawingArea::new();
        полоса.set_size_request(-1, 14);
        полоса.set_margin_top(10);
        {
            let доля = доля.clone();
            полоса.connect_draw(move |п, к| {
                let (ш, в) = (п.allocated_width() as f64, п.allocated_height() as f64);
                let у = в / 2.0;
                к.set_line_cap(gtk::cairo::LineCap::Round);
                к.set_line_width(4.0);
                к.set_source_rgba(1.0, 1.0, 1.0, 0.18);
                к.move_to(2.0, у);
                к.line_to(ш - 2.0, у);
                let _ = к.stroke();
                let x = 2.0 + (ш - 4.0) * доля.get().clamp(0.0, 1.0);
                к.set_source_rgba(1.0, 1.0, 1.0, 0.92);
                к.move_to(2.0, у);
                к.line_to(x, у);
                let _ = к.stroke();
                к.arc(x, у, 5.0, 0.0, 2.0 * PI);
                let _ = к.fill();
                glib::Propagation::Stop
            });
        }
        меню.add(&полоса);
        let времена = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let прошло = gtk::Label::new(Some("0:00"));
        let всего = gtk::Label::new(Some(&media::время(и.длина)));
        for л in [&прошло, &всего] {
            л.style_context().add_class("np-time");
            л.style_context().add_class("no-i18n");
        }
        времена.pack_start(&прошло, false, false, 0);
        времена.pack_end(&всего, false, false, 0);
        меню.add(&времена);
        ХОД.with(|х| *х.borrow_mut() = Some((полоса, прошло, всего, доля)));
        обнови_ход(меню);
    } else {
        ХОД.with(|х| *х.borrow_mut() = None);
    }

    // Крупные кнопки
    let кнопки = gtk::Box::new(gtk::Orientation::Horizontal, 18);
    кнопки.set_halign(gtk::Align::Center);
    кнопки.set_margin_top(8);
    let большая_кнопка = |значок: &str, класс: &str, метод: &'static str, можно: bool| {
        let б = gtk::Button::from_icon_name(Some(значок), gtk::IconSize::LargeToolbar);
        б.style_context().add_class(класс);
        б.set_sensitive(можно);
        б.connect_clicked(move |_| нажми(метод));
        б
    };
    кнопки.add(&большая_кнопка("media-skip-backward-symbolic", "np-big", "Previous", и.назад));
    кнопки.add(&большая_кнопка(if и.играет() { "media-playback-pause-symbolic" } else { "media-playback-start-symbolic" }, "np-play", "PlayPause", true));
    кнопки.add(&большая_кнопка("media-skip-forward-symbolic", "np-big", "Next", и.вперёд));
    меню.add(&кнопки);

    // К окну проигрывателя — к вкладке браузера, к Музыке
    let к_окну = gtk::Button::with_label(&format!("{} «{}»", crate::i18n::tr("Открыть"), и.программа));
    к_окну.style_context().add_class("np-open");
    {
        let кто = и.кто.clone();
        к_окну.connect_clicked(move |_| {
            let кто = кто.clone();
            popup::закрой();
            std::thread::spawn(move || media::подними(&кто));
        });
    }
    меню.add(&к_окну);

    // Играет не одно — можно переключиться
    if !другие.is_empty() {
        let з = gtk::Label::new(Some(&crate::i18n::tr("Ещё звучит")));
        з.style_context().add_class("np-more");
        з.set_xalign(0.0);
        меню.add(&з);
        for д in другие {
            let б = gtk::Button::new();
            б.style_context().add_class("np-other");
            let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            // Звучит — динамик, на паузе — значок паузы
            let значок = gtk::Image::from_icon_name(Some(if д.играет() { "audio-volume-high-symbolic" } else { "media-playback-pause-symbolic" }), gtk::IconSize::Menu);
            ряд.add(&значок);
            let л = gtk::Label::new(Some(&format!("{} — {}", if д.название.is_empty() { "…" } else { &д.название }, д.программа)));
            л.set_ellipsize(gtk::pango::EllipsizeMode::End);
            л.set_max_width_chars(34);
            л.set_xalign(0.0);
            л.style_context().add_class("no-i18n");
            ряд.add(&л);
            б.add(&ряд);
            let кто = д.кто.clone();
            б.connect_clicked(move |_| {
                с_плеером(|п| {
                    п.выбран = Some(кто.clone());
                    п.сейчас = None;
                });
                спроси();
            });
            меню.add(&б);
        }
    }
    меню.show_all();
}

/// Сколько проиграно — спросить и подвинуть полосу
fn обнови_ход(_меню: &gtk::Box) {
    let Some((кто, длина)) = с_плеером(|п| п.сейчас.as_ref().map(|и| (и.кто.clone(), и.длина))).flatten() else { return };
    if длина <= 0 {
        return;
    }
    let (куда, откуда) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = куда.send_blocking(media::позиция(&кто));
    });
    glib::MainContext::default().spawn_local(async move {
        let Ok(Some(поз)) = откуда.recv().await else { return };
        ХОД.with(|х| {
            if let Some((полоса, прошло, _, доля)) = х.borrow().as_ref() {
                доля.set(поз as f64 / длина as f64);
                прошло.set_text(&media::время(поз));
                полоса.queue_draw();
            }
        });
    });
}
