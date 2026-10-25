//! Протоколы Wayland: что делает сервер, когда программа о чём-то просит

use std::os::fd::OwnedFd;

use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_compositor, delegate_cursor_shape, delegate_data_control, delegate_data_device, delegate_fractional_scale,
    delegate_idle_notify, delegate_layer_shell, delegate_output, delegate_pointer_constraints, delegate_presentation,
    delegate_primary_selection, delegate_relative_pointer, delegate_seat, delegate_session_lock, delegate_shm,
    delegate_single_pixel_buffer, delegate_viewporter, delegate_virtual_keyboard_manager, delegate_xdg_activation,
    delegate_xdg_decoration, delegate_xdg_shell, delegate_xwayland_shell,
    desktop::{
        find_popup_root_surface, get_popup_toplevel_coords, layer_map_for_output, LayerSurface, PopupKeyboardGrab,
        PopupKind, PopupPointerGrab, PopupUngrabStrategy, Window, WindowSurfaceType,
    },
    input::{
        pointer::{CursorImageStatus, Focus, GrabStartData},
        Seat, SeatHandler, SeatState,
    },
    output::Output,
    reexports::{
        wayland_protocols::xdg::{decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as DecorationMode, shell::server::xdg_toplevel},
        wayland_server::{
            protocol::{wl_buffer::WlBuffer, wl_data_source::WlDataSource, wl_output::WlOutput, wl_seat::WlSeat, wl_surface::WlSurface},
            Client, Resource,
        },
    },
    utils::{Logical, Rectangle, Serial},
    wayland::{
        buffer::BufferHandler,
        compositor::{get_parent, is_sync_subsurface, with_states, CompositorClientState, CompositorHandler, CompositorState, SurfaceAttributes},
        fractional_scale::{with_fractional_scale, FractionalScaleHandler},
        idle_notify::{IdleNotifierHandler, IdleNotifierState},
        output::OutputHandler,
        pointer_constraints::{with_pointer_constraint, PointerConstraintsHandler},
        seat::WaylandFocus,
        selection::{
            data_device::{set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler},
            primary_selection::{set_primary_focus, PrimarySelectionHandler, PrimarySelectionState},
            wlr_data_control::{DataControlHandler, DataControlState},
            SelectionHandler, SelectionSource, SelectionTarget,
        },
        session_lock::{LockSurface, SessionLockHandler, SessionLockManagerState, SessionLocker},
        shell::{
            wlr_layer::{KeyboardInteractivity, Layer, LayerSurface as WlrLayerSurface, LayerSurfaceData, WlrLayerShellHandler, WlrLayerShellState},
            xdg::{
                decoration::XdgDecorationHandler, PopupSurface, PositionerState, ToplevelSurface, XdgPopupSurfaceData,
                XdgShellHandler, XdgShellState, XdgToplevelSurfaceData,
            },
        },
        shm::{ShmHandler, ShmState},
        tablet_manager::TabletSeatHandler,
        xdg_activation::{XdgActivationHandler, XdgActivationState, XdgActivationToken, XdgActivationTokenData},
        xwayland_shell::{XWaylandShellHandler, XWaylandShellState},
    },
    xwayland::X11Wm,
};

use crate::{
    focus::Фокус,
    grabs::{self, Края, Перенос, Размер},
    state::{ClientState, Glower, Замок},
};

impl CompositorHandler for Glower {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        if let Some(с) = client.get_data::<smithay::xwayland::XWaylandClientData>() {
            return &с.compositor_state;
        }
        &client.get_data::<ClientState>().expect("данные программы").compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
        self.нужен_кадр = true;

        // Значок переноса сдвигает себя сам (attach с x, y): так программа
        // ставит точку, за которую «держат» значок
        if let Some((з, сдвиг)) = self.значок_переноса.as_mut() {
            if з == surface {
                let дельта = with_states(surface, |с| с.cached_state.get::<SurfaceAttributes>().current().buffer_delta.take());
                if let Some(д) = дельта {
                    *сдвиг += д;
                }
            }
        }

