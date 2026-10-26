//! Ctrl+Z в любом поле ввода и тексте — как в Windows.
//!
//! В GTK3 отмены нет вовсе: ни у строки ввода, ни у многострочного текста.
//! Чтобы не прикручивать её к каждому полю каждой программы отдельно, здесь
//! один крючок на «нажата клавиша» для всех виджетов программы сразу: поле,
//! в котором печатают, само копит свою историю.
//!
//! История — снимками текста. Печать подряд копится в один шаг, как в
//! Windows: новый снимок появляется на границе слова (пробел, Enter,
//! стирание, вставка) или после паузы. Ctrl+Z — назад, Ctrl+Y и
//! Ctrl+Shift+Z — вперёд.

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::time::{Duration, Instant};

const КЛЮЧ: &str = "glower-otmena";
const ГЛУБИНА: usize = 200;
const ПАУЗА: Duration = Duration::from_millis(900);

#[derive(Default)]
struct История {
    назад: Vec<(String, i32)>,
    вперёд: Vec<(String, i32)>,
    последнее: Option<Instant>,
}

/// Текст поля и где стоит курсор; None — поле не для правки
fn снимок(в: &gtk::Widget) -> Option<(String, i32)> {
    if let Some(п) = в.downcast_ref::<gtk::Entry>() {
        return п.is_editable().then(|| (п.text().to_string(), п.position()));
    }
    if let Some(т) = в.downcast_ref::<gtk::TextView>() {
        if !т.is_editable() {
            return None;
        }
        let б = т.buffer()?;
        let (н, к) = б.bounds();
        return Some((б.text(&н, &к, true).map(|т| т.to_string()).unwrap_or_default(), б.cursor_position()));
    }
    None
}

fn верни(в: &gtk::Widget, (текст, курсор): &(String, i32)) {
    if let Some(п) = в.downcast_ref::<gtk::Entry>() {
        п.set_text(текст);
        п.set_position(*курсор);
    } else if let Some(т) = в.downcast_ref::<gtk::TextView>() {
        if let Some(б) = т.buffer() {
            б.set_text(текст);
            let и = б.iter_at_offset(*курсор);
            б.place_cursor(&и);
            т.scroll_to_iter(&mut б.iter_at_offset(*курсор), 0.1, false, 0.0, 0.0);
        }
    }
}

fn с_историей<R>(в: &gtk::Widget, дело: impl FnOnce(&mut История) -> R) -> R {
    // Safety: под этим ключом всегда лежит только RefCell<История>
    unsafe {
        if в.data::<RefCell<История>>(КЛЮЧ).is_none() {
            в.set_data(КЛЮЧ, RefCell::new(История::default()));
        }
        let ячейка = в.data::<RefCell<История>>(КЛЮЧ).expect("история").as_ref();
        let mut и = ячейка.borrow_mut();
        дело(&mut и)
    }
}

/// Нажата клавиша в поле. true — нажатие наше (отмена или повтор)
fn клавиша(в: &gtk::Widget, е: &gdk::EventKey) -> bool {
    let Some(сейчас) = снимок(в) else { return false };
    let ctrl = е.state().contains(gdk::ModifierType::CONTROL_MASK);
    let shift = е.state().contains(gdk::ModifierType::SHIFT_MASK);
    let буква = crate::notepad::латинская(е);
    if ctrl && буква == "z" && !shift {
        let шаг = с_историей(в, |и| {
            // Что не успело попасть в историю (печатали только что) — тоже шаг
            while let Some(было) = и.назад.pop() {
                if было.0 != сейчас.0 {
                    и.вперёд.push(сейчас.clone());
                    return Some(было);
                }
            }
            None
        });
        if let Some(было) = шаг {
            верни(в, &было);
        }
        с_историей(в, |и| и.последнее = None);
        return true;
    }
    if ctrl && (буква == "y" || (буква == "z" && shift)) {
        let шаг = с_историей(в, |и| {
            let было = и.вперёд.pop()?;
            и.назад.push(сейчас.clone());
            Some(было)
        });
        if let Some(было) = шаг {
            верни(в, &было);
        }
        return true;
    }
    // Клавиши, которые текста не меняют, историю не трогают
    use gdk::keys::constants as к;
    let ключ = е.keyval();
    let граница = matches!(ключ, к::space | к::Return | к::KP_Enter | к::Tab | к::BackSpace | к::Delete | к::KP_Delete)
        || (ctrl && matches!(буква.as_str(), "v" | "x"));
    let печать = ключ.to_unicode().is_some_and(|с| !с.is_control()) && !ctrl;
    if !граница && !печать {
        return false;
    }
    с_историей(в, |и| {
        let давно = и.последнее.is_none_or(|т| т.elapsed() > ПАУЗА);
        let другое = и.назад.last().is_none_or(|(т, _)| *т != сейчас.0);
        if другое && (граница || давно) {
            и.назад.push(сейчас);
            if и.назад.len() > ГЛУБИНА {
                и.назад.remove(0);
            }
            и.вперёд.clear();
        }
        и.последнее = Some(Instant::now());
    });
    false
}

unsafe extern "C" fn при_клавише(
    _подсказка: *mut glib::gobject_ffi::GSignalInvocationHint,
    число: u32,
    доводы: *const glib::gobject_ffi::GValue,
    _данные: glib::ffi::gpointer,
) -> glib::ffi::gboolean {
    if число >= 2 {
        let доводы = std::slice::from_raw_parts(доводы as *const glib::Value, число as usize);
        if let (Ok(в), Ok(е)) = (доводы[0].get::<gtk::Widget>(), доводы[1].get::<gdk::Event>()) {
            if let Some(е) = е.downcast_ref::<gdk::EventKey>() {
                if е.event_type() == gdk::EventType::KeyPress {
                    клавиша(&в, е);
                }
            }
        }
    }
    glib::ffi::GTRUE
}

/// Включить отмену во всех полях этой программы. Зовётся сразу после gtk::init
pub fn подключи() {
    unsafe {
        use glib::translate::IntoGlib;
        let id = glib::gobject_ffi::g_signal_lookup(c"key-press-event".as_ptr(), gtk::Widget::static_type().into_glib());
        if id != 0 {
            glib::gobject_ffi::g_signal_add_emission_hook(id, 0, Some(при_клавише), std::ptr::null_mut(), None);
        }
    }
}
