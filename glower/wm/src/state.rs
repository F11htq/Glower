//! Всё, что знает оконный сервер: окна и их столы, слои панели, фокус,
//! замок экрана, сочетания клавиш

use std::{
    ffi::OsString,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use smithay::{
    desktop::{
        layer_map_for_output, LayerSurface, PopupManager, Space, Window,
        WindowSurfaceType,
    },
    input::{keyboard::XkbConfig, pointer::CursorImageStatus, Seat, SeatState},
    output::Output,
    reexports::{
        calloop::{generic::Generic, Interest, LoopHandle, LoopSignal, Mode, PostAction},
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::wl_surface::WlSurface,
            Display, DisplayHandle, Resource,
        },
    },
    utils::{Logical, Point, Rectangle, Size, SERIAL_COUNTER},
    wayland::{
        compositor::{CompositorClientState, CompositorState},
        cursor_shape::CursorShapeManagerState,
        fractional_scale::FractionalScaleManagerState,
        idle_notify::IdleNotifierState,
        output::OutputManagerState,
        presentation::PresentationState,
        selection::{
            data_device::DataDeviceState, primary_selection::PrimarySelectionState,
            wlr_data_control::DataControlState,
        },
        session_lock::{LockSurface, SessionLockManagerState},
        shell::{
            wlr_layer::{KeyboardInteractivity, Layer as WlrLayer, WlrLayerShellState},
            xdg::{decoration::XdgDecorationState, XdgShellState},
        },
        shm::ShmState,
        single_pixel_buffer::SinglePixelBufferState,
        socket::ListeningSocketSource,
        viewporter::ViewporterState,
        xdg_activation::XdgActivationState,
        xwayland_shell::XWaylandShellState,
    },
    xwayland::{X11Wm, X11Surface},
};

use crate::{
    anim::Анимации,
    focus::Фокус,
    keys::{self, Сочетание},
    protocols::{foreign_toplevel::ForeignToplevelState, gamma::GammaState, inhibitor::InhibitorState, screencopy::ScreencopyState},
};

/// Окно и то, что о нём помнит сервер сверх самого окна
#[derive(Debug)]
pub struct Запись {
    pub окно: Window,
    pub стол: usize,
    pub свёрнуто: bool,
    /// Где стояло, пока его нет на экране (другой стол, свёрнуто)
    pub место: Point<i32, Logical>,
    /// Размер и место до разворачивания или прилипания к краю
    pub прежнее: Option<Rectangle<i32, Logical>>,
    pub развёрнуто: bool,
    pub во_весь_экран: bool,
}

pub struct Замок {
    pub поверхности: Vec<(Output, LockSurface)>,
}

pub struct Glower {
    pub display_handle: DisplayHandle,
    pub loop_handle: LoopHandle<'static, Glower>,
    pub loop_signal: LoopSignal,
    pub socket_name: OsString,
    pub start_time: Instant,

    pub space: Space<Window>,
    pub окна: Vec<Запись>,
    /// Окна, которые уже есть, но ещё ни разу не нарисовались: им ставим место
    /// при первом кадре, когда известен их размер
    pub ждут: Vec<Window>,
    pub стол: usize,
    pub popups: PopupManager,

    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub xdg_decoration_state: XdgDecorationState,
    pub layer_shell_state: WlrLayerShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Glower>,
    pub data_device_state: DataDeviceState,
    pub primary_selection_state: PrimarySelectionState,
    pub data_control_state: DataControlState,
    pub viewporter_state: ViewporterState,
    pub fractional_scale_state: FractionalScaleManagerState,
    pub xdg_activation_state: XdgActivationState,
    pub presentation_state: PresentationState,
    pub cursor_shape_state: CursorShapeManagerState,
    pub session_lock_state: SessionLockManagerState,
    pub idle_notifier_state: IdleNotifierState<Glower>,
    pub single_pixel_state: SinglePixelBufferState,
    pub xwayland_shell_state: XWaylandShellState,
    pub foreign_toplevel_state: ForeignToplevelState,
    pub inhibitor_state: InhibitorState,
    pub screencopy_state: ScreencopyState,
    pub gamma_state: GammaState,

    pub seat: Seat<Glower>,
    pub cursor_status: CursorImageStatus,
    /// Где мышь. Своя копия: у указателя спрашивать нельзя, пока идёт
    /// захват — его замок уже взят, и сервер повис бы
    pub мышь: Point<f64, Logical>,

