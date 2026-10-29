//! Клавиатура, мышь, тачпад: сочетания сервера и передача программам

use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, GestureBeginEvent, GestureEndEvent, GestureSwipeUpdateEvent,
        InputBackend, InputEvent, KeyState, KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
    },
    input::{
        keyboard::{keysyms, FilterResult},
        pointer::{AxisFrame, ButtonEvent, MotionEvent, RelativeMotionEvent},
    },
    utils::{Logical, Point, SERIAL_COUNTER},
    wayland::{
        pointer_constraints::{with_pointer_constraint, PointerConstraint},
        seat::WaylandFocus,
        shell::wlr_layer::{KeyboardInteractivity, Layer as WlrLayer},
    },
};

use crate::{
    focus::Фокус,
    keys::{Дело, Моды},
    state::Glower,
    xwayland::Кнопка,
};

/// Левая кнопка мыши в кодах ядра (BTN_LEFT)
const ЛЕВАЯ: u32 = 0x110;

impl Glower {
    pub fn ввод<B: InputBackend>(&mut self, событие: InputEvent<B>) {
        // Любое движение — не простой: экран не гаснет, пока человек тут
        self.idle_notifier_state.notify_activity(&self.seat);
        match событие {
            InputEvent::Keyboard { event } => self.клавиша(event.key_code(), event.state(), Event::time_msec(&event)),
            InputEvent::PointerMotion { event } => {
                let у = match self.seat.get_pointer() {
                    Some(у) => у,
                    None => return,
                };
                let было = self.мышь;
                let сдвиг = event.delta();
                let под = self.под_мышью(было);
                // Игра держит мышь у себя — указатель стоит, движение уходит ей
                let mut заперт = false;
                if let Some((ф, _)) = под.as_ref() {
                    if let Some(с) = ф.wl_surface() {
                        with_pointer_constraint(&с, &у, |к| {
                            if let Some(к) = к {
                                if к.is_active() {
                                    if let PointerConstraint::Locked(_) = &*к {
                                        заперт = true;
                                    }
                                }
                            }
                        });
                    }
                }
                let serial = SERIAL_COUNTER.next_serial();
                у.relative_motion(
                    self,
                    под.clone(),
                    &RelativeMotionEvent { delta: сдвиг, delta_unaccel: event.delta_unaccel(), utime: event.time() },
                );
                if !заперт {
                    let новое = self.в_пределах(было + сдвиг);
                    self.мышь = новое;
                    let под = self.под_мышью(новое);
                    let под = self.без_рамки(новое, под);
                    let под = self.только_разрешённым(под);
                    у.motion(self, под.clone(), &MotionEvent { location: новое, serial, time: event.time_msec() });
                    self.дошли_до_новой(&у, под, новое, event.time_msec());
                }
                у.frame(self);
                self.нужен_кадр = true;
            }
            InputEvent::PointerMotionAbsolute { event } => {
                let Some(экран) = self.space.outputs().next().cloned() else { return };
                let гео = self.space.output_geometry(&экран).unwrap_or_default();
                let где = event.position_transformed(гео.size) + гео.loc.to_f64();
                self.мышь = где;
                let serial = SERIAL_COUNTER.next_serial();
                let под = self.под_мышью(где);
                let под = self.без_рамки(где, под);
                let под = self.только_разрешённым(под);
                if let Some(у) = self.seat.get_pointer() {
                    у.motion(self, под.clone(), &MotionEvent { location: где, serial, time: event.time_msec() });
                    self.дошли_до_новой(&у, под, где, event.time_msec());
                    у.frame(self);
                }
                self.нужен_кадр = true;
            }
            InputEvent::PointerButton { event } => self.кнопка(event.button_code(), event.state(), event.time_msec()),
            InputEvent::PointerAxis { event } => {
                let источник = event.source();
                let по_гор = event.amount(Axis::Horizontal).unwrap_or_else(|| event.amount_v120(Axis::Horizontal).unwrap_or(0.0) * 15.0 / 120.0);
                let по_верт = event.amount(Axis::Vertical).unwrap_or_else(|| event.amount_v120(Axis::Vertical).unwrap_or(0.0) * 15.0 / 120.0);
                let mut кадр = AxisFrame::new(event.time_msec()).source(источник);
                if по_гор != 0.0 {
                    кадр = кадр.value(Axis::Horizontal, по_гор);
                    if let Some(д) = event.amount_v120(Axis::Horizontal) {
                        кадр = кадр.v120(Axis::Horizontal, д as i32);
                    }
                }
                if по_верт != 0.0 {
                    кадр = кадр.value(Axis::Vertical, по_верт);
                    if let Some(д) = event.amount_v120(Axis::Vertical) {
                        кадр = кадр.v120(Axis::Vertical, д as i32);
                    }
                }
                if источник == AxisSource::Finger {
                    if event.amount(Axis::Horizontal) == Some(0.0) {
                        кадр = кадр.stop(Axis::Horizontal);
                    }
                    if event.amount(Axis::Vertical) == Some(0.0) {
                        кадр = кадр.stop(Axis::Vertical);
                    }
                }
                if let Some(у) = self.seat.get_pointer() {
                    у.axis(self, кадр);
                    у.frame(self);
                }
            }
            // Жесты тачпада тремя-четырьмя пальцами — как в Windows: в
            // стороны — соседний рабочий стол, вверх — просмотр задач, вниз —
            // рабочий стол. Двумя пальцами libinput жестов не шлёт — это
            // прокрутка, и она идёт программам как обычно
            InputEvent::GestureSwipeBegin { event } => {
                self.жест = (event.fingers() >= 3).then(Point::default);
            }
            InputEvent::GestureSwipeUpdate { event } => {
                if let Some(путь) = self.жест.as_mut() {
                    *путь += event.delta();
                }
            }
            InputEvent::GestureSwipeEnd { event } => {
                let путь = self.жест.take();
                if let (Some(путь), false) = (путь, event.cancelled()) {
                    if let Some(команда) = команда_жеста(путь) {
                        crate::input::запусти(команда);
                    }
                }
            }
            _ => {}
        }
    }

