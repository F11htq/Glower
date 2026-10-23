//! wlr-foreign-toplevel-management: список окон для дока и панели задач,
//! и просьбы оттуда — свернуть, развернуть, сделать текущим, закрыть

use smithay::{
    desktop::{Space, Window},
    reexports::{
        wayland_protocols_wlr::foreign_toplevel::v1::server::{
            zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
            zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
        },
        wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource},
    },
    wayland::{compositor::with_states, shell::xdg::XdgToplevelSurfaceData},
};

use crate::state::{Glower, Запись};

const ВЕРСИЯ: u32 = 3;

#[derive(Debug, Clone, Default, PartialEq)]
struct Сведения {
    заголовок: String,
    класс: String,
    состояния: Vec<u32>,
}

struct Ручки {
    окно: Window,
    номер: u64,
    ручки: Vec<ZwlrForeignToplevelHandleV1>,
    последнее: Сведения,
}

pub struct ForeignToplevelState {
    хозяева: Vec<ZwlrForeignToplevelManagerV1>,
    окна: Vec<Ручки>,
    следующий: u64,
}

fn сведения(окно: &Window, space: &Space<Window>, записи: &[Запись]) -> Сведения {
    let (заголовок, класс) = if let Some(t) = окно.toplevel() {
        with_states(t.wl_surface(), |с| {
            с.data_map
                .get::<XdgToplevelSurfaceData>()
                .map(|д| {
                    let д = д.lock().unwrap();
                    (д.title.clone().unwrap_or_default(), д.app_id.clone().unwrap_or_default())
                })
                .unwrap_or_default()
        })
    } else if let Some(x) = окно.x11_surface() {
        (x.title(), x.class())
    } else {
        Default::default()
    };
    let з = записи.iter().find(|з| &з.окно == окно);
    let mut состояния = vec![];
    if з.map(|з| з.развёрнуто).unwrap_or(false) {
        состояния.push(zwlr_foreign_toplevel_handle_v1::State::Maximized as u32);
    }
    if з.map(|з| з.свёрнуто).unwrap_or(false) {
        состояния.push(zwlr_foreign_toplevel_handle_v1::State::Minimized as u32);
    }
    if з.map(|з| з.во_весь_экран).unwrap_or(false) {
        состояния.push(zwlr_foreign_toplevel_handle_v1::State::Fullscreen as u32);
    }
    // Текущее — верхнее окно этого стола и в фокусе
    let видно = space.elements().any(|о| о == окно);
    if видно && crate::state::активно(окно) {
        состояния.push(zwlr_foreign_toplevel_handle_v1::State::Activated as u32);
    }
    Сведения { заголовок, класс, состояния }
}

fn пошли(ручка: &ZwlrForeignToplevelHandleV1, с: &Сведения) {
    ручка.title(с.заголовок.clone());
    ручка.app_id(с.класс.clone());
    let байты: Vec<u8> = с.состояния.iter().flat_map(|с| с.to_ne_bytes()).collect();
    ручка.state(байты);
    ручка.done();
}

impl ForeignToplevelState {
    pub fn new(dh: &DisplayHandle) -> Self {
        dh.create_global::<Glower, ZwlrForeignToplevelManagerV1, _>(ВЕРСИЯ, ());
        ForeignToplevelState { хозяева: vec![], окна: vec![], следующий: 1 }
    }

    fn ручка_для(&mut self, dh: &DisplayHandle, хозяин: &ZwlrForeignToplevelManagerV1, номер: u64) -> Option<ZwlrForeignToplevelHandleV1> {
        let клиент = хозяин.client()?;
        let ручка = клиент.create_resource::<ZwlrForeignToplevelHandleV1, u64, Glower>(dh, хозяин.version(), номер).ok()?;
        хозяин.toplevel(&ручка);
        Some(ручка)
    }

    pub fn новое(&mut self, dh: &DisplayHandle, окно: &Window) {
        if self.окна.iter().any(|р| &р.окно == окно) {
            return;
        }
        let номер = self.следующий;
        self.следующий += 1;
        let хозяева = self.хозяева.clone();
        let ручки = хозяева.iter().filter_map(|х| self.ручка_для(dh, х, номер)).collect();
        self.окна.push(Ручки { окно: окно.clone(), номер, ручки, последнее: Сведения::default() });
    }