        if !is_sync_subsurface(surface) {
            let mut корень = surface.clone();
            while let Some(р) = get_parent(&корень) {
                корень = р;
            }
            if let Some(окно) = self.окно_поверхности(&корень) {
                окно.on_commit();
                let ждёт = self.ждут.contains(&окно);
                if ждёт && есть_кадр(&корень) && окно.toplevel().is_some() {
                    self.покажи_новое(окно.clone());
                }
                grabs::поправь_место(self, &окно);
            }
        }

        // Первый ответ программе: «можешь рисовать», без него окно не начнёт
        if let Some(t) = self.ждут.iter().find_map(|о| о.toplevel().filter(|t| t.wl_surface() == surface).cloned()) {
            let послано = with_states(surface, |с| {
                с.data_map.get::<XdgToplevelSurfaceData>().map(|д| д.lock().unwrap().initial_configure_sent).unwrap_or(true)
            });
            if !послано {
                t.send_configure();
            }
        }

        self.popups.commit(surface);
        if let Some(PopupKind::Xdg(п)) = self.popups.find_popup(surface) {
            let послано = with_states(surface, |с| {
                с.data_map.get::<XdgPopupSurfaceData>().map(|д| д.lock().unwrap().initial_configure_sent).unwrap_or(true)
            });
            if !послано {
                let _ = п.send_configure();
            }
        }

        self.слой_зафиксирован(surface);
    }
}

/// Есть ли у поверхности нарисованный кадр
fn есть_кадр(поверхность: &WlSurface) -> bool {
    smithay::backend::renderer::utils::with_renderer_surface_state(поверхность, |с| с.buffer().is_some()).unwrap_or(false)
}

impl Glower {
    fn слой_зафиксирован(&mut self, поверхность: &WlSurface) {
        let найдено = self.space.outputs().find_map(|экран| {
            let карта = layer_map_for_output(экран);
            карта.layer_for_surface(поверхность, WindowSurfaceType::TOPLEVEL).cloned().map(|л| (экран.clone(), л))
        });
        let Some((экран, слой)) = найдено else { return };
        let послано = with_states(поверхность, |с| {
            с.data_map.get::<LayerSurfaceData>().map(|д| д.lock().unwrap().initial_configure_sent).unwrap_or(true)
        });
        {
            let mut карта = layer_map_for_output(&экран);
            карта.arrange();
        }
        if !послано {
            слой.layer_surface().send_configure();
            return;
        }
        // Меню панели с клавиатурой — ему фокус, как только оно нарисовалось
        let данные = слой.cached_state();
        if данные.keyboard_interactivity == KeyboardInteractivity::Exclusive
            && matches!(слой.layer(), Layer::Top | Layer::Overlay)
            && есть_кадр(поверхность)
        {
            let уже = self.seat.get_keyboard().and_then(|к| к.current_focus()).and_then(|ф| ф.wl_surface().map(|с| с.into_owned()));
            if уже.as_ref() != Some(поверхность) && self.замок.is_none() {
                self.фокус_слою(&слой);
            }
        }
        self.подгони_развёрнутые(&экран);
    }

    /// Полоса панели или док поменяли размер — развёрнутые окна подгоняем
    fn подгони_развёрнутые(&mut self, экран: &Output) {
        let область = self.рабочая_область(экран);
        let развёрнутые: Vec<Window> = self.окна.iter().filter(|з| з.развёрнуто && з.стол == self.стол && !з.свёрнуто).map(|з| з.окно.clone()).collect();
        for о in развёрнутые {
            if self.space.element_geometry(&о) != Some(область) {
                self.поставь(&о, область, Some(true));
            }
        }
    }

    /// Всплывающее меню не должно вылезать за экран
    fn поставь_всплывающее(&self, всплывающее: &PopupSurface) {
        let Ok(корень) = find_popup_root_surface(&PopupKind::Xdg(всплывающее.clone())) else { return };
        let (корень_где, экран_гео) = if let Some(окно) = self.окно_поверхности(&корень) {
            let где = self.space.element_location(&окно).unwrap_or_default();
            let экран = self.space.outputs_for_element(&окно).into_iter().next().or_else(|| self.главный_экран());
            (где, экран.and_then(|э| self.space.output_geometry(&э)).unwrap_or_default())
        } else {
            let найдено = self.space.outputs().find_map(|э| {
                let карта = layer_map_for_output(э);
                let л = карта.layer_for_surface(&корень, WindowSurfaceType::TOPLEVEL)?;
                let гео = self.space.output_geometry(э)?;
                Some((карта.layer_geometry(л)?.loc + гео.loc, гео))
            });
            match найдено {
                Some(н) => н,
                None => return,
            }
        };
        let mut цель: Rectangle<i32, Logical> = экран_гео;
        цель.loc -= корень_где;
        цель.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(всплывающее.clone()));
        всплывающее.with_pending_state(|с| {
            с.geometry = с.positioner.get_unconstrained_geometry(цель);
        });
    }
}