    /// Мышь не уходит за края экранов
    fn в_пределах(&self, точка: Point<f64, Logical>) -> Point<f64, Logical> {
        if self.space.output_under(точка).next().is_some() {
            return точка;
        }
        let mut лучшее = точка;
        let mut наименьшее = f64::MAX;
        for э in self.space.outputs() {
            let Some(г) = self.space.output_geometry(э) else { continue };
            let x = точка.x.clamp(г.loc.x as f64, (г.loc.x + г.size.w - 1) as f64);
            let y = точка.y.clamp(г.loc.y as f64, (г.loc.y + г.size.h - 1) as f64);
            let д = (x - точка.x).powi(2) + (y - точка.y).powi(2);
            if д < наименьшее {
                наименьшее = д;
                лучшее = Point::from((x, y));
            }
        }
        лучшее
    }

    /// Над рамкой окна X11 мышь ничья: рамку рисует сервер, и указатель —
    /// обычная стрелка, а не тот, что выбрала программа под ней
    fn без_рамки(&mut self, где: Point<f64, Logical>, под: Option<(Фокус, Point<f64, Logical>)>) -> Option<(Фокус, Point<f64, Logical>)> {
        if self.замок.is_none() && self.рамка_под(где).is_some() {
            self.cursor_status = smithay::input::pointer::CursorImageStatus::default_named();
            return None;
        }
        под
    }

    /// Пока мышь захвачена (несут файл), новой поверхности сервер шлёт
    /// только «вошли» — а GTK отвечает лишь на «двинулись». Влетел курсор
    /// на тонкую полоску у края одним рывком и замер — программа ничего и
    /// не узнала (док не выезжал под файлом). Досылаем движение сразу
    fn дошли_до_новой(
        &mut self,
        у: &smithay::input::pointer::PointerHandle<Self>,
        под: Option<(Фокус, Point<f64, Logical>)>,
        где: Point<f64, Logical>,
        время: u32,
    ) {
        let новая = под.as_ref().and_then(|(ф, _)| ф.wl_surface().map(|с| с.into_owned()));
        if у.is_grabbed() && новая.is_some() && новая != self.фокус_захвата {
            у.motion(self, под, &MotionEvent { location: где, serial: SERIAL_COUNTER.next_serial(), time: время });
        }
        self.фокус_захвата = новая;
    }

    /// Ввод заперт замком — мышь видит только его
    fn только_разрешённым(&self, под: Option<(Фокус, Point<f64, Logical>)>) -> Option<(Фокус, Point<f64, Logical>)> {
        if self.inhibitor_state.кто.is_none() {
            return под;
        }
        под.filter(|(ф, _)| {
            let клиент = ф.wl_surface().and_then(|с| Glower::клиент_поверхности(&с));
            self.inhibitor_state.можно(клиент.as_ref())
        })
    }

