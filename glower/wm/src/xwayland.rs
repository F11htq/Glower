//! Программы X11 (Wine, старые программы) через Xwayland. Заголовков они
//! сами не рисуют — рамку с кнопками рисует сервер

use std::{os::fd::OwnedFd, process::Stdio};

use smithay::{
    backend::renderer::{
        element::{solid::SolidColorRenderElement, Id, Kind},
        Color32F,
    },
    desktop::Window,
    utils::{Logical, Physical, Point, Rectangle, Scale, Size, SERIAL_COUNTER},
    wayland::selection::{
        data_device::{clear_data_device_selection, request_data_device_client_selection, set_data_device_selection},
        primary_selection::{clear_primary_selection, request_primary_client_selection, set_primary_selection},
        SelectionTarget,
    },
    xwayland::{
        xwm::{Reorder, ResizeEdge as X11ResizeEdge, XwmId},
        X11Surface, X11Wm, XWayland, XWaylandEvent, XwmHandler,
    },
};

use crate::{
    grabs::Края,
    state::{Glower, Запись},
};

/// Высота заголовка, который сервер рисует окнам X11
pub const ЗАГОЛОВОК: i32 = 30;
const КНОПКА: i32 = 46;

pub fn запусти(состояние: &mut Glower) {
    let dh = состояние.display_handle.clone();
    let (xwayland, клиент) = match XWayland::spawn(&dh, None, std::iter::empty::<(String, String)>(), true, Stdio::null(), Stdio::null(), |_| ()) {
        Ok(в) => в,
        Err(е) => {
            tracing::warn!("Xwayland не запустился: {е} — программы X11 работать не будут");
            return;
        }
    };
    // Номер экрана известен сразу — ставим DISPLAY до запуска панели, чтобы
    // программы, которые она откроет, знали, куда идти
    let номер = xwayland.display_number();
    std::env::set_var("DISPLAY", format!(":{номер}"));
    let ручка = состояние.loop_handle.clone();
    let итог = ручка.insert_source(xwayland, move |событие, _, с| match событие {
        XWaylandEvent::Ready { x11_socket, display_number } => {
            match X11Wm::start_wm(с.loop_handle.clone(), x11_socket, клиент.clone()) {
                Ok(wm) => {
                    с.xwm = Some(wm);
                    с.xdisplay = Some(display_number);
                    std::env::set_var("DISPLAY", format!(":{display_number}"));
                    tracing::info!("Xwayland готов: DISPLAY=:{display_number}");
                }
                Err(е) => tracing::warn!("оконный менеджер X11 не встал: {е}"),
            }
        }
        XWaylandEvent::Error => tracing::warn!("Xwayland упал при запуске"),
    });
    if let Err(е) = итог {
        tracing::warn!("Xwayland: {е}");
    }
}

/// Нужна ли окну наша рамка: обычным окнам — да, всплывающим и без рамки — нет
pub fn с_рамкой(x: &X11Surface) -> bool {
    !x.is_override_redirect() && !x.is_decorated()
}

/// Рамка окна X11: полоса заголовка и три кнопки, простыми прямоугольниками
pub fn рамка(x: &X11Surface, где: Point<i32, Physical>, масштаб: Scale<f64>, активно: bool) -> Vec<SolidColorRenderElement> {
    if !с_рамкой(x) {
        return vec![];
    }
    let гео = x.geometry();
    let ш = (гео.size.w as f64 * масштаб.x).round() as i32;
    let в = (ЗАГОЛОВОК as f64 * масштаб.y).round() as i32;
    let кн = (КНОПКА as f64 * масштаб.x).round() as i32;
    let верх: Point<i32, Physical> = Point::from((где.x, где.y - в));
    let фон = if активно { Color32F::new(0.12, 0.14, 0.19, 1.0) } else { Color32F::new(0.17, 0.18, 0.22, 1.0) };
    let знак = Color32F::new(0.92, 0.93, 0.96, if активно { 1.0 } else { 0.6 });
    let прямо = |x: i32, y: i32, w: i32, h: i32, цвет: Color32F| {
        SolidColorRenderElement::new(Id::new(), Rectangle::new(Point::from((x, y)), Size::from((w, h))), 0usize, цвет, Kind::Unspecified)
    };
    let mut всё = vec![];
    let т = (масштаб.x.round() as i32).max(1);
    let ц = верх.y + в / 2;
    // Знаки кнопок: закрыть — крест из квадратиков, развернуть — квадрат, свернуть — черта
    let закрыть_x = верх.x + ш - кн / 2;
    for i in -4..=4 {
        let к = i * т;
        всё.push(прямо(закрыть_x + к - т, ц + к - т, 2 * т, 2 * т, знак));
        всё.push(прямо(закрыть_x + к - т, ц - к - т, 2 * т, 2 * т, знак));
    }
    let разв_x = верх.x + ш - кн - кн / 2;
    let с = 5 * т;
    всё.push(прямо(разв_x - с, ц - с, 2 * с, т, знак));
    всё.push(прямо(разв_x - с, ц + с - т, 2 * с, т, знак));
    всё.push(прямо(разв_x - с, ц - с, т, 2 * с, знак));
    всё.push(прямо(разв_x + с - т, ц - с, т, 2 * с, знак));
    let свер_x = верх.x + ш - 2 * кн - кн / 2;
    всё.push(прямо(свер_x - с, ц, 2 * с, т, знак));
    всё.push(прямо(верх.x, верх.y, ш, в, фон));
    всё
}