    pub замок: Option<Замок>,
    pub сочетания: Vec<Сочетание>,
    pub папка_настроек: PathBuf,
    /// Нажата одна Super и ещё ничего вместе с ней
    pub одинокая_super: bool,
    pub раскладка: String,

    pub xwm: Option<X11Wm>,
    pub xdisplay: Option<u32>,

    /// Окна, которые сейчас тянут за край: им поправляем место после кадра
    pub изменяемые: Vec<(Window, crate::grabs::Края, Rectangle<i32, Logical>)>,
    pub анимации: Анимации,
    pub бэкенд: Option<crate::backend::Бэкенд>,
    /// Клавиши, нажатие которых забрал сервер: их отпускание программам не шлём
    pub подавлены: Vec<smithay::input::keyboard::Keycode>,
    /// Сколько кадров подряд надо перерисовать: анимации, новые окна
    pub нужен_кадр: bool,
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

impl Glower {
    pub fn new(
        display: Display<Glower>,
        loop_handle: LoopHandle<'static, Glower>,
        loop_signal: LoopSignal,
        папка_настроек: PathBuf,
        seat_name: &str,
    ) -> Self {
        let dh = display.handle();

        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let xdg_decoration_state = XdgDecorationState::new::<Self>(&dh);
        let layer_shell_state = WlrLayerShellState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let primary_selection_state = PrimarySelectionState::new::<Self>(&dh);
        let data_control_state = DataControlState::new::<Self, _>(&dh, Some(&primary_selection_state), |_| true);
        let viewporter_state = ViewporterState::new::<Self>(&dh);
        let fractional_scale_state = FractionalScaleManagerState::new::<Self>(&dh);
        let xdg_activation_state = XdgActivationState::new::<Self>(&dh);
        let presentation_state = PresentationState::new::<Self>(&dh, libc_clock_monotonic());
        let cursor_shape_state = CursorShapeManagerState::new::<Self>(&dh);
        let session_lock_state = SessionLockManagerState::new::<Self, _>(&dh, |_| true);
        let idle_notifier_state = IdleNotifierState::new(&dh, loop_handle.clone());
        let single_pixel_state = SinglePixelBufferState::new::<Self>(&dh);
        let xwayland_shell_state = XWaylandShellState::new::<Self>(&dh);
        let foreign_toplevel_state = ForeignToplevelState::new(&dh);
        let inhibitor_state = InhibitorState::new(&dh);
        let screencopy_state = ScreencopyState::new(&dh);
        let gamma_state = GammaState::new(&dh);
        smithay::wayland::relative_pointer::RelativePointerManagerState::new::<Self>(&dh);
        smithay::wayland::pointer_constraints::PointerConstraintsState::new::<Self>(&dh);
        smithay::wayland::virtual_keyboard::VirtualKeyboardManagerState::new::<Self, _>(&dh, |_| true);

        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, seat_name);
        let пустая = XkbConfig::default();
        seat.add_keyboard(пустая, 600, 25).expect("клавиатура");
        seat.add_pointer();

        let socket_name = Self::слушай(display, &loop_handle);

        let mut состояние = Glower {
            display_handle: dh,
            loop_handle,
            loop_signal,
            socket_name,
            start_time: Instant::now(),
            space: Space::default(),
            окна: vec![],
            ждут: vec![],
            стол: 0,
            popups: PopupManager::default(),
            compositor_state,
            xdg_shell_state,
            xdg_decoration_state,
            layer_shell_state,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            primary_selection_state,
            data_control_state,
            viewporter_state,
            fractional_scale_state,
            xdg_activation_state,
            presentation_state,
            cursor_shape_state,
            session_lock_state,
            idle_notifier_state,
            single_pixel_state,
            xwayland_shell_state,
            foreign_toplevel_state,
            inhibitor_state,
            screencopy_state,
            gamma_state,
            seat,
            cursor_status: CursorImageStatus::default_named(),
            мышь: Point::from((640.0, 400.0)),
            замок: None,
            сочетания: vec![],
            папка_настроек,
            одинокая_super: false,
            раскладка: String::new(),
            xwm: None,
            xdisplay: None,
            изменяемые: vec![],
            анимации: Анимации::default(),
            бэкенд: None,
            подавлены: vec![],
            нужен_кадр: true,
        };
        состояние.перечитай_настройки();
        состояние
    }