impl XdgShellHandler for Glower {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let окно = Window::new_wayland_window(surface);
        self.ждут.push(окно);
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        self.поставь_всплывающее(&surface);
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn reposition_request(&mut self, surface: PopupSurface, positioner: PositionerState, token: u32) {
        surface.with_pending_state(|с| {
            с.geometry = positioner.get_geometry();
            с.positioner = positioner;
        });
        self.поставь_всплывающее(&surface);
        surface.send_repositioned(token);
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: WlSeat, serial: Serial) {
        let Some(seat) = Seat::<Self>::from_resource(&seat) else { return };
        let Some(окно) = self.окно_поверхности(surface.wl_surface()) else { return };
        self.начни_перенос(&seat, окно, serial);
    }

    fn resize_request(&mut self, surface: ToplevelSurface, seat: WlSeat, serial: Serial, edges: xdg_toplevel::ResizeEdge) {
        let Some(seat) = Seat::<Self>::from_resource(&seat) else { return };
        let Some(окно) = self.окно_поверхности(surface.wl_surface()) else { return };
        self.начни_размер(&seat, окно, serial, Края::из_xdg(edges));
    }

    fn grab(&mut self, surface: PopupSurface, seat: WlSeat, serial: Serial) {
        let Some(seat) = Seat::<Self>::from_resource(&seat) else { return };
        let вид = PopupKind::Xdg(surface);
        let Ok(корень) = find_popup_root_surface(&вид) else { return };
        let корень_фокус = match self.окно_поверхности(&корень) {
            Some(о) => match о.x11_surface() {
                Some(x) => Фокус::X11(x.clone()),
                None => Фокус::Wl(корень),
            },
            None => Фокус::Wl(корень),
        };
        let Ok(mut захват) = self.popups.grab_popup(корень_фокус, вид, &seat, serial) else { return };
        if let Some(к) = seat.get_keyboard() {
            if к.is_grabbed() && !(к.has_grab(serial) || к.has_grab(захват.previous_serial().unwrap_or(serial))) {
                захват.ungrab(PopupUngrabStrategy::All);
                return;
            }
            к.set_focus(self, захват.current_grab(), serial);
            к.set_grab(self, PopupKeyboardGrab::new(&захват), serial);
        }
        if let Some(у) = seat.get_pointer() {
            if у.is_grabbed() && !(у.has_grab(serial) || у.has_grab(захват.previous_serial().unwrap_or_else(|| захват.serial()))) {
                захват.ungrab(PopupUngrabStrategy::All);
                return;
            }
            у.set_grab(self, PopupPointerGrab::new(&захват), serial, Focus::Keep);
        }
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        if let Some(о) = self.окно_поверхности(surface.wl_surface()) {
            if !self.запись(&о).map(|з| з.развёрнуто).unwrap_or(false) {
                self.разверни(&о);
            }
        } else {
            // Ещё не показано: сразу даём размер рабочей области, чтобы
            // первый же кадр был нужного размера
            let область = self.главный_экран().map(|э| self.рабочая_область(&э));
            surface.with_pending_state(|с| {
                с.states.set(xdg_toplevel::State::Maximized);
                if let Some(о) = область {
                    с.size = Some(о.size);
                }
            });
            if surface.is_initial_configure_sent() {
                surface.send_pending_configure();
            }
        }
    }

    fn unmaximize_request(&mut self, surface: ToplevelSurface) {
        if let Some(о) = self.окно_поверхности(surface.wl_surface()) {
            if self.запись(&о).map(|з| з.развёрнуто).unwrap_or(false) {
                self.разверни(&о);
            }
        }
    }

