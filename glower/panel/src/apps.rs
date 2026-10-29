//! Общее для нативных программ системы: Блокнот, Калькулятор и следующие.
//!
//! Каждая — та же программа, что панель, под своим именем (glower-notepad,
//! glower-calc), и настоящее окно оконного сервера: Alt+Tab, Alt+F4,
//! сворачивание — всё как у любой программы.

use gtk::glib;
use gtk::prelude::*;
use std::path::PathBuf;

/// Вид программ — те же тёмные поверхности, что у окон системы на странице.
const СТИЛЬ: &str = r#"
window.glower-app { background-color: @glower_bg; }
/* Углы и тень окна — как в Windows 11: скругление 10 точек, мягкая тень
   под окном и тонкая светлая кайма, чтобы тёмное окно не сливалось с
   тёмными обоями. Развёрнутое и прижатое к краю окно — без них */
window.glower-app.csd, window.glower-app.csd decoration { border-radius: 10px; }
window.glower-app.csd decoration {
  box-shadow: 0 14px 40px rgba(0, 0, 0, 0.60), 0 4px 12px rgba(0, 0, 0, 0.40), 0 0 0 1px rgba(255, 255, 255, 0.09);
}
window.glower-app.csd decoration:backdrop {
  box-shadow: 0 8px 26px rgba(0, 0, 0, 0.42), 0 0 0 1px rgba(255, 255, 255, 0.06);
}
window.glower-app.maximized, window.glower-app.maximized decoration,
window.glower-app.tiled, window.glower-app.tiled decoration,
window.glower-app.fullscreen, window.glower-app.fullscreen decoration { border-radius: 0; }
window.glower-app.maximized decoration, window.glower-app.fullscreen decoration { box-shadow: none; }
window.glower-app.tiled decoration { box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.06); }
/* Верхняя строка окна — одна на всю систему: та же высота, цвет и кнопки
   «свернуть, развернуть, закрыть», что у терминала (foot.ini, [csd]) и у
   рамки, которую оконный сервер рисует программам X11 */
window.glower-app headerbar.titlebar, window.glower-app .titlebar.default-decoration {
  background-image: none;
  background-color: #12141b;
  min-height: 34px;
  padding: 0 0 0 10px;
  margin: 0;
  border: none;
  border-radius: 10px 10px 0 0;
  box-shadow: inset 0 -1px rgba(255, 255, 255, 0.06);
}
window.glower-app.maximized headerbar.titlebar, window.glower-app.tiled headerbar.titlebar,
window.glower-app.fullscreen headerbar.titlebar { border-radius: 0; }
/* Крестик в углу окна — со скруглением угла, иначе его подсветка торчит
   квадратом из круглого угла */