    fn клавиша(&mut self, код: smithay::input::keyboard::Keycode, нажато: KeyState, время: u32) {
        let Some(клавиатура) = self.seat.get_keyboard() else { return };
        let serial = SERIAL_COUNTER.next_serial();
        let заперто = self.замок.is_some() || self.inhibitor_state.кто.is_some();
        let дела = клавиатура.input::<Vec<Дело>, _>(self, код, нажато, serial, время, |с, моды, ручка| {
            if заперто {
                return FilterResult::Forward;
            }
            let код_клавиши = ручка.raw_code();
            // Отпускание той клавиши, нажатие которой забрал сервер, — тоже ему
            if нажато == KeyState::Released {
                if let Some(i) = с.подавлены.iter().position(|к| *к == код_клавиши) {
                    с.подавлены.remove(i);
                    return FilterResult::Intercept(vec![]);
                }
            }
            let Some(символ) = ручка.raw_latin_sym_or_raw_current_sym() else { return FilterResult::Forward };
            tracing::debug!("клавиша {:?} {:?}, моды {:?}", символ, нажато, Моды::from(моды));
            // Ctrl+Alt+Fn: xkb сам превращает их в XF86Switch_VT_n
            let код = символ.raw();
            if (keysyms::KEY_XF86Switch_VT_1..=keysyms::KEY_XF86Switch_VT_12).contains(&код) && нажато == KeyState::Pressed {
                с.подавлены.push(код_клавиши);
                return FilterResult::Intercept(vec![Дело::Консоль((код - keysyms::KEY_XF86Switch_VT_1 + 1) as i32)]);
            }
            let super_ли = matches!(символ.raw(), keysyms::KEY_Super_L | keysyms::KEY_Super_R);
            let м = Моды::from(моды);
            match нажато {
                KeyState::Pressed => {
                    с.одинокая_super = super_ли && !м.ctrl && !м.alt && !м.shift;
                    if super_ли {
                        return FilterResult::Forward;
                    }
                    let найдено = с.сочетания.iter().find(|сч| {
                        !сч.одинокая && сч.моды == м && сч.клавиша.raw() == нижний(символ.raw())
                    });
                    if let Some(сч) = найдено {
                        let дела = сч.дела.clone();
                        с.подавлены.push(код_клавиши);
                        return FilterResult::Intercept(дела);
                    }
                    FilterResult::Forward
                }
                KeyState::Released => {
                    if super_ли && с.одинокая_super {
                        с.одинокая_super = false;
                        if let Some(сч) = с.сочетания.iter().find(|сч| сч.одинокая && сч.клавиша.raw() == символ.raw()) {
                            return FilterResult::Intercept(сч.дела.clone());
                        }
                    }
                    FilterResult::Forward
                }
            }
        });
        if let Some(дела) = дела {
            for д in дела {
                self.выполни(д);
            }
        }
        // Alt+Shift мог сменить раскладку
        self.запиши_раскладку();
    }

    pub fn выполни(&mut self, дело: Дело) {
        let в_фокусе = self.окно_в_фокусе();
        match дело {
            Дело::Запусти(команда) => запусти(&команда),
            Дело::Закрой => {
                match в_фокусе {
                    Some(о) => self.закрой(&о),
                    // Окна в фокусе нет — значит, человек на рабочем столе, и
                    // Alt+F4 там, как в Windows, — «Завершение работы»
                    None => запусти("glower-menu клавиша A-F4"),
                }
            }
            Дело::Следующее => self.следующее(),
            Дело::ККраю(влево) => {
                if let Some(о) = в_фокусе {
                    self.к_краю(&о, влево);
                }
            }
            Дело::Развернуть => {
                if let Some(о) = в_фокусе {
                    if о.x11_surface().is_some() {
                        self.разверни_x11(&о);
                    } else {
                        self.разверни(&о);
                    }
                }
            }
            Дело::Свернуть => {
                if let Some(о) = в_фокусе {
                    self.сверни(&о);
                }
            }
            Дело::НаСтол(n) => self.на_стол(n),
            Дело::ОтправьНаСтол(n) => {
                if let Some(о) = в_фокусе {
                    self.отправь_на_стол(&о, n);
                }
            }
            Дело::Консоль(n) => {
                if let Some(crate::backend::Бэкенд::Udev(u)) = self.бэкенд.as_mut() {
                    u.смени_консоль(n);
                }
            }
            Дело::Ничего => {}
        }
    }