    fn слушай(display: Display<Glower>, loop_handle: &LoopHandle<'static, Glower>) -> OsString {
        let гнездо = ListeningSocketSource::new_auto().expect("гнездо Wayland");
        let имя = гнездо.socket_name().to_os_string();
        loop_handle
            .insert_source(гнездо, |поток, _, с| {
                if let Err(е) = с.display_handle.insert_client(поток, Arc::new(ClientState::default())) {
                    tracing::warn!("не принял программу: {е}");
                }
            })
            .expect("слушать Wayland");
        loop_handle
            .insert_source(Generic::new(display, Interest::READ, Mode::Level), |_, display, с| {
                // Safety: экран не уничтожаем, пока жив цикл событий
                unsafe {
                    display.get_mut().dispatch_clients(с).ok();
                }
                Ok(PostAction::Continue)
            })
            .expect("цикл Wayland");
        имя
    }

    /// rc.xml и environment из папки настроек: сочетания и раскладки.
    /// Зовётся при запуске и по SIGHUP — как у labwc
    pub fn перечитай_настройки(&mut self) {
        let rc = std::fs::read_to_string(self.папка_настроек.join("rc.xml")).unwrap_or_default();
        self.сочетания = keys::разбери(&rc);
        let среда = std::fs::read_to_string(self.папка_настроек.join("environment")).unwrap_or_default();
        let пары = keys::среда(&среда);
        let взять = |имя: &str| {
            пары.iter()
                .rev()
                .find(|(к, _)| к == имя)
                .map(|(_, з)| з.clone())
                .or_else(|| std::env::var(имя).ok())
                .unwrap_or_default()
        };
        let раскладки = взять("XKB_DEFAULT_LAYOUT");
        let варианты = взять("XKB_DEFAULT_VARIANT");
        let модель = взять("XKB_DEFAULT_MODEL");
        let настройки = взять("XKB_DEFAULT_OPTIONS");
        let конфиг = XkbConfig {
            layout: &раскладки,
            variant: &варианты,
            model: &модель,
            options: (!настройки.is_empty()).then_some(настройки.clone()),
            ..Default::default()
        };
        if let Some(клавиатура) = self.seat.get_keyboard() {
            if let Err(е) = клавиатура.set_xkb_config(self, конфиг) {
                tracing::warn!("раскладки «{раскладки}» не встали: {е:?}");
            }
        }
        self.запиши_раскладку();
    }

    /// Номер текущей раскладки (с единицы) и имена всех — для меню на панели
    pub fn раскладки(&mut self) -> Vec<String> {
        let Some(клавиатура) = self.seat.get_keyboard() else { return Vec::new() };
        клавиатура.with_xkb_state(self, |к| {
            let xkb = к.xkb().lock().unwrap();
            let сейчас = xkb.active_layout();
            let имена: Vec<String> = xkb.layouts().map(|р| xkb.layout_name(р).to_string()).collect();
            let номер = xkb.layouts().position(|р| р == сейчас).map(|н| н + 1).unwrap_or(1);
            std::iter::once(номер.to_string()).chain(имена).collect()
        })
    }

    /// Включить раскладку по номеру (с нуля) — щелчок в меню раскладок
    pub fn включи_раскладку(&mut self, номер: usize) -> bool {
        let Some(клавиатура) = self.seat.get_keyboard() else { return false };
        let есть = клавиатура.with_xkb_state(self, |mut к| {
            let нужная = к.xkb().lock().unwrap().layouts().nth(номер);
            match нужная {
                Some(р) => {
                    к.set_layout(р);
                    true
                }
                None => false,
            }
        });
        if есть {
            self.запиши_раскладку();
        }
        есть
    }

    /// Какая раскладка сейчас — в файл, его читает панель. Так значок языка
    /// на панели показывает текущую раскладку, а не весь их список
    pub fn запиши_раскладку(&mut self) {
        let Some(клавиатура) = self.seat.get_keyboard() else { return };
        let имя = клавиатура.with_xkb_state(self, |к| {
            let xkb = к.xkb().lock().unwrap();
            xkb.layout_name(xkb.active_layout()).to_string()
        });
        if имя == self.раскладка {
            return;
        }
        self.раскладка = имя.clone();
        let коротко = короткое_имя_раскладки(&имя);
        if let Some(папка) = папка_выполнения() {
            let _ = std::fs::create_dir_all(&папка);
            let путь = папка.join("layout");
            let врем = папка.join("layout.new");
            if std::fs::write(&врем, format!("{коротко}\n{имя}\n")).is_ok() {
                let _ = std::fs::rename(врем, путь);
            }
        }
    }

