//! Окошко установки программы из скачанного файла — как у Windows при
//! запуске установщика: имя, полоса хода, что сейчас делается. Раньше о
//! ходе говорили только два уведомления, «устанавливаю» и «установлено»,
//! а между ними минуты тишины — не понять, идёт ли что-нибудь вообще.
//!
//! Окно можно скрыть: установка идёт дальше, а чем кончилась, скажет
//! уведомление. Кончилась удачно — кнопка «Открыть».

use crate::agent::позови;
use crate::notify;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use serde_json::json;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

pub const СТИЛЬ: &str = r#"
window.instwin { background-color: @glower_bg; color: @glower_fg; }
.instwin headerbar { background-image: none; background-color: @glower_bg2; border: none; box-shadow: none; }
.instwin button { background-image: none; background-color: alpha(@glower_fg, 0.08); color: @glower_fg; border: none; border-radius: 8px; padding: 6px 16px; box-shadow: none; }
.instwin button:hover { background-color: alpha(@glower_fg, 0.14); }
.instwin button.accent { background-image: linear-gradient(140deg, @glower_accent, @glower_accent2); color: #ffffff; font-weight: 600; }
.instwin progressbar progress { background-image: linear-gradient(90deg, @glower_accent, @glower_accent2); border: none; }
.instwin progressbar trough { background-color: alpha(@glower_fg, 0.10); border: none; }
.instwin .name { font-size: 16px; font-weight: 600; }
.instwin .step { font-size: 12px; opacity: 0.65; }
.instwin progressbar trough { min-height: 6px; border-radius: 3px; }
.instwin progressbar progress { min-height: 6px; border-radius: 3px; }
"#;

/// Ярлык поставленной программы: сперва по списку файлов пакета, потом по имени
pub fn ярлык_пакета(пакет: &str) -> Option<gio::AppInfo> {
    if пакет.contains('.') {
        // flatpak: имя и есть имя ярлыка
        if let Some(а) = gio::DesktopAppInfo::new(&format!("{пакет}.desktop")) {
            return Some(а.upcast());
        }
    }
    if let Ok(в) = std::process::Command::new("dpkg-query").args(["-L", пакет]).output() {
        for с in String::from_utf8_lossy(&в.stdout).lines() {
            if с.starts_with("/usr/share/applications/") && с.ends_with(".desktop") {
                if let Some(а) = gio::DesktopAppInfo::from_filename(с).filter(|а| !а.is_nodisplay()) {
                    return Some(а.upcast());
                }
            }
        }
    }
    let ниже = пакет.to_lowercase();
    gio::AppInfo::all().into_iter().find(|а| {
        let id = а.id().map(|и| и.to_lowercase()).unwrap_or_default();
        id.trim_end_matches(".desktop") == ниже
    })
}

pub fn покажи(имя: &str, пакет: &str) {
    thread_local!(static СТИЛЬ_ЕСТЬ: Cell<bool> = const { Cell::new(false) });
    if !СТИЛЬ_ЕСТЬ.with(|с| с.replace(true)) {
        let с = gtk::CssProvider::new();
        if с.load_from_data(СТИЛЬ.as_bytes()).is_ok() {
            if let Some(э) = gtk::gdk::Screen::default() {
                gtk::StyleContext::add_provider_for_screen(&э, &с, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
            }
        }
    }
    let окно = gtk::Window::new(gtk::WindowType::Toplevel);
    окно.set_title(&crate::i18n::tr("Установка программы"));
    окно.set_default_size(440, -1);
    окно.set_resizable(false);
    окно.set_icon_name(Some("system-software-install"));
    окно.style_context().add_class("glower-app");
    crate::apps::строка_заголовка(&окно);
    окно.style_context().add_class("instwin");

    let к = gtk::Box::new(gtk::Orientation::Vertical, 14);
    к.set_margin(20);
    окно.add(&к);
    let верх = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    let значок = gtk::Image::from_icon_name(Some("system-software-install"), gtk::IconSize::Dialog);
    значок.set_pixel_size(48);
    верх.add(&значок);
    let тексты = gtk::Box::new(gtk::Orientation::Vertical, 4);
    тексты.set_valign(gtk::Align::Center);
    let название = gtk::Label::new(Some(имя));
    название.set_xalign(0.0);
    название.style_context().add_class("name");
    тексты.add(&название);
    let шаг = gtk::Label::new(Some(&crate::i18n::tr("Готовлюсь к установке…")));
    шаг.set_xalign(0.0);
    шаг.set_line_wrap(true);
    шаг.set_max_width_chars(44);
    шаг.style_context().add_class("step");
    тексты.add(&шаг);
    верх.add(&тексты);
    к.add(&верх);
    let ход = gtk::ProgressBar::new();
    к.add(&ход);
    let низ = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    низ.set_halign(gtk::Align::End);
    let скрыть = gtk::Button::with_label(&crate::i18n::tr("Скрыть"));
    let отмена = gtk::Button::with_label(&crate::i18n::tr("Отмена"));
    низ.add(&скрыть);
    низ.add(&отмена);
    к.add(&низ);
    окно.show_all();

    let скрыто = Rc::new(Cell::new(false));
    let кончено = Rc::new(Cell::new(false));
    {
        let (окно, скрыто) = (окно.clone(), скрыто.clone());
        скрыть.connect_clicked(move |_| {
            скрыто.set(true);
            окно.hide();
        });
    }
    {
        // Закрыли крестиком посреди работы — как «Скрыть», а не отмена
        let (скрыто, кончено) = (скрыто.clone(), кончено.clone());
        окно.connect_delete_event(move |о, _| {
            if кончено.get() {
                return glib::Propagation::Proceed;
            }
            скрыто.set(true);
            о.hide();
            glib::Propagation::Stop
        });
    }
    отмена.connect_clicked(move |б| {
        б.set_sensitive(false);
        позови("pkg.cancel", json!({}), |_| {});
    });

    let (имя, пакет) = (имя.to_string(), пакет.to_string());
    let ждём = Rc::new(Cell::new(false));
    glib::timeout_add_local(Duration::from_millis(700), move || {
        if кончено.get() {
            return glib::ControlFlow::Break;
        }
        if ждём.replace(true) {
            return glib::ControlFlow::Continue;
        }
        let (окно, шаг, ход, низ, скрыто, кончено, ждём, имя, пакет) =
            (окно.clone(), шаг.clone(), ход.clone(), низ.clone(), скрыто.clone(), кончено.clone(), ждём.clone(), имя.clone(), пакет.clone());
        позови("pkg.job", json!({}), move |о| {
            ждём.set(false);
            let Ok(j) = о else { return };
            if кончено.get() {
                return;
            }
            if j["running"].as_bool() == Some(true) {
                let сколько = j["percent"].as_f64().unwrap_or(0.0);
                ход.set_fraction((сколько / 100.0).clamp(0.0, 1.0));
                let т = j["step"].as_str().unwrap_or("");
                шаг.set_text(&if т.is_empty() { format!("{}%", сколько.round()) } else { format!("{т} · {}%", сколько.round()) });
                return;
            }
            кончено.set(true);
            for р in низ.children() {
                низ.remove(&р);
            }
            let удачно = j["ok"].as_bool() == Some(true);
            let отменено = j["error"].as_str() == Some("работа остановлена");
            if удачно {
                ход.set_fraction(1.0);
                шаг.set_text(&crate::i18n::tr("Установлено"));
                crate::core::обнови_программы();
                if let Some(а) = ярлык_пакета(&пакет) {
                    let б = gtk::Button::with_label(&crate::i18n::tr("Открыть"));
                    б.style_context().add_class("accent");
                    let о2 = окно.clone();
                    б.connect_clicked(move |_| {
                        if let Err(е) = а.launch(&[], None::<&gio::AppLaunchContext>) {
                            notify::покажи("Не запустилась", &е.to_string(), "⚠️");
                        }
                        о2.close();
                    });
                    низ.add(&б);
                }
                if скрыто.get() {
                    notify::покажи(&имя, "Установлено", "✅");
                }
            } else if отменено {
                шаг.set_text(&crate::i18n::tr("Установка отменена"));
            } else {
                let е = j["error"].as_str().unwrap_or("система не сказала, что пошло не так");
                шаг.set_text(&format!("{} {е}", crate::i18n::tr("Не установилось:")));
                if скрыто.get() {
                    notify::покажи(&format!("{имя}: установка не удалась"), е, "⚠️");
                }
            }
            let готово = gtk::Button::with_label(&crate::i18n::tr(if удачно { "Готово" } else { "Закрыть" }));
            let о2 = окно.clone();
            готово.connect_clicked(move |_| о2.close());
            низ.add(&готово);
            низ.show_all();
        });
        glib::ControlFlow::Continue
    });
}