    pub fn обнови(&mut self, окно: &Window, space: &Space<Window>, записи: &[Запись]) {
        let Some(р) = self.окна.iter_mut().find(|р| &р.окно == окно) else { return };
        let с = сведения(окно, space, записи);
        if с == р.последнее {
            return;
        }
        for ручка in &р.ручки {
            пошли(ручка, &с);
        }
        р.последнее = с;
    }

    pub fn обнови_все(&mut self, space: &Space<Window>, записи: &[Запись]) {
        let окна: Vec<Window> = self.окна.iter().map(|р| р.окно.clone()).collect();
        for о in окна {
            self.обнови(&о, space, записи);
        }
    }

    pub fn закрыто(&mut self, окно: &Window) {
        if let Some(i) = self.окна.iter().position(|р| &р.окно == окно) {
            for ручка in &self.окна[i].ручки {
                ручка.closed();
            }
            self.окна.remove(i);
        }
    }

    fn окно_по_номеру(&self, номер: u64) -> Option<Window> {
        self.окна.iter().find(|р| р.номер == номер).map(|р| р.окно.clone())
    }
}

impl GlobalDispatch<ZwlrForeignToplevelManagerV1, ()> for Glower {
    fn bind(
        state: &mut Self,
        dh: &DisplayHandle,
        _client: &Client,
        resource: New<ZwlrForeignToplevelManagerV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        let хозяин = data_init.init(resource, ());
        // Новому слушателю — все окна, что уже есть
        let номера: Vec<(u64, Window)> = state.foreign_toplevel_state.окна.iter().map(|р| (р.номер, р.окно.clone())).collect();
        for (номер, окно) in номера {
            if let Some(ручка) = state.foreign_toplevel_state.ручка_для(dh, &хозяин, номер) {
                let с = сведения(&окно, &state.space, &state.окна);
                пошли(&ручка, &с);
                if let Some(р) = state.foreign_toplevel_state.окна.iter_mut().find(|р| р.номер == номер) {
                    р.ручки.push(ручка);
                }
            }
        }
        state.foreign_toplevel_state.хозяева.push(хозяин);
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for Glower {
    fn request(
        state: &mut Self,
        _client: &Client,
        resource: &ZwlrForeignToplevelManagerV1,
        request: zwlr_foreign_toplevel_manager_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        if let zwlr_foreign_toplevel_manager_v1::Request::Stop = request {
            state.foreign_toplevel_state.хозяева.retain(|х| х != resource);
            resource.finished();
        }
    }

    fn destroyed(state: &mut Self, _client: smithay::reexports::wayland_server::backend::ClientId, resource: &ZwlrForeignToplevelManagerV1, _data: &()) {
        state.foreign_toplevel_state.хозяева.retain(|х| х != resource);
    }
}

impl Dispatch<ZwlrForeignToplevelHandleV1, u64> for Glower {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &ZwlrForeignToplevelHandleV1,
        request: zwlr_foreign_toplevel_handle_v1::Request,
        номер: &u64,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        use zwlr_foreign_toplevel_handle_v1::Request as З;
        let Some(окно) = state.foreign_toplevel_state.окно_по_номеру(*номер) else { return };
        let развёрнуто = state.запись(&окно).map(|з| з.развёрнуто).unwrap_or(false);
        let свёрнуто = state.запись(&окно).map(|з| з.свёрнуто).unwrap_or(false);
        match request {
            З::SetMaximized => {
                if свёрнуто {
                    state.разверни_из_дока(&окно);
                }
                if !развёрнуто {
                    state.разверни(&окно);
                }
            }
            З::UnsetMaximized => {
                if развёрнуто {
                    state.разверни(&окно);
                }
            }
            З::SetMinimized => {
                if !свёрнуто && state.space.element_location(&окно).is_some() {
                    state.сверни(&окно);
                }
            }
            З::UnsetMinimized => {
                if свёрнуто {
                    state.разверни_из_дока(&окно);
                }
            }
            З::Activate { .. } => state.разверни_из_дока(&окно),
            З::Close => state.закрой(&окно),
            З::SetFullscreen { .. } => state.во_весь_экран(&окно, true),
            З::UnsetFullscreen => state.во_весь_экран(&окно, false),
            _ => {}
        }
        state.foreign_toplevel_state.обнови_все(&state.space, &state.окна);
    }

    fn destroyed(state: &mut Self, _client: smithay::reexports::wayland_server::backend::ClientId, resource: &ZwlrForeignToplevelHandleV1, _data: &u64) {
        for р in state.foreign_toplevel_state.окна.iter_mut() {
            р.ручки.retain(|р2| р2 != resource);
        }
    }
}