    /* ---------- окна ---------- */

    pub fn запись(&self, окно: &Window) -> Option<&Запись> {
        self.окна.iter().find(|з| &з.окно == окно)
    }

    pub fn запись_мут(&mut self, окно: &Window) -> Option<&mut Запись> {
        self.окна.iter_mut().find(|з| &з.окно == окно)
    }

    pub fn окно_поверхности(&self, поверхность: &WlSurface) -> Option<Window> {
        self.окна
            .iter()
            .map(|з| &з.окно)
            .chain(self.ждут.iter())
            .find(|о| о.wl_surface().as_deref() == Some(поверхность))
            .cloned()
    }

    pub fn окно_x11(&self, x: &X11Surface) -> Option<Window> {
        self.окна.iter().map(|з| &з.окно).chain(self.ждут.iter()).find(|о| о.x11_surface() == Some(x)).cloned()
    }

    pub fn главный_экран(&self) -> Option<Output> {
        let мышь = self.мышь;
        self.space.output_under(мышь).next().cloned().or_else(|| self.space.outputs().next().cloned())
    }

    /// Где окнам можно стоять: экран без полос панели и дока
    pub fn рабочая_область(&self, экран: &Output) -> Rectangle<i32, Logical> {
        let гео = self.space.output_geometry(экран).unwrap_or_default();
        let mut зона = layer_map_for_output(экран).non_exclusive_zone();
        зона.loc += гео.loc;
        if зона.is_empty() {
            гео
        } else {
            зона
        }
    }

    /// Первый кадр окна: ставим его посередине свободного места, со сдвигом
    /// лесенкой, чтобы новые окна не ложились ровно друг на друга
    pub fn покажи_новое(&mut self, окно: Window) {
        self.ждут.retain(|о| о != &окно);
        let Some(экран) = self.главный_экран() else { return };
        let область = self.рабочая_область(&экран);
        let размер = окно.geometry().size;
        let ступень = (self.space.elements().count() as i32 % 6) * 28;
        let x = область.loc.x + ((область.size.w - размер.w) / 2).max(0) + ступень - 70;
        let y = область.loc.y + ((область.size.h - размер.h) / 3).max(0) + ступень - 42;
        // Программа могла попросить «во весь экран» или «развернуть» ещё до
        // первого кадра — так делает установщик. Раньше эта просьба
        // терялась, и окно вставало посередине обычным
        let (весь, развёрнуто) = окно
            .toplevel()
            .map(|t| t.with_pending_state(|с| (с.states.contains(xdg_toplevel::State::Fullscreen), с.states.contains(xdg_toplevel::State::Maximized))))
            .unwrap_or((false, false));
        let экран_гео = self.space.output_geometry(&экран).unwrap_or(область);
        let место = if весь {
            экран_гео.loc
        } else if развёрнуто {
            область.loc
        } else {
            Point::from((x.max(область.loc.x), y.max(область.loc.y)))
        };
        let обычное = Rectangle::new(Point::from((x.max(область.loc.x), y.max(область.loc.y))), размер);
        self.окна.push(Запись {
            окно: окно.clone(),
            стол: self.стол,
            свёрнуто: false,
            место,
            прежнее: (весь || развёрнуто).then_some(обычное),
            развёрнуто: развёрнуто && !весь,
            во_весь_экран: весь,
        });
        if весь || развёрнуто {
            if let Some(t) = окно.toplevel() {
                let нужно = if весь { экран_гео.size } else { область.size };
                if t.with_pending_state(|с| с.size) != Some(нужно) {
                    t.with_pending_state(|с| с.size = Some(нужно));
                    t.send_pending_configure();
                }
            }
        }
        self.space.map_element(окно.clone(), место, true);
        self.анимации.появилось(&окно);
        self.foreign_toplevel_state.новое(&self.display_handle, &окно);
        self.дай_фокус(Some(&окно));
        self.нужен_кадр = true;
    }

    pub fn убери_окно(&mut self, окно: &Window) {
        self.ждут.retain(|о| о != окно);
        let было_в_фокусе = self.окно_в_фокусе().as_ref() == Some(окно);
        self.space.unmap_elem(окно);
        self.окна.retain(|з| &з.окно != окно);
        self.foreign_toplevel_state.закрыто(окно);
        if было_в_фокусе {
            let верхнее = self.space.elements().last().cloned();
            self.дай_фокус(верхнее.as_ref());
        }
        self.нужен_кадр = true;
    }

