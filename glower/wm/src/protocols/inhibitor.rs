//! wlr-input-inhibitor: замок экрана просит «весь ввод только мне».
//! Пока он жив, сочетания не работают, а клавиши и мышь достаются только ему

use smithay::reexports::{
    wayland_protocols_wlr::input_inhibitor::v1::server::{
        zwlr_input_inhibit_manager_v1::{self, ZwlrInputInhibitManagerV1},
        zwlr_input_inhibitor_v1::{self, ZwlrInputInhibitorV1},
    },
    wayland_server::{backend::ClientId, Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource},
};

use crate::state::Glower;

pub struct InhibitorState {
    pub кто: Option<ClientId>,
}

impl InhibitorState {
    pub fn new(dh: &DisplayHandle) -> Self {
        dh.create_global::<Glower, ZwlrInputInhibitManagerV1, _>(1, ());
        InhibitorState { кто: None }
    }

    /// Можно ли этой программе получать ввод
    pub fn можно(&self, клиент: Option<&ClientId>) -> bool {
        match (&self.кто, клиент) {
            (None, _) => true,
            (Some(к), Some(с)) => к == с,
            (Some(_), None) => false,
        }
    }
}

impl GlobalDispatch<ZwlrInputInhibitManagerV1, ()> for Glower {
    fn bind(_state: &mut Self, _dh: &DisplayHandle, _client: &Client, resource: New<ZwlrInputInhibitManagerV1>, _g: &(), data_init: &mut DataInit<'_, Self>) {
        data_init.init(resource, ());
    }
}

impl Dispatch<ZwlrInputInhibitManagerV1, ()> for Glower {
    fn request(
        state: &mut Self,
        client: &Client,
        resource: &ZwlrInputInhibitManagerV1,
        request: zwlr_input_inhibit_manager_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let zwlr_input_inhibit_manager_v1::Request::GetInhibitor { id } = request {
            if state.inhibitor_state.кто.is_some() {
                resource.post_error(zwlr_input_inhibit_manager_v1::Error::AlreadyInhibited, "ввод уже заперт");
                return;
            }
            data_init.init(id, ());
            state.inhibitor_state.кто = Some(client.id());
            state.запри_ввод(client.id());
        }
    }
}

impl Dispatch<ZwlrInputInhibitorV1, ()> for Glower {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &ZwlrInputInhibitorV1,
        _request: zwlr_input_inhibitor_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
    }

    fn destroyed(state: &mut Self, _client: ClientId, _resource: &ZwlrInputInhibitorV1, _data: &()) {
        state.inhibitor_state.кто = None;
        state.верни_фокус();
    }
}

impl Glower {
    /// Ввод заперт: клавиатура — слою замка этой программы
    fn запри_ввод(&mut self, кто: ClientId) {
        let слой = self.space.outputs().find_map(|экран| {
            smithay::desktop::layer_map_for_output(экран)
                .layers()
                .find(|л| л.wl_surface().client().map(|к| к.id()) == Some(кто.clone()))
                .cloned()
        });
        match слой {
            Some(л) => self.фокус_слою(&л),
            None => {
                if let Some(к) = self.seat.get_keyboard() {
                    к.set_focus(self, None, smithay::utils::SERIAL_COUNTER.next_serial());
                }
            }
        }
    }
}