    fn fullscreen_request(&mut self, surface: ToplevelSurface, _output: Option<WlOutput>) {
        if let Some(о) = self.окно_поверхности(surface.wl_surface()) {
            if self.запись(&о).is_some() {
                self.во_весь_экран(&о, true);
                return;
            }
        }
        // Ещё не показано: размер экрана сразу, место — при первом кадре
        let экран = self.главный_экран().and_then(|э| self.space.output_geometry(&э));
        surface.with_pending_state(|с| {
            с.states.set(xdg_toplevel::State::Fullscreen);
            if let Some(г) = экран {
                с.size = Some(г.size);
            }
        });
        if surface.is_initial_configure_sent() {
            surface.send_pending_configure();
        }
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        if let Some(о) = self.окно_поверхности(surface.wl_surface()) {
            self.во_весь_экран(&о, false);
        }
    }

    fn minimize_request(&mut self, surface: ToplevelSurface) {
        if let Some(о) = self.окно_поверхности(surface.wl_surface()) {
            self.сверни(&о);
        }
    }

    fn title_changed(&mut self, surface: ToplevelSurface) {
        if let Some(о) = self.окно_поверхности(surface.wl_surface()) {
            self.foreign_toplevel_state.обнови(&о, &self.space, &self.окна);
        }
    }

    fn app_id_changed(&mut self, surface: ToplevelSurface) {
        self.title_changed(surface);
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        let Some(окно) = self.окно_поверхности(surface.wl_surface()) else { return };
        if self.space.element_location(&окно).is_some() {
            self.сними_на_прощание(&окно);
        }
        self.убери_окно(&окно);
    }

    fn popup_destroyed(&mut self, _surface: PopupSurface) {
        self.нужен_кадр = true;
    }
}

impl Glower {
    pub fn начни_перенос(&mut self, seat: &Seat<Self>, окно: Window, serial: Serial) {
        let Some(указатель) = seat.get_pointer() else { return };
        tracing::debug!("просят перенос: захват {}, serial {:?}", указатель.has_grab(serial), serial);
        let Some(начало) = указатель.grab_start_data().filter(|_| указатель.has_grab(serial)) else { return };
        if начало.focus.as_ref().and_then(|(ф, _)| ф.wl_surface().map(|с| с.into_owned())).map(|с| Some(с.id().same_client_as(&окно.wl_surface()?.id()))) .flatten() != Some(true) {
            return;
        }
        let где = self.space.element_location(&окно).unwrap_or_default();
        let захват = Перенос { start_data: начало, окно, начало: где };
        указатель.set_grab(self, захват, serial, Focus::Clear);
    }

    pub fn начни_размер(&mut self, seat: &Seat<Self>, окно: Window, serial: Serial, края: Края) {
        let Some(указатель) = seat.get_pointer() else { return };
        let Some(начало) = указатель.grab_start_data().filter(|_| указатель.has_grab(serial)) else { return };
        let Some(гео) = self.space.element_geometry(&окно) else { return };
        let захват = Размер { start_data: начало, окно, края, начало: гео, последний: гео.size };
        указатель.set_grab(self, захват, serial, Focus::Clear);
    }

    /// Перенос по Super+мышь, без заголовка: захват начинает сам сервер
    pub fn перенос_мышью(&mut self, окно: Window, serial: Serial, кнопка: u32) {
        let Some(указатель) = self.seat.get_pointer() else { return };
        let где = self.мышь;
        let начало = GrabStartData { focus: None, button: кнопка, location: где };
        let место = self.space.element_location(&окно).unwrap_or_default();
        указатель.set_grab(self, Перенос { start_data: начало, окно, начало: место }, serial, Focus::Clear);
    }
}

impl WlrLayerShellHandler for Glower {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(&mut self, surface: WlrLayerSurface, output: Option<WlOutput>, _layer: Layer, namespace: String) {
        let экран = output.as_ref().and_then(Output::from_resource).or_else(|| self.главный_экран());
        let Some(экран) = экран else { return };
        let mut карта = layer_map_for_output(&экран);
        if let Err(е) = карта.map_layer(&LayerSurface::new(surface, namespace)) {
            tracing::warn!("слой не встал: {е:?}");
        }
    }

