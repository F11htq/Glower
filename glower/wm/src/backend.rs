//! Где сервер рисует: окном внутри другого экрана (winit — для проверки и
//! для X-сервера на старых машинах) или прямо на видеокарте (DRM)

use smithay::{backend::renderer::gles::GlesRenderer, output::Output};

pub enum Бэкенд {
    Winit(crate::winit::Winit),
    Udev(crate::udev::Udev),
}

impl Бэкенд {
    pub fn renderer(&mut self) -> Option<&mut GlesRenderer> {
        match self {
            Бэкенд::Winit(w) => Some(w.backend.renderer()),
            Бэкенд::Udev(u) => u.renderer(),
        }
    }

    pub fn размер_гаммы(&self, экран: &Output) -> Option<u32> {
        match self {
            Бэкенд::Winit(_) => None,
            Бэкенд::Udev(u) => u.размер_гаммы(экран),
        }
    }

    pub fn задай_гамму(&mut self, экран: &Output, таблица: Option<&[u16]>) -> bool {
        match self {
            Бэкенд::Winit(_) => false,
            Бэкенд::Udev(u) => u.задай_гамму(экран, таблица),
        }
    }
}