    fn кнопка(&mut self, код: u32, состояние: ButtonState, время: u32) {
        let Some(у) = self.seat.get_pointer() else { return };
        let serial = SERIAL_COUNTER.next_serial();
        let где = self.мышь;
        if состояние == ButtonState::Pressed && !у.is_grabbed() && self.замок.is_none() && self.inhibitor_state.кто.is_none() {
            // Слои панели сверху — им и клавиатуру, если просят
            if let Some((_, _, слой)) = self.слой_под(где, self.слои_над_окнами()) {
                if слой.cached_state().keyboard_interactivity != KeyboardInteractivity::None {
                    self.фокус_слою(&слой);
                }
            } else if let Some((окно, к)) = self.рамка_под(где) {
                self.дай_фокус(Some(&окно));
                match к {
                    Кнопка::Закрыть => self.закрой(&окно),
                    Кнопка::Развернуть => self.разверни_x11(&окно),
                    Кнопка::Свернуть => self.сверни(&окно),
                    Кнопка::Заголовок => {
                        if код == ЛЕВАЯ {
                            self.перенос_мышью(окно, serial, код);
                        }
                    }
                }
                return;
            } else if let Some((окно, _)) = self.space.element_under(где).map(|(о, м)| (о.clone(), м)) {
                self.дай_фокус(Some(&окно));
                // Super + левая кнопка — тащить окно за любое место
                let super_зажата = self.seat.get_keyboard().map(|к| к.modifier_state().logo).unwrap_or(false);
                if super_зажата && код == ЛЕВАЯ {
                    self.перенос_мышью(окно, serial, код);
                    return;
                }
            } else if let Some((_, _, слой)) = self.слой_под(где, &[WlrLayer::Bottom, WlrLayer::Background]) {
                // Щелчок по столу — фокус уходит с окон
                if слой.cached_state().keyboard_interactivity != KeyboardInteractivity::None {
                    self.фокус_слою(&слой);
                } else {
                    self.дай_фокус(None);
                }
            }
        }
        у.button(self, &ButtonEvent { button: код, state: состояние, serial, time: время });
        у.frame(self);
    }
}

/// Для сравнения с rc.xml: буквы там строчные
fn нижний(с: u32) -> u32 {
    if (keysyms::KEY_A..=keysyms::KEY_Z).contains(&с) {
        с + (keysyms::KEY_a - keysyms::KEY_A)
    } else {
        с
    }
}

/// Команда из rc.xml — через sh, как у labwc; ждём выхода в стороне
pub fn запусти(команда: &str) {
    match std::process::Command::new("sh").arg("-c").arg(команда).spawn() {
        Ok(mut ребёнок) => {
            std::thread::spawn(move || ребёнок.wait());
        }
        Err(е) => tracing::warn!("не запустилось «{команда}»: {е}"),
    }
}

/// Что сделать по жесту: путь пальцев за жест (в точках тачпада). Короткий
/// или косой жест — ничего: случайное касание не должно уводить со стола
pub fn команда_жеста(путь: Point<f64, Logical>) -> Option<&'static str> {
    const ПОРОГ: f64 = 90.0;
    let (x, y) = (путь.x, путь.y);
    if x.abs() >= ПОРОГ && x.abs() > y.abs() * 1.5 {
        // Пальцы влево — содержимое уходит влево, приходит стол справа
        Some(if x < 0.0 { "glower-desk вперёд" } else { "glower-desk назад" })
    } else if y.abs() >= ПОРОГ && y.abs() > x.abs() * 1.5 {
        Some(if y < 0.0 { "glower-menu задачи" } else { "glower-menu клавиша W-d" })
    } else {
        None
    }
}

#[cfg(test)]
mod жесты {
    use super::*;

    #[test]
    fn жесты_по_направлению() {
        assert_eq!(команда_жеста(Point::from((-200.0, 10.0))), Some("glower-desk вперёд"));
        assert_eq!(команда_жеста(Point::from((200.0, -20.0))), Some("glower-desk назад"));
        assert_eq!(команда_жеста(Point::from((5.0, -150.0))), Some("glower-menu задачи"));
        assert_eq!(команда_жеста(Point::from((0.0, 150.0))), Some("glower-menu клавиша W-d"));
        assert_eq!(команда_жеста(Point::from((40.0, 10.0))), None, "короткий — мимо");
        assert_eq!(команда_жеста(Point::from((120.0, 110.0))), None, "косой — мимо");
    }
}