    pub fn окно_в_фокусе(&self) -> Option<Window> {
        let ф = self.seat.get_keyboard()?.current_focus()?;
        match ф {
            Фокус::Wl(с) => self.окно_поверхности(&с),
            Фокус::X11(х) => self.окно_x11(&х),
        }
    }

    /// Окно наверх и клавиатура ему. None — убрать фокус со всех окон
    pub fn дай_фокус(&mut self, окно: Option<&Window>) {
        if self.замок.is_some() {
            return;
        }
        let serial = SERIAL_COUNTER.next_serial();
        if let Some(окно) = окно {
            self.space.raise_element(окно, true);
            if let Some(x) = окно.x11_surface() {
                if let Some(xwm) = self.xwm.as_mut() {
                    let _ = xwm.raise_window(x);
                }
            }
        }
        for о in self.space.elements() {
            let активно = Some(о) == окно;
            о.set_activated(активно);
            if let Some(t) = о.toplevel() {
                t.send_pending_configure();
            }
        }
        // Пока открыт слой, который держит клавиатуру (меню программ, экран
        // выделения для снимка), окно её не забирает — даже новое. Иначе
        // программа, догрузившаяся под открытым меню, уводила Esc себе, и
        // меню было уже не закрыть с клавиатуры
        if let Some(л) = self.слой_с_клавиатурой() {
            self.фокус_слою(&л);
        } else {
            let фокус = окно.and_then(|о| match о.x11_surface() {
                Some(x) => Some(Фокус::X11(x.clone())),
                None => о.toplevel().map(|t| Фокус::Wl(t.wl_surface().clone())),
            });
            if let Some(к) = self.seat.get_keyboard() {
                к.set_focus(self, фокус, serial);
            }
        }
        self.foreign_toplevel_state.обнови_все(&self.space, &self.окна);
    }

    /// Слой поверх окон, который держит клавиатуру всегда (меню панели)
    pub fn слой_с_клавиатурой(&self) -> Option<LayerSurface> {
        self.space.outputs().find_map(|экран| {
            let карта = layer_map_for_output(экран);
            let найдено = карта
                .layers()
                .rev()
                .find(|л| {
                    л.cached_state().keyboard_interactivity == KeyboardInteractivity::Exclusive
                        && matches!(л.layer(), WlrLayer::Top | WlrLayer::Overlay)
                        // Скрытое меню остаётся в списке, но без кадра — оно не в счёт
                        && smithay::backend::renderer::utils::with_renderer_surface_state(л.wl_surface(), |с| с.buffer().is_some()).unwrap_or(false)
                })
                .cloned();
            найдено
        })
    }

    /// Слою панели — клавиатуру, если он о ней просит
    pub fn фокус_слою(&mut self, слой: &LayerSurface) {
        let serial = SERIAL_COUNTER.next_serial();
        if let Some(к) = self.seat.get_keyboard() {
            к.set_focus(self, Some(Фокус::Wl(слой.wl_surface().clone())), serial);
        }
    }

    /// После того как слой с клавиатурой ушёл — фокус обратно верхнему окну
    pub fn верни_фокус(&mut self) {
        if let Some(з) = self.замок.as_ref() {
            if let Some((_, п)) = з.поверхности.first() {
                let п = п.wl_surface().clone();
                if let Some(к) = self.seat.get_keyboard() {
                    к.set_focus(self, Some(Фокус::Wl(п)), SERIAL_COUNTER.next_serial());
                }
            }
            return;
        }
        // Слой, который держит клавиатуру всегда (меню панели), — первым
        if let Some(л) = self.слой_с_клавиатурой() {
            self.фокус_слою(&л);
            return;
        }
        let верхнее = self.space.elements().last().cloned();
        self.дай_фокус(верхнее.as_ref());
    }

    pub fn закрой(&mut self, окно: &Window) {
        if let Some(t) = окно.toplevel() {
            t.send_close();
        } else if let Some(x) = окно.x11_surface() {
            let _ = x.close();
        }
    }

    pub fn сверни(&mut self, окно: &Window) {
        let Some(место) = self.space.element_location(окно) else { return };
        if let Some(з) = self.запись_мут(окно) {
            з.свёрнуто = true;
            з.место = место;
        }
        self.анимации.свернулось(окно, self.space.element_geometry(окно));
        self.space.unmap_elem(окно);
        let верхнее = self.space.elements().last().cloned();
        self.дай_фокус(верхнее.as_ref());
        self.нужен_кадр = true;
    }

