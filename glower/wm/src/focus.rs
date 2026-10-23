//! Кому достаются клавиатура и мышь: поверхности Wayland или окну X11
//! (программы через Xwayland получают фокус своим, иксовым путём)

use std::borrow::Cow;

use smithay::{
    backend::input::KeyState,
    desktop::PopupKind,
    input::{
        keyboard::{KeyboardTarget, KeysymHandle, ModifiersState},
        pointer::{
            AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent,
            GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent,
            GestureSwipeUpdateEvent, MotionEvent, PointerTarget, RelativeMotionEvent,
        },
        touch::{DownEvent, OrientationEvent, ShapeEvent, TouchTarget, UpEvent},
        Seat,
    },
    reexports::wayland_server::{backend::ObjectId, protocol::wl_surface::WlSurface},
    utils::{IsAlive, Serial},
    wayland::seat::WaylandFocus,
    xwayland::X11Surface,
};

use crate::state::Glower;

#[derive(Debug, Clone, PartialEq)]
pub enum Фокус {
    Wl(WlSurface),
    X11(X11Surface),
}

impl IsAlive for Фокус {
    fn alive(&self) -> bool {
        match self {
            Фокус::Wl(с) => с.alive(),
            Фокус::X11(х) => х.alive(),
        }
    }
}

impl WaylandFocus for Фокус {
    fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        match self {
            Фокус::Wl(с) => Some(Cow::Borrowed(с)),
            Фокус::X11(х) => х.wl_surface().map(Cow::Owned),
        }
    }
    fn same_client_as(&self, id: &ObjectId) -> bool {
        match self {
            Фокус::Wl(с) => с.same_client_as(id),
            Фокус::X11(х) => х.same_client_as(id),
        }
    }
}

impl From<WlSurface> for Фокус {
    fn from(с: WlSurface) -> Self {
        Фокус::Wl(с)
    }
}

impl From<PopupKind> for Фокус {
    fn from(п: PopupKind) -> Self {
        Фокус::Wl(п.wl_surface().clone())
    }
}

impl From<X11Surface> for Фокус {
    fn from(х: X11Surface) -> Self {
        Фокус::X11(х)
    }
}

/// Все вызовы — тому, кто внутри
macro_rules! передай {
    ($себя:ident, $цель:ident => $дело:expr) => {
        match $себя {
            Фокус::Wl($цель) => $дело,
            Фокус::X11($цель) => $дело,
        }
    };
}

impl KeyboardTarget<Glower> for Фокус {
    fn enter(&self, seat: &Seat<Glower>, data: &mut Glower, keys: Vec<KeysymHandle<'_>>, serial: Serial) {
        передай!(self, ц => KeyboardTarget::enter(ц, seat, data, keys, serial))
    }
    fn leave(&self, seat: &Seat<Glower>, data: &mut Glower, serial: Serial) {
        передай!(self, ц => KeyboardTarget::leave(ц, seat, data, serial))
    }
    fn key(&self, seat: &Seat<Glower>, data: &mut Glower, key: KeysymHandle<'_>, state: KeyState, serial: Serial, time: u32) {
        передай!(self, ц => KeyboardTarget::key(ц, seat, data, key, state, serial, time))
    }
    fn modifiers(&self, seat: &Seat<Glower>, data: &mut Glower, modifiers: ModifiersState, serial: Serial) {
        передай!(self, ц => KeyboardTarget::modifiers(ц, seat, data, modifiers, serial))
    }
}