    fn new_popup(&mut self, _parent: WlrLayerSurface, popup: PopupSurface) {
        self.поставь_всплывающее(&popup);
    }

    fn layer_destroyed(&mut self, surface: WlrLayerSurface) {
        let был_фокус = self
            .seat
            .get_keyboard()
            .and_then(|к| к.current_focus())
            .and_then(|ф| ф.wl_surface().map(|с| с.into_owned()))
            .as_ref()
            == Some(surface.wl_surface());
        let экраны: Vec<Output> = self.space.outputs().cloned().collect();
        for э in экраны {
            let mut карта = layer_map_for_output(&э);
            let слой = карта.layers().find(|л| л.layer_surface() == &surface).cloned();
            if let Some(л) = слой {
                карта.unmap_layer(&л);
                drop(карта);
                self.подгони_развёрнутые(&э);
            }
        }
        if был_фокус {
            self.верни_фокус();
        }
        self.нужен_кадр = true;
    }
}

impl SeatHandler for Glower {
    type KeyboardFocus = Фокус;
    type PointerFocus = Фокус;
    type TouchFocus = Фокус;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&Фокус>) {
        let dh = &self.display_handle;
        let клиент = focused.and_then(|ф| ф.wl_surface()).and_then(|с| dh.get_client(с.id()).ok());
        set_data_device_focus(dh, seat, клиент.clone());
        set_primary_focus(dh, seat, клиент);
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        self.cursor_status = image;
        self.нужен_кадр = true;
    }

    fn led_state_changed(&mut self, _seat: &Seat<Self>, _led_state: smithay::input::keyboard::LedState) {}
}

impl TabletSeatHandler for Glower {}

impl SelectionHandler for Glower {
    type SelectionUserData = ();

    fn new_selection(&mut self, ty: SelectionTarget, source: Option<SelectionSource>, _seat: Seat<Self>) {
        if let Some(xwm) = self.xwm.as_mut() {
            if let Err(е) = xwm.new_selection(ty, source.map(|с| с.mime_types())) {
                tracing::warn!("буфер обмена для X11: {е}");
            }
        }
    }

    fn send_selection(&mut self, ty: SelectionTarget, mime_type: String, fd: OwnedFd, _seat: Seat<Self>, _user_data: &()) {
        if let Some(xwm) = self.xwm.as_mut() {
            if let Err(е) = xwm.send_selection(ty, mime_type, fd, self.loop_handle.clone()) {
                tracing::warn!("буфер обмена из X11: {е}");
            }
        }
    }
}

impl DataDeviceHandler for Glower {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}
// Перенос мышью между программами сам по себе ведёт smithay; наше дело —
// рисовать у указателя значок, который дала программа
impl ClientDndGrabHandler for Glower {
    fn started(&mut self, _source: Option<WlDataSource>, icon: Option<WlSurface>, _seat: Seat<Self>) {
        self.значок_переноса = icon.map(|з| (з, (0, 0).into()));
        self.нужен_кадр = true;
    }
    fn dropped(&mut self, _target: Option<WlSurface>, _validated: bool, _seat: Seat<Self>) {
        self.значок_переноса = None;
        self.нужен_кадр = true;
    }
}
impl ServerDndGrabHandler for Glower {}

impl PrimarySelectionHandler for Glower {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.primary_selection_state
    }
}

impl DataControlHandler for Glower {
    fn data_control_state(&self) -> &DataControlState {
        &self.data_control_state
    }
}

impl BufferHandler for Glower {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

impl ShmHandler for Glower {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl OutputHandler for Glower {}

impl XdgDecorationHandler for Glower {
    // Заголовки рисуют сами программы: у GTK и Qt они свои, привычные
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|с| с.decoration_mode = Some(DecorationMode::ClientSide));
    }
    fn request_mode(&mut self, toplevel: ToplevelSurface, _mode: DecorationMode) {
        toplevel.with_pending_state(|с| с.decoration_mode = Some(DecorationMode::ClientSide));
        if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure();
        }
    }
    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        self.request_mode(toplevel, DecorationMode::ClientSide);
    }
}