    /// Вернуть свёрнутое или окно с другого стола: стол переключаем сами
    pub fn разверни_из_дока(&mut self, окно: &Window) {
        let Some((стол, свёрнуто, место)) = self.запись(окно).map(|з| (з.стол, з.свёрнуто, з.место)) else { return };
        if стол != self.стол {
            self.на_стол(стол);
        }
        if свёрнуто {
            if let Some(з) = self.запись_мут(окно) {
                з.свёрнуто = false;
            }
            self.space.map_element(окно.clone(), место, true);
            self.анимации.появилось(окно);
        }
        self.дай_фокус(Some(окно));
        self.нужен_кадр = true;
    }

    pub fn разверни(&mut self, окно: &Window) {
        let Some(экран) = self.главный_экран() else { return };
        let область = self.рабочая_область(&экран);
        let Some(гео) = self.space.element_geometry(окно) else { return };
        let Some(з) = self.запись_мут(окно) else { return };
        if з.развёрнуто {
            з.развёрнуто = false;
            let прежнее = з.прежнее.take().unwrap_or(Rectangle::new(
                область.loc + Point::from((60, 40)),
                Size::from((область.size.w * 2 / 3, область.size.h * 2 / 3)),
            ));
            self.поставь(окно, прежнее, Some(false));
        } else {
            з.развёрнуто = true;
            if з.прежнее.is_none() {
                з.прежнее = Some(гео);
            }
            self.поставь(окно, область, Some(true));
        }
    }

    /// Прилипнуть к половине экрана (Win+стрелка)
    pub fn к_краю(&mut self, окно: &Window, влево: bool) {
        let Some(экран) = self.главный_экран() else { return };
        let область = self.рабочая_область(&экран);
        let Some(гео) = self.space.element_geometry(окно) else { return };
        if let Some(з) = self.запись_мут(окно) {
            if з.прежнее.is_none() {
                з.прежнее = Some(гео);
            }
            з.развёрнуто = false;
        }
        let пол = область.size.w / 2;
        let куда = Rectangle::new(
            Point::from((if влево { область.loc.x } else { область.loc.x + пол }, область.loc.y)),
            Size::from((пол, область.size.h)),
        );
        self.поставь(окно, куда, Some(false));
    }

    /// Поставить окно в прямоугольник: просим размер и сразу ставим место
    pub fn поставь(&mut self, окно: &Window, куда: Rectangle<i32, Logical>, развёрнуто: Option<bool>) {
        if let Some(t) = окно.toplevel() {
            t.with_pending_state(|с| {
                с.size = Some(куда.size);
                if let Some(р) = развёрнуто {
                    if р {
                        с.states.set(xdg_toplevel::State::Maximized);
                    } else {
                        с.states.unset(xdg_toplevel::State::Maximized);
                    }
                }
            });
            t.send_pending_configure();
        } else if let Some(x) = окно.x11_surface() {
            if let Some(р) = развёрнуто {
                let _ = x.set_maximized(р);
            }
            let _ = x.configure(куда);
        }
        self.space.map_element(окно.clone(), куда.loc, true);
        self.анимации.переехало(окно);
        self.нужен_кадр = true;
    }

    pub fn во_весь_экран(&mut self, окно: &Window, да: bool) {
        let Some(экран) = self.главный_экран() else { return };
        let гео = self.space.output_geometry(&экран).unwrap_or_default();
        let текущее = self.space.element_geometry(окно);
        let Some(з) = self.запись_мут(окно) else { return };
        if да == з.во_весь_экран {
            return;
        }
        з.во_весь_экран = да;
        let прежнее = if да {
            з.прежнее = з.прежнее.or(текущее);
            None
        } else {
            з.прежнее.take()
        };
        if let Some(t) = окно.toplevel() {
            t.with_pending_state(|с| {
                if да {
                    с.states.set(xdg_toplevel::State::Fullscreen);
                    с.size = Some(гео.size);
                } else {
                    с.states.unset(xdg_toplevel::State::Fullscreen);
                    с.size = прежнее.map(|п| п.size);
                }
            });
            t.send_pending_configure();
        } else if let Some(x) = окно.x11_surface() {
            let _ = x.set_fullscreen(да);
            let _ = x.configure(if да { гео } else { прежнее.unwrap_or(гео) });
        }
        let место = if да { гео.loc } else { прежнее.map(|п| п.loc).unwrap_or(гео.loc) };
        self.space.map_element(окно.clone(), место, true);
        self.нужен_кадр = true;
    }

