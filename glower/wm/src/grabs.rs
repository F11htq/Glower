//! Перетаскивание окна за заголовок и изменение размера за край

use smithay::{
    desktop::Window,
    input::pointer::{
        AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent, GesturePinchEndEvent,
        GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent, GestureSwipeUpdateEvent, GrabStartData,
        MotionEvent, PointerGrab, PointerInnerHandle, RelativeMotionEvent,
    },
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    utils::{Logical, Point, Rectangle, Size},
};

use crate::{focus::Фокус, state::Glower};

/// Общая часть захватов: всё, кроме движения и кнопок, передаём как есть
macro_rules! прочее_как_есть {
    () => {
        fn relative_motion(
            &mut self,
            data: &mut Glower,
            handle: &mut PointerInnerHandle<'_, Glower>,
            focus: Option<(Фокус, Point<f64, Logical>)>,
            event: &RelativeMotionEvent,
        ) {
            handle.relative_motion(data, focus, event);
        }
        fn axis(&mut self, data: &mut Glower, handle: &mut PointerInnerHandle<'_, Glower>, details: AxisFrame) {
            handle.axis(data, details)
        }
        fn frame(&mut self, data: &mut Glower, handle: &mut PointerInnerHandle<'_, Glower>) {
            handle.frame(data);
        }
        fn gesture_swipe_begin(&mut self, data: &mut Glower, handle: &mut PointerInnerHandle<'_, Glower>, event: &GestureSwipeBeginEvent) {
            handle.gesture_swipe_begin(data, event)
        }
        fn gesture_swipe_update(&mut self, data: &mut Glower, handle: &mut PointerInnerHandle<'_, Glower>, event: &GestureSwipeUpdateEvent) {
            handle.gesture_swipe_update(data, event)
        }
        fn gesture_swipe_end(&mut self, data: &mut Glower, handle: &mut PointerInnerHandle<'_, Glower>, event: &GestureSwipeEndEvent) {
            handle.gesture_swipe_end(data, event)
        }
        fn gesture_pinch_begin(&mut self, data: &mut Glower, handle: &mut PointerInnerHandle<'_, Glower>, event: &GesturePinchBeginEvent) {
            handle.gesture_pinch_begin(data, event)
        }
        fn gesture_pinch_update(&mut self, data: &mut Glower, handle: &mut PointerInnerHandle<'_, Glower>, event: &GesturePinchUpdateEvent) {
            handle.gesture_pinch_update(data, event)
        }
        fn gesture_pinch_end(&mut self, data: &mut Glower, handle: &mut PointerInnerHandle<'_, Glower>, event: &GesturePinchEndEvent) {
            handle.gesture_pinch_end(data, event)
        }
        fn gesture_hold_begin(&mut self, data: &mut Glower, handle: &mut PointerInnerHandle<'_, Glower>, event: &GestureHoldBeginEvent) {
            handle.gesture_hold_begin(data, event)
        }
        fn gesture_hold_end(&mut self, data: &mut Glower, handle: &mut PointerInnerHandle<'_, Glower>, event: &GestureHoldEndEvent) {
            handle.gesture_hold_end(data, event)
        }
        fn start_data(&self) -> &GrabStartData<Glower> {
            &self.start_data
        }
    };
}

pub struct Перенос {
    pub start_data: GrabStartData<Glower>,
    pub окно: Window,
    pub начало: Point<i32, Logical>,
}

impl PointerGrab<Glower> for Перенос {
    fn motion(&mut self, data: &mut Glower, handle: &mut PointerInnerHandle<'_, Glower>, _focus: Option<(Фокус, Point<f64, Logical>)>, event: &MotionEvent) {
        // Пока тащим — фокус ничей, иначе окна под мышью получали бы движение
        handle.motion(data, None, event);
        let сдвиг = event.location - self.start_data.location;
        let новое = self.начало.to_f64() + сдвиг;
        // Развёрнутое окно, которое потащили, — сначала свернуть в прежний размер
        let развёрнуто = data.запись(&self.окно).map(|з| з.развёрнуто).unwrap_or(false);
        if развёрнуто && сдвиг.x.abs() + сдвиг.y.abs() > 12.0 {
            let прежнее = data.запись_мут(&self.окно).and_then(|з| {
                з.развёрнуто = false;
                з.прежнее.take()
            });
            if let Some(п) = прежнее {
                // Окно остаётся под мышью той же долей своей ширины
                let доля = (self.start_data.location.x - self.начало.x as f64) / data.space.element_geometry(&self.окно).map(|г| г.size.w as f64).unwrap_or(1.0).max(1.0);
                let x = event.location.x - п.size.w as f64 * доля;
                self.начало = Point::from((x as i32, self.начало.y));
                self.start_data.location = event.location;
                let куда = Rectangle::new(self.начало, п.size);
                data.поставь(&self.окно, куда, Some(false));
            }
            return;
        }
        data.space.map_element(self.окно.clone(), новое.to_i32_round(), true);
        data.нужен_кадр = true;
    }

