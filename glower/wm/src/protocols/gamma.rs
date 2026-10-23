//! wlr-gamma-control: ночной свет (wlsunset) меняет таблицу гаммы экрана.
//! На настоящем экране (DRM) таблица уходит видеокарте; во вложенном окне
//! менять нечего — отвечаем отказом, и wlsunset это понимает

use std::{os::fd::OwnedFd, os::unix::fs::FileExt};

use smithay::{
    output::Output,
    reexports::{
        wayland_protocols_wlr::gamma_control::v1::server::{
            zwlr_gamma_control_manager_v1::{self, ZwlrGammaControlManagerV1},
            zwlr_gamma_control_v1::{self, ZwlrGammaControlV1},
        },
        wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource},
    },
};

use crate::state::Glower;

pub struct GammaState {
    /// У каждого экрана таблицей правит кто-то один
    pub занято: Vec<(Output, ZwlrGammaControlV1)>,
}

impl GammaState {
    pub fn new(dh: &DisplayHandle) -> Self {
        dh.create_global::<Glower, ZwlrGammaControlManagerV1, _>(1, ());
        GammaState { занято: vec![] }
    }
}

impl GlobalDispatch<ZwlrGammaControlManagerV1, ()> for Glower {
    fn bind(_state: &mut Self, _dh: &DisplayHandle, _client: &Client, resource: New<ZwlrGammaControlManagerV1>, _g: &(), data_init: &mut DataInit<'_, Self>) {
        data_init.init(resource, ());
    }
}

impl Dispatch<ZwlrGammaControlManagerV1, ()> for Glower {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &ZwlrGammaControlManagerV1,
        request: zwlr_gamma_control_manager_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let zwlr_gamma_control_manager_v1::Request::GetGammaControl { id, output } = request {
            let экран = Output::from_resource(&output);
            let упр = data_init.init(id, экран.clone());
            let Some(экран) = экран else {
                упр.failed();
                return;
            };
            if state.gamma_state.занято.iter().any(|(э, _)| э == &экран) {
                упр.failed();
                return;
            }
            let размер = state.бэкенд.as_ref().and_then(|б| б.размер_гаммы(&экран));
            match размер {
                Some(р) if р > 0 => {
                    упр.gamma_size(р);
                    state.gamma_state.занято.push((экран, упр));
                }
                _ => упр.failed(),
            }
        }
    }
}

impl Dispatch<ZwlrGammaControlV1, Option<Output>> for Glower {
    fn request(
        state: &mut Self,
        _client: &Client,
        resource: &ZwlrGammaControlV1,
        request: zwlr_gamma_control_v1::Request,
        data: &Option<Output>,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        let zwlr_gamma_control_v1::Request::SetGamma { fd } = request else { return };
        let Some(экран) = data.as_ref() else { return };
        let Some(размер) = state.бэкенд.as_ref().and_then(|б| б.размер_гаммы(экран)) else {
            resource.failed();
            return;
        };
        match прочти_таблицу(fd, размер as usize) {
            Some(таблица) => {
                let вышло = state.бэкенд.as_mut().map(|б| б.задай_гамму(экран, Some(&таблица))).unwrap_or(false);
                if !вышло {
                    resource.failed();
                }
            }
            None => resource.post_error(zwlr_gamma_control_v1::Error::InvalidGamma, "таблица гаммы не того размера"),
        }
    }

    fn destroyed(state: &mut Self, _client: smithay::reexports::wayland_server::backend::ClientId, resource: &ZwlrGammaControlV1, data: &Option<Output>) {
        state.gamma_state.занято.retain(|(_, у)| у != resource);
        // Хозяин ушёл — экрану обычные цвета
        if let Some(экран) = data {
            if let Some(б) = state.бэкенд.as_mut() {
                б.задай_гамму(экран, None);
            }
        }
    }
}

/// Таблица: красные, зелёные, синие — по `размер` чисел u16 подряд
fn прочти_таблицу(fd: OwnedFd, размер: usize) -> Option<Vec<u16>> {
    // Читаем с начала файла: программа могла оставить позицию в конце
    let файл = std::fs::File::from(fd);
    let mut байты = vec![0u8; размер * 3 * 2];
    файл.read_exact_at(&mut байты, 0).ok()?;
    Some(байты.chunks_exact(2).map(|п| u16::from_ne_bytes([п[0], п[1]])).collect())
}