    /// Alt+Tab: следующее окно этого стола
    pub fn следующее(&mut self) {
        let окна: Vec<Window> = self.space.elements().cloned().collect();
        if окна.len() < 2 {
            // Свёрнутые этого стола тоже в очереди
            if let Some(з) = self.окна.iter().find(|з| з.стол == self.стол && з.свёрнуто) {
                let о = з.окно.clone();
                self.разверни_из_дока(&о);
            }
            return;
        }
        // Нижнее окно стопки — наверх: так по кругу проходим все
        let нижнее = окна[0].clone();
        self.дай_фокус(Some(&нижнее));
    }

    /* ---------- рабочие столы ---------- */

    pub fn на_стол(&mut self, n: usize) {
        if n == self.стол || n > 9 {
            return;
        }
        let старые: Vec<Window> = self.space.elements().cloned().collect();
        for о in &старые {
            let где = self.space.element_location(о).unwrap_or_default();
            if let Some(з) = self.запись_мут(о) {
                з.место = где;
            }
            self.space.unmap_elem(о);
        }
        let вправо = n > self.стол;
        self.стол = n;
        let новые: Vec<(Window, Point<i32, Logical>)> = self
            .окна
            .iter()
            .filter(|з| з.стол == n && !з.свёрнуто)
            .map(|з| (з.окно.clone(), з.место))
            .collect();
        for (о, место) in &новые {
            self.space.map_element(о.clone(), *место, false);
            self.анимации.сменился_стол(о, вправо);
        }
        let верхнее = новые.last().map(|(о, _)| о.clone());
        self.дай_фокус(верхнее.as_ref());
        self.нужен_кадр = true;
    }

    pub fn отправь_на_стол(&mut self, окно: &Window, n: usize) {
        if n > 9 {
            return;
        }
        let где = self.space.element_location(окно);
        let тут = self.стол;
        let Some(з) = self.запись_мут(окно) else { return };
        if з.стол == n {
            return;
        }
        з.стол = n;
        if let Some(где) = где {
            з.место = где;
        }
        if n != тут {
            self.space.unmap_elem(окно);
            let верхнее = self.space.elements().last().cloned();
            self.дай_фокус(верхнее.as_ref());
        }
        self.нужен_кадр = true;
    }

    /* ---------- что под мышью ---------- */

    pub fn слой_под(&self, точка: Point<f64, Logical>, слои: &[WlrLayer]) -> Option<(Фокус, Point<f64, Logical>, LayerSurface)> {
        let экран = self.space.output_under(точка).next()?;
        let гео = self.space.output_geometry(экран)?;
        let карта = layer_map_for_output(экран);
        for слой in слои {
            if let Some(л) = карта.layer_under(*слой, точка - гео.loc.to_f64()) {
                let где = карта.layer_geometry(л).unwrap_or_default().loc + гео.loc;
                if let Some((с, м)) = л.surface_under(точка - где.to_f64(), WindowSurfaceType::ALL) {
                    return Some((Фокус::Wl(с), (м + где).to_f64(), л.clone()));
                }
            }
        }
        None
    }

    /// Верхнее окно во весь экран: тогда полоса панели (слой Top) уходит
    /// под него — и в картинке, и для мыши. Слой Overlay (меню программ,
    /// экран блокировки) остаётся сверху всегда
    pub fn во_весь_экран_сверху(&self) -> bool {
        self.space.elements().last().and_then(|о| self.запись(о)).is_some_and(|з| з.во_весь_экран && !з.свёрнуто)
    }