pub enum Кнопка {
    Закрыть,
    Развернуть,
    Свернуть,
    Заголовок,
}

impl Glower {
    /// Что у рамки окна X11 под мышью
    pub fn рамка_под(&self, точка: Point<f64, Logical>) -> Option<(Window, Кнопка)> {
        for о in self.space.elements().rev() {
            let Some(x) = о.x11_surface() else { continue };
            if !с_рамкой(x) {
                continue;
            }
            let Some(гео) = self.space.element_geometry(о) else { continue };
            let полоса = Rectangle::<i32, Logical>::new(Point::from((гео.loc.x, гео.loc.y - ЗАГОЛОВОК)), Size::from((гео.size.w, ЗАГОЛОВОК)));
            if !полоса.to_f64().contains(точка) {
                // Окно выше по стопке закрывает рамки тех, что ниже
                if гео.to_f64().contains(точка) {
                    return None;
                }
                continue;
            }
            let справа = (полоса.loc.x + полоса.size.w) as f64 - точка.x;
            let к = if справа < КНОПКА as f64 {
                Кнопка::Закрыть
            } else if справа < 2.0 * КНОПКА as f64 {
                Кнопка::Развернуть
            } else if справа < 3.0 * КНОПКА as f64 {
                Кнопка::Свернуть
            } else {
                Кнопка::Заголовок
            };
            return Some((о.clone(), к));
        }
        None
    }

    fn место_для_x11(&self, размер: Size<i32, Logical>) -> Point<i32, Logical> {
        let Some(экран) = self.главный_экран() else { return Point::from((0, ЗАГОЛОВОК)) };
        let о = self.рабочая_область(&экран);
        let x = о.loc.x + ((о.size.w - размер.w) / 2).max(0);
        let y = о.loc.y + ЗАГОЛОВОК + ((о.size.h - ЗАГОЛОВОК - размер.h) / 3).max(0);
        Point::from((x, y))
    }
}