impl PointerTarget<Glower> for Фокус {
    fn enter(&self, seat: &Seat<Glower>, data: &mut Glower, event: &MotionEvent) {
        передай!(self, ц => PointerTarget::enter(ц, seat, data, event))
    }
    fn motion(&self, seat: &Seat<Glower>, data: &mut Glower, event: &MotionEvent) {
        передай!(self, ц => PointerTarget::motion(ц, seat, data, event))
    }
    fn relative_motion(&self, seat: &Seat<Glower>, data: &mut Glower, event: &RelativeMotionEvent) {
        передай!(self, ц => PointerTarget::relative_motion(ц, seat, data, event))
    }
    fn button(&self, seat: &Seat<Glower>, data: &mut Glower, event: &ButtonEvent) {
        передай!(self, ц => PointerTarget::button(ц, seat, data, event))
    }
    fn axis(&self, seat: &Seat<Glower>, data: &mut Glower, frame: AxisFrame) {
        передай!(self, ц => PointerTarget::axis(ц, seat, data, frame))
    }
    fn frame(&self, seat: &Seat<Glower>, data: &mut Glower) {
        передай!(self, ц => PointerTarget::frame(ц, seat, data))
    }
    fn gesture_swipe_begin(&self, seat: &Seat<Glower>, data: &mut Glower, event: &GestureSwipeBeginEvent) {
        передай!(self, ц => PointerTarget::gesture_swipe_begin(ц, seat, data, event))
    }
    fn gesture_swipe_update(&self, seat: &Seat<Glower>, data: &mut Glower, event: &GestureSwipeUpdateEvent) {
        передай!(self, ц => PointerTarget::gesture_swipe_update(ц, seat, data, event))
    }
    fn gesture_swipe_end(&self, seat: &Seat<Glower>, data: &mut Glower, event: &GestureSwipeEndEvent) {
        передай!(self, ц => PointerTarget::gesture_swipe_end(ц, seat, data, event))
    }
    fn gesture_pinch_begin(&self, seat: &Seat<Glower>, data: &mut Glower, event: &GesturePinchBeginEvent) {
        передай!(self, ц => PointerTarget::gesture_pinch_begin(ц, seat, data, event))
    }
    fn gesture_pinch_update(&self, seat: &Seat<Glower>, data: &mut Glower, event: &GesturePinchUpdateEvent) {
        передай!(self, ц => PointerTarget::gesture_pinch_update(ц, seat, data, event))
    }
    fn gesture_pinch_end(&self, seat: &Seat<Glower>, data: &mut Glower, event: &GesturePinchEndEvent) {
        передай!(self, ц => PointerTarget::gesture_pinch_end(ц, seat, data, event))
    }
    fn gesture_hold_begin(&self, seat: &Seat<Glower>, data: &mut Glower, event: &GestureHoldBeginEvent) {
        передай!(self, ц => PointerTarget::gesture_hold_begin(ц, seat, data, event))
    }
    fn gesture_hold_end(&self, seat: &Seat<Glower>, data: &mut Glower, event: &GestureHoldEndEvent) {
        передай!(self, ц => PointerTarget::gesture_hold_end(ц, seat, data, event))
    }
    fn leave(&self, seat: &Seat<Glower>, data: &mut Glower, serial: Serial, time: u32) {
        передай!(self, ц => PointerTarget::leave(ц, seat, data, serial, time))
    }
}

impl TouchTarget<Glower> for Фокус {
    fn down(&self, seat: &Seat<Glower>, data: &mut Glower, event: &DownEvent, seq: Serial) {
        передай!(self, ц => TouchTarget::down(ц, seat, data, event, seq))
    }
    fn up(&self, seat: &Seat<Glower>, data: &mut Glower, event: &UpEvent, seq: Serial) {
        передай!(self, ц => TouchTarget::up(ц, seat, data, event, seq))
    }
    fn motion(&self, seat: &Seat<Glower>, data: &mut Glower, event: &smithay::input::touch::MotionEvent, seq: Serial) {
        передай!(self, ц => TouchTarget::motion(ц, seat, data, event, seq))
    }
    fn frame(&self, seat: &Seat<Glower>, data: &mut Glower, seq: Serial) {
        передай!(self, ц => TouchTarget::frame(ц, seat, data, seq))
    }
    fn cancel(&self, seat: &Seat<Glower>, data: &mut Glower, seq: Serial) {
        передай!(self, ц => TouchTarget::cancel(ц, seat, data, seq))
    }
    fn shape(&self, seat: &Seat<Glower>, data: &mut Glower, event: &ShapeEvent, seq: Serial) {
        передай!(self, ц => TouchTarget::shape(ц, seat, data, event, seq))
    }
    fn orientation(&self, seat: &Seat<Glower>, data: &mut Glower, event: &OrientationEvent, seq: Serial) {
        передай!(self, ц => TouchTarget::orientation(ц, seat, data, event, seq))
    }
}