    /// Слои над окнами — с учётом окна во весь экран
    pub fn слои_над_окнами(&self) -> &'static [WlrLayer] {
        if self.во_весь_экран_сверху() {
            &[WlrLayer::Overlay]
        } else {
            &[WlrLayer::Overlay, WlrLayer::Top]
        }
    }

    pub fn под_мышью(&self, точка: Point<f64, Logical>) -> Option<(Фокус, Point<f64, Logical>)> {
        if let Some(з) = self.замок.as_ref() {
            let экран = self.space.output_under(точка).next()?;
            let гео = self.space.output_geometry(экран)?;
            let (_, п) = з.поверхности.iter().find(|(о, _)| о == экран)?;
            return Some((Фокус::Wl(п.wl_surface().clone()), гео.loc.to_f64()));
        }
        if let Some((ф, м, _)) = self.слой_под(точка, self.слои_над_окнами()) {
            return Some((ф, м));
        }
        if let Some((окно, место)) = self.space.element_under(точка) {
            if let Some((с, м)) = окно.surface_under(точка - место.to_f64(), WindowSurfaceType::ALL) {
                let ф = match окно.x11_surface() {
                    Some(x) => Фокус::X11(x.clone()),
                    None => Фокус::Wl(с),
                };
                return Some((ф, (м + место).to_f64()));
            }
        }
        self.слой_под(точка, &[WlrLayer::Bottom, WlrLayer::Background]).map(|(ф, м, _)| (ф, м))
    }

    /// Кадр показан: программы узнают, что можно рисовать следующий
    pub fn кадр_показан(&mut self, экран: &Output) {
        let время = self.start_time.elapsed();
        let тот_же = |_: &WlSurface, _: &smithay::wayland::compositor::SurfaceData| Some(экран.clone());
        for о in self.space.elements() {
            if self.space.outputs_for_element(о).contains(экран) {
                о.send_frame(экран, время, Some(Duration::ZERO), тот_же);
            }
        }
        for л in layer_map_for_output(экран).layers() {
            л.send_frame(экран, время, Some(Duration::ZERO), тот_же);
        }
        if let Some(з) = self.замок.as_ref() {
            for (_, п) in &з.поверхности {
                smithay::desktop::utils::send_frames_surface_tree(п.wl_surface(), экран, время, Some(Duration::ZERO), тот_же);
            }
        }
        if let CursorImageStatus::Surface(с) = &self.cursor_status {
            smithay::desktop::utils::send_frames_surface_tree(с, экран, время, Some(Duration::ZERO), тот_же);
        }
    }

    /// Окна, которые закрыли сами программы — убираем из списков
    pub fn приберись(&mut self) {
        let мёртвые: Vec<Window> = self.окна.iter().filter(|з| !з.окно.alive()).map(|з| з.окно.clone()).collect();
        for о in мёртвые {
            self.убери_окно(&о);
        }
        self.ждут.retain(|о| о.alive());
        self.space.refresh();
        self.popups.cleanup();
        let _ = self.display_handle.flush_clients();
    }

    pub fn клиент_поверхности(с: &WlSurface) -> Option<ClientId> {
        с.client().map(|к| к.id())
    }
}

use smithay::utils::IsAlive;
use smithay::wayland::seat::WaylandFocus;

/// Окно сейчас текущее (в фокусе)
pub fn активно(окно: &Window) -> bool {
    if let Some(t) = окно.toplevel() {
        return t.with_pending_state(|с| с.states.contains(xdg_toplevel::State::Activated));
    }
    окно.x11_surface().map(|x| x.is_activated()).unwrap_or(false)
}

/// $XDG_RUNTIME_DIR/glower-wm — сюда сервер пишет то, что читает панель
pub fn папка_выполнения() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR").map(|п| PathBuf::from(п).join("glower-wm"))
}

/// «English (US)» → «EN», «Russian» → «RU»: для значка на панели
pub fn короткое_имя_раскладки(имя: &str) -> String {
    const ИМЕНА: &[(&str, &str)] = &[
        ("English", "EN"), ("Russian", "RU"), ("Ukrainian", "UK"), ("Belarusian", "BE"), ("Kazakh", "KK"),
        ("German", "DE"), ("French", "FR"), ("Spanish", "ES"), ("Italian", "IT"), ("Portuguese", "PT"),
        ("Polish", "PL"), ("Turkish", "TR"),
    ];
    ИМЕНА
        .iter()
        .find(|(н, _)| имя.starts_with(н))
        .map(|(_, к)| к.to_string())
        .unwrap_or_else(|| имя.chars().filter(|с| с.is_alphabetic()).take(2).collect::<String>().to_uppercase())
}

fn libc_clock_monotonic() -> u32 {
    // CLOCK_MONOTONIC в Linux — 1
    1
}

#[cfg(test)]
mod проверки {
    use super::*;

    #[test]
    fn раскладки_коротко() {
        assert_eq!(короткое_имя_раскладки("English (US)"), "EN");
        assert_eq!(короткое_имя_раскладки("Russian"), "RU");
        assert_eq!(короткое_имя_раскладки("Georgian"), "GE");
    }
}