impl FractionalScaleHandler for Glower {
    fn new_fractional_scale(&mut self, surface: WlSurface) {
        let масштаб = self.главный_экран().map(|э| э.current_scale().fractional_scale()).unwrap_or(1.0);
        with_states(&surface, |с| with_fractional_scale(с, |дробный| дробный.set_preferred_scale(масштаб)));
    }
}

impl XdgActivationHandler for Glower {
    fn activation_state(&mut self) -> &mut XdgActivationState {
        &mut self.xdg_activation_state
    }

    fn token_created(&mut self, _token: XdgActivationToken, data: XdgActivationTokenData) -> bool {
        // Просить внимание может только тот, кто недавно получал ввод
        data.serial.is_some()
    }

    fn request_activation(&mut self, _token: XdgActivationToken, token_data: XdgActivationTokenData, surface: WlSurface) {
        if token_data.timestamp.elapsed().as_secs() < 10 {
            if let Some(о) = self.окно_поверхности(&surface) {
                self.разверни_из_дока(&о);
            }
        }
    }
}

impl SessionLockHandler for Glower {
    fn lock_state(&mut self) -> &mut SessionLockManagerState {
        &mut self.session_lock_state
    }

    fn lock(&mut self, confirmation: SessionLocker) {
        self.замок = Some(Замок { поверхности: vec![] });
        if let Some(к) = self.seat.get_keyboard() {
            к.set_focus(self, None, smithay::utils::SERIAL_COUNTER.next_serial());
        }
        confirmation.lock();
        self.нужен_кадр = true;
    }

    fn unlock(&mut self) {
        self.замок = None;
        self.верни_фокус();
        self.нужен_кадр = true;
    }

    fn new_surface(&mut self, surface: LockSurface, output: WlOutput) {
        let Some(экран) = Output::from_resource(&output) else { return };
        let Some(гео) = self.space.output_geometry(&экран) else { return };
        surface.with_pending_state(|с| {
            с.size = Some((гео.size.w as u32, гео.size.h as u32).into());
        });
        surface.send_configure();
        let поверхность = surface.wl_surface().clone();
        if let Some(з) = self.замок.as_mut() {
            з.поверхности.push((экран, surface));
        }
        if let Some(к) = self.seat.get_keyboard() {
            к.set_focus(self, Some(Фокус::Wl(поверхность)), smithay::utils::SERIAL_COUNTER.next_serial());
        }
    }
}

impl IdleNotifierHandler for Glower {
    fn idle_notifier_state(&mut self) -> &mut IdleNotifierState<Self> {
        &mut self.idle_notifier_state
    }
}

impl XWaylandShellHandler for Glower {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.xwayland_shell_state
    }
}

impl PointerConstraintsHandler for Glower {
    fn new_constraint(&mut self, surface: &WlSurface, pointer: &smithay::input::pointer::PointerHandle<Self>) {
        // Игры просят держать мышь у себя — включаем, если окно уже в фокусе
        if pointer.current_focus().and_then(|ф| ф.wl_surface().map(|с| с.into_owned())).as_ref() == Some(surface) {
            with_pointer_constraint(surface, pointer, |с| {
                if let Some(с) = с {
                    с.activate();
                }
            });
        }
    }

    fn cursor_position_hint(&mut self, _surface: &WlSurface, _pointer: &smithay::input::pointer::PointerHandle<Self>, _location: smithay::utils::Point<f64, Logical>) {}
}

delegate_compositor!(Glower);
delegate_xdg_shell!(Glower);
delegate_xdg_decoration!(Glower);
delegate_layer_shell!(Glower);
delegate_shm!(Glower);
delegate_output!(Glower);
delegate_seat!(Glower);
delegate_data_device!(Glower);
delegate_primary_selection!(Glower);
delegate_data_control!(Glower);
delegate_viewporter!(Glower);
delegate_fractional_scale!(Glower);
delegate_xdg_activation!(Glower);
delegate_presentation!(Glower);
delegate_cursor_shape!(Glower);
delegate_session_lock!(Glower);
delegate_idle_notify!(Glower);
delegate_single_pixel_buffer!(Glower);
delegate_xwayland_shell!(Glower);
delegate_relative_pointer!(Glower);
delegate_pointer_constraints!(Glower);
delegate_virtual_keyboard_manager!(Glower);