    fn button(&mut self, data: &mut Glower, handle: &mut PointerInnerHandle<'_, Glower>, event: &ButtonEvent) {
        handle.button(data, event);
        if !handle.current_pressed().contains(&self.start_data.button) {
            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    fn unset(&mut self, data: &mut Glower) {
        // Окно у верхнего края — развернуть, как в Windows
        let Some(гео) = data.space.element_geometry(&self.окно) else { return };
        let мышь = data.мышь;
        if let Some(экран) = data.главный_экран() {
            let область = data.рабочая_область(&экран);
            if мышь.y <= (область.loc.y + 2) as f64 && гео.loc.y <= область.loc.y + 4 {
                let окно = self.окно.clone();
                data.разверни(&окно);
            }
        }
    }

    прочее_как_есть!();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Края(pub u32);

impl Края {
    pub const ВЕРХ: u32 = 1;
    pub const НИЗ: u32 = 2;
    pub const ЛЕВО: u32 = 4;
    pub const ПРАВО: u32 = 8;

    pub fn из_xdg(к: xdg_toplevel::ResizeEdge) -> Self {
        use xdg_toplevel::ResizeEdge as Р;
        Края(match к {
            Р::Top => Self::ВЕРХ,
            Р::Bottom => Self::НИЗ,
            Р::Left => Self::ЛЕВО,
            Р::Right => Self::ПРАВО,
            Р::TopLeft => Self::ВЕРХ | Self::ЛЕВО,
            Р::TopRight => Self::ВЕРХ | Self::ПРАВО,
            Р::BottomLeft => Self::НИЗ | Self::ЛЕВО,
            Р::BottomRight => Self::НИЗ | Self::ПРАВО,
            _ => 0,
        })
    }

    pub fn есть(&self, к: u32) -> bool {
        self.0 & к != 0
    }
}

pub struct Размер {
    pub start_data: GrabStartData<Glower>,
    pub окно: Window,
    pub края: Края,
    pub начало: Rectangle<i32, Logical>,
    pub последний: Size<i32, Logical>,
}

impl PointerGrab<Glower> for Размер {
    fn motion(&mut self, data: &mut Glower, handle: &mut PointerInnerHandle<'_, Glower>, _focus: Option<(Фокус, Point<f64, Logical>)>, event: &MotionEvent) {
        handle.motion(data, None, event);
        let mut д = event.location - self.start_data.location;
        let mut ш = self.начало.size.w;
        let mut в = self.начало.size.h;
        if self.края.есть(Края::ЛЕВО) {
            д.x = -д.x;
        }
        if self.края.есть(Края::ЛЕВО) || self.края.есть(Края::ПРАВО) {
            ш = (ш as f64 + д.x) as i32;
        }
        if self.края.есть(Края::ВЕРХ) {
            д.y = -д.y;
        }
        if self.края.есть(Края::ВЕРХ) || self.края.есть(Края::НИЗ) {
            в = (в as f64 + д.y) as i32;
        }
        let (мин, макс) = пределы(&self.окно);
        ш = ш.max(мин.w.max(120));
        в = в.max(мин.h.max(60));
        if макс.w > 0 {
            ш = ш.min(макс.w);
        }
        if макс.h > 0 {
            в = в.min(макс.h);
        }
        self.последний = Size::from((ш, в));
        if let Some(t) = self.окно.toplevel() {
            t.with_pending_state(|с| {
                с.states.set(xdg_toplevel::State::Resizing);
                с.size = Some(self.последний);
            });
            t.send_pending_configure();
        } else if let Some(x) = self.окно.x11_surface() {
            let mut гео = x.geometry();
            гео.size = self.последний;
            if self.края.есть(Края::ЛЕВО) {
                гео.loc.x = self.начало.loc.x + self.начало.size.w - ш;
            }
            if self.края.есть(Края::ВЕРХ) {
                гео.loc.y = self.начало.loc.y + self.начало.size.h - в;
            }
            let _ = x.configure(гео);
        }
        // Тянем за левый или верхний край — окно растёт влево или вверх:
        // место пересчитываем по готовому размеру при его кадре (см. commit)
        data.изменяемые.retain(|(о, ..)| о != &self.окно);
        data.изменяемые.push((self.окно.clone(), self.края, self.начало));
        data.нужен_кадр = true;
    }

    fn button(&mut self, data: &mut Glower, handle: &mut PointerInnerHandle<'_, Glower>, event: &ButtonEvent) {
        handle.button(data, event);
        if !handle.current_pressed().contains(&self.start_data.button) {
            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    fn unset(&mut self, data: &mut Glower) {
        if let Some(t) = self.окно.toplevel() {
            t.with_pending_state(|с| {
                с.states.unset(xdg_toplevel::State::Resizing);
                с.size = Some(self.последний);
            });
            t.send_pending_configure();
        }
    }

    прочее_как_есть!();
}

/// Наименьший и наибольший размер, о которых просит программа
fn пределы(окно: &Window) -> (Size<i32, Logical>, Size<i32, Logical>) {
    if let Some(t) = окно.toplevel() {
        return smithay::wayland::compositor::with_states(t.wl_surface(), |с| {
            let mut данные = с.cached_state.get::<smithay::wayland::shell::xdg::SurfaceCachedState>();
            let тек = данные.current();
            (тек.min_size, тек.max_size)
        });
    }
    if let Some(x) = окно.x11_surface() {
        return (x.min_size().unwrap_or_default(), x.max_size().unwrap_or_default());
    }
    (Size::default(), Size::default())
}

/// Окно растёт влево или вверх — после нового кадра двигаем его так, чтобы
/// противоположный край остался на месте
pub fn поправь_место(data: &mut Glower, окно: &Window) {
    let Some(i) = data.изменяемые.iter().position(|(о, ..)| о == окно) else { return };
    let (_, края, начало) = data.изменяемые[i];
    let размер = окно.geometry().size;
    let Some(mut где) = data.space.element_location(окно) else { return };
    if края.есть(Края::ЛЕВО) {
        где.x = начало.loc.x + начало.size.w - размер.w;
    }
    if края.есть(Края::ВЕРХ) {
        где.y = начало.loc.y + начало.size.h - размер.h;
    }
    data.space.map_element(окно.clone(), где, false);
    // Захват кончился — больше не следим
    if !data.seat.get_pointer().map(|у| у.is_grabbed()).unwrap_or(false) {
        data.изменяемые.remove(i);
    }
}