window.glower-app.csd headerbar.titlebar button.titlebutton.close { border-radius: 0 10px 0 0; }
window.glower-app.maximized headerbar.titlebar button.titlebutton.close,
window.glower-app.tiled headerbar.titlebar button.titlebutton.close { border-radius: 0; }
window.glower-app headerbar.titlebar *, window.glower-app .titlebar.default-decoration * { color: #e8eaf2; }
window.glower-app headerbar.titlebar .title { font-weight: 600; font-size: 13px; margin-left: 2px; }
window.glower-app headerbar.titlebar:backdrop .title { opacity: 0.6; }
window.glower-app headerbar.titlebar button.titlebutton,
window.glower-app .titlebar.default-decoration button.titlebutton {
  background-image: none;
  background-color: transparent;
  border: none;
  border-radius: 0;
  box-shadow: none;
  min-width: 46px;
  min-height: 34px;
  padding: 0;
  margin: 0;
}
window.glower-app headerbar.titlebar button.titlebutton:hover { background-color: rgba(255, 255, 255, 0.10); }
window.glower-app headerbar.titlebar button.titlebutton.close:hover { background-color: #e81123; }
window.glower-app, window.glower-app * {
  color: @glower_fg;
  font-family: sans-serif;
  text-shadow: none;
  -gtk-icon-shadow: none;
}
.glower-app .toolbar { padding: 8px 10px; border-bottom: 1px solid alpha(@glower_fg, 0.07); }
.glower-app .findbar { padding: 6px 10px; border-bottom: 1px solid alpha(@glower_fg, 0.07); background-color: @glower_bg3; }
.glower-app button {
  background-image: none;
  background-color: alpha(@glower_fg, 0.07);
  border: none;
  border-radius: 9px;
  box-shadow: none;
  padding: 5px 11px;
  min-height: 0;
}
.glower-app button:hover { background-color: alpha(@glower_fg, 0.13); }
.glower-app button.accent { background-image: linear-gradient(140deg, @glower_accent, @glower_accent2); }
.glower-app button.tab-close { background: none; padding: 0 2px; }
.glower-app entry {
  background-color: alpha(@glower_fg, 0.06);
  border: 1px solid alpha(@glower_fg, 0.12);
  border-radius: 9px;
  box-shadow: none;
  padding: 4px 9px;
  min-height: 0;
}
.glower-app entry:focus { border-color: alpha(@glower_accent, 0.8); }
.glower-app .muted { opacity: 0.6; }
.glower-app textview, .glower-app textview text { background-color: @glower_bg2; }
.glower-app .statusbar { padding: 4px 12px; font-size: 12px; opacity: 0.65; border-top: 1px solid alpha(@glower_fg, 0.07); }
.glower-app notebook header { background-color: @glower_bg; border: none; box-shadow: none; }
.glower-app notebook tab { padding: 4px 10px; border: none; }
.glower-app notebook tab:checked { background-color: @glower_bg2; border-radius: 8px 8px 0 0; }
.glower-app notebook stack { background-color: @glower_bg2; }
.glower-app switch {
  background-image: none;
  background-color: alpha(@glower_fg, 0.16);
  border: none;
  border-radius: 14px;
  box-shadow: none;
  min-width: 46px;
  min-height: 24px;
  color: transparent;
  font-size: 0;
}
.glower-app switch:checked { background-image: linear-gradient(140deg, @glower_accent, @glower_accent2); }
.glower-app switch image { color: transparent; opacity: 0; }
.glower-app switch slider {
  background-image: none;
  background-color: #ffffff;
  border: none;
  border-radius: 10px;
  min-width: 20px;
  min-height: 20px;
  margin: 2px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.35);
}
.glower-app switch:disabled { opacity: 0.45; }
"#;

/// Меню по правой кнопке, выпадающие списки, всплывающие окошки и подсказки —
/// одного вида во всей системе: скруглённые, с тенью, пункт под мышью
/// подсвечен цветом акцента. Без этого GTK рисовал их серыми прямоугольниками
/// Adwaita, чужими рядом со всем остальным
pub const МЕНЮ: &str = r#"
window.popup.csd decoration, window.popup decoration {
  border-radius: 12px;
  box-shadow: 0 10px 28px rgba(0, 0, 0, 0.45), 0 0 0 1px alpha(@glower_fg, 0.10);
}
window.popup, window.popup.background { background-color: transparent; }
menu, .context-menu {
  background-color: @glower_bg;
  border: none;
  border-radius: 12px;
  padding: 6px;
}
menu menuitem {
  padding: 7px 14px 7px 12px;
  border-radius: 8px;
  min-height: 18px;
  color: @glower_fg;
  text-shadow: none;
  transition: background-color 120ms ease-out;
}
menu menuitem:hover { background-color: alpha(@glower_accent, 0.85); color: #ffffff; }
menu menuitem:hover label, menu menuitem:hover arrow { color: #ffffff; }
menu menuitem:disabled, menu menuitem:disabled label { color: alpha(@glower_fg, 0.4); }
menu menuitem arrow { min-width: 14px; min-height: 14px; color: alpha(@glower_fg, 0.6); }
menu menuitem accelerator { color: alpha(@glower_fg, 0.5); }
menu separator { margin: 4px 8px; min-height: 1px; background-color: alpha(@glower_fg, 0.09); }
menu > arrow { background-color: @glower_bg; border: none; }
popover {
  background-color: @glower_bg;
  border: none;
  border-radius: 12px;
  box-shadow: 0 10px 28px rgba(0, 0, 0, 0.40), 0 0 0 1px alpha(@glower_fg, 0.10);
  padding: 6px;
}
popover modelbutton { border-radius: 8px; padding: 6px 12px; }
popover modelbutton:hover { background-color: alpha(@glower_accent, 0.85); color: #ffffff; }
tooltip, tooltip.background {
  background-color: rgba(22, 24, 32, 0.96);
  border-radius: 8px;
  border: 1px solid rgba(255, 255, 255, 0.08);
}
tooltip label { color: #e8eaf2; padding: 2px 4px; }
/* Окна вопросов («Стереть диск?», «Удалить навсегда?») */
messagedialog.csd decoration, messagedialog decoration {
  border-radius: 14px;
  box-shadow: 0 16px 44px rgba(0, 0, 0, 0.55), 0 0 0 1px alpha(@glower_fg, 0.10);
}
messagedialog, messagedialog.background { background-color: @glower_bg; border-radius: 0 0 14px 14px; }
messagedialog .titlebar, messagedialog headerbar { background-image: none; background-color: @glower_bg; border: none; box-shadow: none; min-height: 0; padding: 0; border-radius: 14px 14px 0 0; }
messagedialog label { color: @glower_fg; }
messagedialog .titlebar + box, messagedialog box.vertical > box.horizontal { padding: 8px 10px 4px; }
messagedialog .dialog-action-area, messagedialog .dialog-action-box { padding: 4px 16px 16px 16px; border: none; margin: 0; }
messagedialog .dialog-action-area button, messagedialog .dialog-action-box button {
  background-image: none;
  background-color: alpha(@glower_fg, 0.08);
  border: none;
  border-radius: 10px;
  box-shadow: none;
  padding: 9px 16px;
  margin: 0 4px;
  color: @glower_fg;
}
messagedialog .dialog-action-area button:hover, messagedialog .dialog-action-box button:hover { background-color: alpha(@glower_fg, 0.14); }
messagedialog .dialog-action-area button:last-child, messagedialog .dialog-action-box button:last-child {
  background-image: linear-gradient(140deg, @glower_accent, @glower_accent2);
  color: #ffffff;
  font-weight: 600;
}
messagedialog .dialog-action-area button:last-child label, messagedialog .dialog-action-box button:last-child label { color: #ffffff; }
check, radio {
  min-width: 16px;
  min-height: 16px;
  background-image: none;
  background-color: alpha(@glower_fg, 0.06);
  border: 1.5px solid alpha(@glower_fg, 0.35);
  box-shadow: none;
  -gtk-icon-source: none;
  margin-right: 8px;
}
check { border-radius: 5px; }
radio { border-radius: 50%; }
check:hover, radio:hover { border-color: alpha(@glower_fg, 0.55); }
check:checked, radio:checked {
  background-color: @glower_accent;
  border-color: @glower_accent;
  color: #ffffff;
}
check:checked { -gtk-icon-source: -gtk-icontheme("object-select-symbolic"); }
radio:checked { -gtk-icon-source: -gtk-icontheme("media-record-symbolic"); -gtk-icon-transform: scale(0.6); }
check:indeterminate { background-color: @glower_accent; border-color: @glower_accent; -gtk-icon-source: -gtk-icontheme("list-remove-symbolic"); color: #ffffff; }
"#;

/// Имя программы для оконного сервера (по нему её узнают док и Alt+Tab),
/// GTK, тёмная тема и общий стиль.
pub fn подготовь(имя: &str, название: &str) {
    glib::set_prgname(Some(имя));
    glib::set_application_name(название);
    if gtk::init().is_err() {
        eprintln!("{имя}: нет экрана");
        std::process::exit(1);
    }
    crate::i18n::подключи();
    crate::undo::подключи();
    // Кнопки окна — как в Windows и как у терминала: свернуть, развернуть,
    // закрыть. Без этого GTK оставлял одну «закрыть»
    if let Some(н) = gtk::Settings::default() {
        н.set_gtk_decoration_layout(Some("menu:minimize,maximize,close"));
        // Значки кнопок — из Adwaita, она стоит в системе всегда. Без этого
        // GTK шёл по общей настройке (в Ubuntu там Yaru, которой у нас нет)
        // и брал цветные значки из запасной темы вперемешку с пустыми
        н.set_gtk_icon_theme_name(Some("Adwaita"));
    }
    // Тема и акцент — до своего стиля: в нём имена цветов, а не цвета
    crate::theme::подключи();
    let стиль = gtk::CssProvider::new();
    стиль.load_from_data(format!("{СТИЛЬ}{МЕНЮ}").as_bytes()).expect("стиль программ");
    gtk::StyleContext::add_provider_for_screen(
        &gtk::gdk::Screen::default().expect("экран"),
        &стиль,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

/// Своя верхняя строка окна: имя слева, кнопки справа — как в Windows и
/// как у терминала (свою строку foot рисует сам и ставит имя слева, а GTK
/// по умолчанию — посередине). Зовётся сразу после создания окна: строку
/// можно поставить только до того, как окно появилось на экране
pub fn строка_заголовка(окно: &gtk::Window) {
    if окно.window_type() != gtk::WindowType::Toplevel || окно.is_realized() || !окно.is_decorated() || окно.titlebar().is_some() {
        return;
    }
    let строка = gtk::HeaderBar::new();
    строка.set_show_close_button(true);
    let имя = gtk::Label::new(окно.title().as_deref());
    имя.style_context().add_class("title");
    имя.set_ellipsize(gtk::pango::EllipsizeMode::End);
    имя.set_xalign(0.0);
    окно.bind_property("title", &имя, "label").build();
    // Строка заголовка отдаёт левой части только её наименьшую ширину, и с
    // многоточием это ноль: в узком окне (Калькулятор) имя пропадало вовсе.
    // Просим ширину по длине имени, но не больше 28 знаков
    let ширина = |имя: &gtk::Label| имя.set_width_chars(имя.text().chars().count().min(28) as i32);
    ширина(&имя);
    имя.connect_label_notify(ширина);
    строка.pack_start(&имя);
    // Пустая середина — иначе GTK поставил бы туда имя второй раз
    строка.set_custom_title(Some(&gtk::Box::new(gtk::Orientation::Horizontal, 0)));
    строка.show_all();
    // Ставя свою строку, GTK стирает заголовок окна — возвращаем его, иначе
    // окна, назвавшие себя заранее (Календарь, Paint, Музыка), оставались
    // без имени и в строке, и в панели задач
    let было = окно.title();
    окно.set_titlebar(Some(&строка));
    if окно.title().is_none() {
        if let Some(т) = было {
            окно.set_title(&т);
        }
    }
}

/// Картинка из папки assets системы: знак, логотип с надписью, обои. Ищем
/// там, куда её кладёт установка, а на стенде — рядом с исходниками
pub fn файл_системы(имя: &str) -> Option<PathBuf> {
    let рядом = std::env::current_exe().ok().and_then(|п| п.parent().map(|р| р.join("../../../assets").join(имя)));
    [PathBuf::from("/usr/share/glower/ui/assets").join(имя), PathBuf::from("assets").join(имя)]
        .into_iter()
        .chain(рядом)
        .find(|п| п.is_file())
        // Путь уходит другим программам (Параметрам, замку) — только полный:
        // у них своя рабочая папка
        .map(|п| std::fs::canonicalize(&п).unwrap_or(п))
}

/// Та же картинка, вписанная в ширину и высоту (пропорции сохраняются)
pub fn картинка_системы(имя: &str, ширина: i32, высота: i32) -> Option<gtk::gdk_pixbuf::Pixbuf> {
    gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(файл_системы(имя)?, ширина, высота, true).ok()
}

/// Сделать дело вне асинхронной задачи — следующим шагом цикла событий.
///
/// Окно-вопрос с `dialog.run()` крутит вложенный цикл событий. Если он
/// запущен изнутри задачи (`spawn_local`), GLib во вложенном цикле
/// принимается опрашивать другие задачи, пока первая ещё не отпущена, и
/// падает (EnterError) — вместе со всей панелью. Так двойной щелчок по
/// скачанному .deb ронял рабочий стол. Поэтому всё, что приходит из задач
/// (ответы агента, итоги работы с файлами), отдаём сюда
pub fn вне_задачи(дело: impl FnOnce() + 'static) {
    // Приоритет обычный, а не «простоя»: ответ не должен ждать, пока
    // перерисуется всё остальное
    let mut дело = Some(дело);
    glib::idle_add_local_full(glib::Priority::DEFAULT, move || {
        if let Some(д) = дело.take() {
            д();
        }
        glib::ControlFlow::Break
    });
}

pub fn кнопка(текст: &str, подсказка: &str) -> gtk::Button {
    let б = gtk::Button::new();
    знак_кнопки(&б, текст);
    if !подсказка.is_empty() {
        б.set_tooltip_text(Some(подсказка));
    }
    б.set_valign(gtk::Align::Center);
    б
}

/// Кнопка только со значком (без подсказки)
pub fn значок_кнопка(текст: &str) -> gtk::Button {
    кнопка(текст, "")
}

/// Значки кнопок — настоящие, из темы Adwaita, а не символы шрифта: «⏮»,
/// «💾», «‹» в разных шрифтах выходили разного размера, съезжали с
/// середины и выглядели чужими. Символ в начале надписи — только имя
/// значка: «💾 Сохранить» становится значком сохранения и словом
fn значок_по_символу(символ: &str) -> Option<&'static str> {
    Some(match символ.trim_end_matches('\u{fe0f}') {
        "‹" => "go-previous-symbolic",
        "›" | "→" => "go-next-symbolic",
        "↑" => "go-up-symbolic",
        "↓" => "go-down-symbolic",
        "＋" | "➕" | "+" => "list-add-symbolic",
        "⊕" => "zoom-in-symbolic",
        "⊖" | "−" => "zoom-out-symbolic",
        "☰" => "view-list-symbolic",
        "▦" => "view-grid-symbolic",
        "↩" | "↶" => "edit-undo-symbolic",
        "↷" => "edit-redo-symbolic",
        "🧹" => "edit-clear-all-symbolic",
        "⏮" => "media-skip-backward-symbolic",
        "▶" => "media-playback-start-symbolic",
        "⏸" => "media-playback-pause-symbolic",
        "⏭" => "media-skip-forward-symbolic",
        "📁" => "folder-symbolic",
        "📂" => "document-open-symbolic",
        "💾" => "document-save-symbolic",
        "🔎" | "🔍" => "edit-find-symbolic",
        "📄" => "document-new-symbolic",
        "⤢" => "zoom-fit-best-symbolic",
        "⟲" => "object-rotate-left-symbolic",
        "⟳" => "object-rotate-right-symbolic",
        "🖼" => "image-x-generic-symbolic",
        "🗑" => "user-trash-symbolic",
        "⛶" => "view-fullscreen-symbolic",
        "✂" => "edit-cut-symbolic",
        "🖥" => "video-display-symbolic",
        "📋" => "edit-copy-symbolic",
        "…" => "document-save-as-symbolic",
        "×" => "window-close-symbolic",
        "💻" => "computer-symbolic",
        "📌" | "📍" => "view-pin-symbolic",
        "⏺" => "media-record-symbolic",
        "📱" => "phone-symbolic",
        "🗜" => "package-x-generic-symbolic",
        "🖴" => "drive-harddisk-symbolic",
        "⬆" => "software-update-available-symbolic",
        "🔔" => "preferences-system-notifications-symbolic",
        "🔕" => "notifications-disabled-symbolic",
        другой => return имя_значка(другой),
    })
}

/// Какой значок темы стоит за эмодзи — одна таблица на всю систему:
/// разделы, места, строки настроек, программы, уведомления
pub fn имя_значка(знак: &str) -> Option<&'static str> {
    Some(match знак.trim_end_matches('\u{fe0f}') {
        "💻" => "computer-symbolic",
        "📱" => "phone-symbolic",
        "⏺" => "media-record-symbolic",
        "⏹" => "media-playback-stop-symbolic",
        "🗜" => "package-x-generic-symbolic",
        "🖥" => "video-display-symbolic",
        "🏠" => "go-home-symbolic",
        "🎨" | "🖌" => "applications-graphics-symbolic",
        "🧱" | "🧩" => "view-app-grid-symbolic",
        "🔊" | "🎚" => "audio-volume-high-symbolic",
        "🔈" => "audio-volume-low-symbolic",
        "🔇" => "audio-volume-muted-symbolic",
        "💡" | "☀" => "display-brightness-symbolic",
        "📶" => "network-wireless-symbolic",
        "📡" => "network-transmit-receive-symbolic",
        "💾" | "🫙" => "drive-removable-media-symbolic",
        "🖴" => "drive-harddisk-symbolic",
        "💽" => "drive-optical-symbolic",
        "🖨" => "printer-symbolic",
        "🎧" => "audio-headphones-symbolic",
        "🔵" => "bluetooth-symbolic",
        "🔔" => "preferences-system-notifications-symbolic",
        "🔕" | "📭" => "notifications-disabled-symbolic",
        "📦" => "package-x-generic-symbolic",
        "🛍" => "system-software-install-symbolic",
        "👤" | "🙂" => "avatar-default-symbolic",
        "👥" => "system-users-symbolic",
        "🌍" | "🗣" => "preferences-desktop-locale-symbolic",
        "⌨" => "input-keyboard-symbolic",
        "🔐" | "🛡" => "security-high-symbolic",
        "🔒" => "system-lock-screen-symbolic",
        "🔑" => "dialog-password-symbolic",
        "⬆" => "software-update-available-symbolic",
        "⬇" => "folder-download-symbolic",
        "ℹ" => "help-about-symbolic",
        "🗑" => "user-trash-symbolic",
        "📄" => "x-office-document-symbolic",
        "🖼" => "image-x-generic-symbolic",
        "🎵" => "audio-x-generic-symbolic",
        "🎬" => "applications-multimedia-symbolic",
        "🎮" => "input-gaming-symbolic",
        "📚" => "view-list-bullet-symbolic",
        "🌙" | "🌗" => "weather-clear-night-symbolic",
        "🌤" => "weather-few-clouds-symbolic",
        "✨" => "starred-symbolic",
        "🔀" => "media-playlist-shuffle-symbolic",
        "🔁" | "🔄" | "♻" => "view-refresh-symbolic",
        "🌐" => "web-browser-symbolic",
        "🕐" | "⏱" => "preferences-system-time-symbolic",
        "⏰" => "alarm-symbolic",
        "📅" => "x-office-calendar-symbolic",
        "🚀" => "system-run-symbolic",
        "🗂" | "📁" => "folder-symbolic",
        "🪟" => "preferences-desktop-display-symbolic",
        "📤" => "mail-send-symbolic",
        "🛠" | "🔧" => "applications-utilities-symbolic",
        "🧪" => "applications-science-symbolic",
        "🧑\u{200d}💻" => "utilities-terminal-symbolic",
        "⚠" => "dialog-warning-symbolic",
        "✅" | "✓" => "object-select-symbolic",
        "✂" => "edit-cut-symbolic",
        "🔌" => "ac-adapter-symbolic",
        "🔋" => "battery-full-symbolic",
        "🪫" => "battery-low-symbolic",
        "🚫" | "✖" => "action-unavailable-symbolic",
        "📴" => "airplane-mode-symbolic",
        "📷" => "camera-photo-symbolic",
        "🏷" => "computer-symbolic",
        "🏙" => "preferences-desktop-wallpaper-symbolic",
        "🔗" => "insert-link-symbolic",
        "🔍" | "🔎" => "system-search-symbolic",
        "🎯" => "find-location-symbolic",
        "📝" => "accessories-text-editor-symbolic",
        "⚙" => "emblem-system-symbolic",
        "🧮" => "accessories-calculator-symbolic",
        "📊" | "🧠" => "view-continuous-symbolic",
        "🎉" => "emblem-ok-symbolic",
        "💵" => "emblem-documents-symbolic",
        "↔" => "object-flip-horizontal-symbolic",
        "🖇" | "🧲" => "insert-link-symbolic",
        "🐧" | "▢" => "application-x-executable-symbolic",
        // Род программы в каталоге Магазина — пока не пришёл её настоящий значок
        "📬" | "✉" => "mail-unread-symbolic",
        "📥" => "folder-download-symbolic",
        "📆" => "x-office-calendar-symbolic",
        "🧭" => "find-location-symbolic",
        "📖" | "📑" | "📰" => "x-office-document-symbolic",
        "💰" | "🏦" | "📈" => "x-office-spreadsheet-symbolic",
        "🗒" | "✍" | "🖊" | "✒" => "accessories-text-editor-symbolic",
        "▶" => "media-playback-start-symbolic",
        "🎥" | "🎞" => "camera-video-symbolic",
        "🎙" => "audio-input-microphone-symbolic",
        "📸" => "camera-photo-symbolic",
        "🖍" | "🧊" => "applications-graphics-symbolic",
        "🕹" => "input-gaming-symbolic",
        "⏪" => "media-seek-backward-symbolic",
        "🗄" | "🗃" => "folder-symbolic",
        "🔢" => "accessories-calculator-symbolic",
        "🧹" => "edit-clear-all-symbolic",
        "🩺" => "applications-utilities-symbolic",
        "✈" => "mail-send-symbolic",
        _ => return None,
    })
}

/// Значок вместо эмодзи нужного размера; без пары в теме — сам символ
pub fn значок_эмодзи(знак: &str, размер: i32) -> gtk::Widget {
    match имя_значка(знак) {
        Some(имя) => {
            let к = gtk::Image::from_icon_name(Some(имя), gtk::IconSize::Button);
            к.set_pixel_size(размер);
            к.upcast()
        }
        None => gtk::Label::new(Some(знак)).upcast(),
    }
}

/// Пункт меню: символ в начале надписи — значком, как у кнопок
pub fn пункт_меню(текст: &str) -> gtk::MenuItem {
    let текст = crate::i18n::tr(текст);
    let п = gtk::MenuItem::new();
    match текст.split_once(' ') {
        Some((с, о)) if !с.chars().any(|ч| ч.is_alphanumeric()) && значок_по_символу(с).is_some() => {
            let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let к = gtk::Image::from_icon_name(значок_по_символу(с), gtk::IconSize::Menu);
            ряд.add(&к);
            let л = gtk::Label::new(Some(о.trim()));
            л.set_xalign(0.0);
            ряд.add(&л);
            п.add(&ряд);
        }
        _ => {
            let л = gtk::Label::new(Some(&текст));
            л.set_xalign(0.0);
            п.add(&л);
        }
    }
    п
}

/// Поставить на кнопку надпись, а символ в её начале — значком. Годится и
/// для смены на ходу («▶» ↔ «⏸»): прежнее содержимое кнопки убирается
pub fn знак_кнопки(б: &gtk::Button, текст: &str) {
    // Переводим целиком, вместе с символом: так строки лежат в словарях
    let текст = crate::i18n::tr(текст);
    if let Some(было) = б.child() {
        б.remove(&было);
    }
    let (символ, остаток) = match текст.split_once(' ') {
        Some((с, о)) if !с.chars().any(|ч| ч.is_alphanumeric()) => (с, о.trim()),
        _ if !текст.chars().any(|ч| ч.is_alphanumeric()) => (текст.as_str(), ""),
        _ => ("", текст.as_str()),
    };
    match значок_по_символу(символ) {
        Some(имя) => {
            let картинка = gtk::Image::from_icon_name(Some(имя), gtk::IconSize::Button);
            if остаток.is_empty() {
                б.add(&картинка);
                б.style_context().add_class("image-button");
            } else {
                let ряд = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                ряд.set_halign(gtk::Align::Center);
                ряд.add(&картинка);
                ряд.add(&gtk::Label::new(Some(остаток)));
                б.add(&ряд);
                б.style_context().remove_class("image-button");
            }
        }
        None => {
            б.add(&gtk::Label::new(Some(&текст)));
            б.style_context().remove_class("image-button");
        }
    }
    б.show_all();
}

/// Где по умолчанию открывать и сохранять: документы системы
/// (~/GlowerOS/Документы), а нет их — дом.
pub fn папка_документов() -> PathBuf {
    let дом = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()));
    for п in [дом.join("GlowerOS").join("Документы"), дом.join("Документы"), дом.join("Documents")] {
        if п.is_dir() {
            return п;
        }
    }
    дом
}

/// Данные программ — файлом в ~/.config/glower: календарь, будильники,
/// дела. Их читает и рабочий стол (напоминания, виджеты), поэтому не в
/// памяти программы, а на диске
pub fn файл_данных(имя: &str) -> PathBuf {
    glib::user_config_dir().join("glower").join(имя)
}

pub fn читай_данные(имя: &str) -> serde_json::Value {
    std::fs::read_to_string(файл_данных(имя)).ok().and_then(|т| serde_json::from_str(&т).ok()).unwrap_or(serde_json::Value::Null)
}

/// Записать целиком: сперва во временный файл, потом подменой
pub fn пиши_данные(имя: &str, что: &serde_json::Value) {
    let путь = файл_данных(имя);
    if let Some(р) = путь.parent() {
        let _ = std::fs::create_dir_all(р);
    }
    let временный = путь.with_extension("new");
    if std::fs::write(&временный, serde_json::to_vec_pretty(что).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(&временный, &путь);
    }
}

/// Папка «Изображения» системы
pub fn папка_изображений() -> PathBuf {
    let дом = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()));
    let корень = std::env::var("GLOWER_ROOT").map(PathBuf::from).unwrap_or_else(|_| дом.join("GlowerOS"));
    for п in [корень.join("Изображения"), дом.join("Изображения"), дом.join("Pictures")] {
        if п.is_dir() {
            return п;
        }
    }
    корень.join("Изображения")
}

#[cfg(test)]
mod проверки {
    /// Одно непонятное GTK свойство — и стиль не грузится вовсе, а панель
    /// и все программы падают на старте. Проверяем разбором до сборки
    #[test]
    fn стили_разбираются() {
        use gtk::prelude::*;
        if gtk::init().is_err() {
            return;
        }
        let цвета = crate::theme::цвета(false, &("#3a86ff".into(), "#8b5cf6".into()));
        for (имя, css) in [("программ", format!("{цвета}{}{}", super::СТИЛЬ, super::МЕНЮ)), ("панели", format!("{цвета}{}{}", crate::СТИЛЬ, crate::osd::СТИЛЬ))] {
            let п = gtk::CssProvider::new();
            if let Err(е) = п.load_from_data(css.as_bytes()) {
                panic!("стиль {имя}: {е}");
            }
        }
    }
}