impl XwmHandler for Glower {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.xwm.as_mut().expect("X11Wm")
    }

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) {}
    fn new_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Err(е) = window.set_mapped(true) {
            tracing::warn!("окно X11 не показалось: {е}");
            return;
        }
        let mut гео = window.geometry();
        if гео.size.w < 2 || гео.size.h < 2 {
            гео.size = Size::from((800, 560));
        }
        let место = if с_рамкой(&window) { self.место_для_x11(гео.size) } else { гео.loc };
        гео.loc = место;
        let _ = window.configure(гео);
        let окно = Window::new_x11_window(window);
        self.окна.push(Запись {
            окно: окно.clone(),
            стол: self.стол,
            свёрнуто: false,
            место,
            прежнее: None,
            развёрнуто: false,
            во_весь_экран: false,
        });
        self.space.map_element(окно.clone(), место, true);
        self.анимации.появилось(&окно);
        self.foreign_toplevel_state.новое(&self.display_handle, &окно);
        self.дай_фокус(Some(&окно));
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let место = window.geometry().loc;
        let окно = Window::new_x11_window(window);
        self.space.map_element(окно, место, true);
        self.нужен_кадр = true;
    }

    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let окно = self.space.elements().find(|о| о.x11_surface() == Some(&window)).cloned();
        let в_списке = self.окно_x11(&window);
        if let Some(о) = в_списке.clone() {
            if self.space.element_location(&о).is_some() {
                self.сними_на_прощание(&о);
            }
            self.убери_окно(&о);
        } else if let Some(о) = окно {
            self.space.unmap_elem(&о);
        }
        if !window.is_override_redirect() {
            let _ = window.set_mapped(false);
        }
        self.нужен_кадр = true;
    }

    fn destroyed_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn configure_request(&mut self, _xwm: XwmId, window: X11Surface, _x: Option<i32>, _y: Option<i32>, w: Option<u32>, h: Option<u32>, _reorder: Option<Reorder>) {
        // Место окну даёт сервер, а размер — пусть просит какой хочет
        let mut гео = window.geometry();
        if let Some(w) = w {
            гео.size.w = w as i32;
        }
        if let Some(h) = h {
            гео.size.h = h as i32;
        }
        let _ = window.configure(гео);
    }

    fn configure_notify(&mut self, _xwm: XwmId, window: X11Surface, geometry: Rectangle<i32, Logical>, _above: Option<u32>) {
        let Some(окно) = self.space.elements().find(|о| о.x11_surface() == Some(&window)).cloned() else { return };
        if window.is_override_redirect() {
            self.space.map_element(окно, geometry.loc, false);
        }
        self.нужен_кадр = true;
    }

    fn maximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(о) = self.окно_x11(&window) {
            if !self.запись(&о).map(|з| з.развёрнуто).unwrap_or(true) {
                self.разверни_x11(&о);
            }
        }
    }

    fn unmaximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(о) = self.окно_x11(&window) {
            if self.запись(&о).map(|з| з.развёрнуто).unwrap_or(false) {
                self.разверни_x11(&о);
            }
        }
    }

    fn fullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(о) = self.окно_x11(&window) {
            self.во_весь_экран(&о, true);
        }
    }

    fn unfullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(о) = self.окно_x11(&window) {
            self.во_весь_экран(&о, false);
        }
    }

    fn minimize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(о) = self.окно_x11(&window) {
            self.сверни(&о);
        }
    }

    fn resize_request(&mut self, _xwm: XwmId, window: X11Surface, _button: u32, edge: X11ResizeEdge) {
        let Some(о) = self.окно_x11(&window) else { return };
        let края = Края(match edge {
            X11ResizeEdge::Top => Края::ВЕРХ,
            X11ResizeEdge::Bottom => Края::НИЗ,
            X11ResizeEdge::Left => Края::ЛЕВО,
            X11ResizeEdge::Right => Края::ПРАВО,
            X11ResizeEdge::TopLeft => Края::ВЕРХ | Края::ЛЕВО,
            X11ResizeEdge::TopRight => Края::ВЕРХ | Края::ПРАВО,
            X11ResizeEdge::BottomLeft => Края::НИЗ | Края::ЛЕВО,
            X11ResizeEdge::BottomRight => Края::НИЗ | Края::ПРАВО,
        });
        self.размер_мышью(о, края);
    }

    fn move_request(&mut self, _xwm: XwmId, window: X11Surface, button: u32) {
        let Some(о) = self.окно_x11(&window) else { return };
        self.перенос_мышью(о, SERIAL_COUNTER.next_serial(), button);
    }

    fn allow_selection_access(&mut self, _xwm: XwmId, _selection: SelectionTarget) -> bool {
        // Буфер обмена отдаём, когда фокус у окна X11
        matches!(self.seat.get_keyboard().and_then(|к| к.current_focus()), Some(crate::focus::Фокус::X11(_)))
    }

    fn send_selection(&mut self, _xwm: XwmId, selection: SelectionTarget, mime_type: String, fd: OwnedFd) {
        let итог = match selection {
            SelectionTarget::Clipboard => request_data_device_client_selection(&self.seat, mime_type, fd).map_err(|е| е.to_string()),
            SelectionTarget::Primary => request_primary_client_selection(&self.seat, mime_type, fd).map_err(|е| е.to_string()),
        };
        if let Err(е) = итог {
            tracing::warn!("буфер обмена для X11: {е}");
        }
    }

    fn new_selection(&mut self, _xwm: XwmId, selection: SelectionTarget, mime_types: Vec<String>) {
        match selection {
            SelectionTarget::Clipboard => set_data_device_selection(&self.display_handle, &self.seat, mime_types, ()),
            SelectionTarget::Primary => set_primary_selection(&self.display_handle, &self.seat, mime_types, ()),
        }
    }

    fn cleared_selection(&mut self, _xwm: XwmId, selection: SelectionTarget) {
        match selection {
            SelectionTarget::Clipboard => clear_data_device_selection(&self.display_handle, &self.seat),
            SelectionTarget::Primary => clear_primary_selection(&self.display_handle, &self.seat),
        }
    }
}

impl Glower {
    /// Развернуть окно X11: с нашей рамкой — место под заголовок
    pub fn разверни_x11(&mut self, окно: &Window) {
        self.разверни(окно);
        if let (Some(x), Some(true)) = (окно.x11_surface(), self.запись(окно).map(|з| з.развёрнуто)) {
            if с_рамкой(x) {
                if let Some(гео) = self.space.element_geometry(окно) {
                    let куда = Rectangle::new(Point::from((гео.loc.x, гео.loc.y + ЗАГОЛОВОК)), Size::from((гео.size.w, гео.size.h - ЗАГОЛОВОК)));
                    self.поставь(окно, куда, Some(true));
                }
            }
        }
    }

    pub fn размер_мышью(&mut self, окно: Window, края: Края) {
        let Some(указатель) = self.seat.get_pointer() else { return };
        let Some(гео) = self.space.element_geometry(&окно) else { return };
        let начало = smithay::input::pointer::GrabStartData { focus: None, button: 0x110, location: self.мышь };
        let захват = crate::grabs::Размер { start_data: начало, окно, края, начало: гео, последний: гео.size };
        указатель.set_grab(self, захват, SERIAL_COUNTER.next_serial(), smithay::input::pointer::Focus::Clear);
    }
}
