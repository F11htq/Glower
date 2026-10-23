#![allow(confusable_idents, mixed_script_confusables)]

//! glower-wm — свой оконный сервер GlowerOS на Smithay: Wayland, Xwayland,
//! размытие под панелью и меню, анимации окон.
//!
//! Запускается так же, как labwc, и понимает его настройки:
//!
//!     glower-wm [-C папка] [-s команда]   — сеанс: rc.xml и environment из папки
//!     glower-wm -e                        — завершить работающий сервер
//!
//! На машине с видеокартой рисует прямо на ней (DRM), а если запущен внутри
//! другого экрана (есть WAYLAND_DISPLAY или DISPLAY) — окном в нём.

mod anim;
mod backend;
mod blur;
mod cursor;
mod focus;
mod grabs;
mod handlers;
mod input;
mod ipc;
mod keys;
mod protocols;
mod render;
mod state;
mod udev;
mod winit;
mod xwayland;

use std::path::PathBuf;

use smithay::reexports::{calloop::EventLoop, wayland_server::Display};

use state::Glower;

fn main() {
    let доводы: Vec<String> = std::env::args().skip(1).collect();
    if доводы.iter().any(|д| д == "-e" || д == "--exit") {
        выйти();
        return;
    }
    if доводы.iter().any(|д| д == "-h" || д == "--help") {
        println!("glower-wm [-C папка-настроек] [-s команда-запуска] [--winit|--drm] | -e");
        return;
    }
    let значение = |ключ: &str| доводы.iter().position(|д| д == ключ).and_then(|i| доводы.get(i + 1)).cloned();
    let папка = значение("-C")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("GLOWER_LABWC").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/usr/share/glower/labwc"));
    let запуск = значение("-s");

    let фильтр = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,glower_wm=info"));
    tracing_subscriber::fmt().with_env_filter(фильтр).with_writer(std::io::stderr).init();

    let вложенно = доводы.iter().any(|д| д == "--winit")
        || (!доводы.iter().any(|д| д == "--drm") && (std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some()));

    let mut цикл: EventLoop<'static, Glower> = EventLoop::try_new().expect("цикл событий");
    let display: Display<Glower> = Display::new().expect("экран Wayland");
    let имя_места = if вложенно { "winit" } else { "seat0" };
    let mut состояние = Glower::new(display, цикл.handle(), цикл.get_signal(), папка, имя_места);
    состояние.анимации.выключены = std::env::var_os("GLOWER_WM_NO_ANIM").is_some();

    let итог = if вложенно { winit::запусти(&mut цикл, &mut состояние) } else { udev::запусти(&mut цикл, &mut состояние) };
    if let Err(е) = итог {
        tracing::error!("экран не поднялся: {е}");
        std::process::exit(1);
    }

    // Окружение для программ сеанса
    std::env::set_var("WAYLAND_DISPLAY", &состояние.socket_name);
    std::env::set_var("GLOWER_WM_PID", std::process::id().to_string());
    std::env::set_var("XDG_SESSION_TYPE", "wayland");
    if let Some(путь) = ipc::слушай(&mut состояние) {
        std::env::set_var("GLOWER_WM_SOCKET", путь);
    }
    if std::env::var_os("GLOWER_WM_NO_X").is_none() {
        xwayland::запусти(&mut состояние);
    }
    tracing::info!("glower-wm слушает {:?}", состояние.socket_name);

    // SIGHUP — перечитать настройки (так агент меняет раскладки), SIGTERM — выйти
    use calloop::signals::{Signal, Signals};
    if let Ok(сигналы) = Signals::new(&[Signal::SIGHUP, Signal::SIGTERM, Signal::SIGINT]) {
        let _ = цикл.handle().insert_source(сигналы, |событие, _, с| match событие.signal() {
            Signal::SIGHUP => с.перечитай_настройки(),
            _ => с.loop_signal.stop(),
        });
    }

    if let Some(команда) = запуск {
        input::запусти(&команда);
    }

    let итог = цикл.run(None, &mut состояние, |с| {
        с.приберись();
        if с.нужен_кадр || !с.screencopy_state.ждут.is_empty() {
            match с.бэкенд.as_ref() {
                Some(backend::Бэкенд::Winit(w)) => w.backend.window().request_redraw(),
                Some(backend::Бэкенд::Udev(_)) => udev::нарисуй(с),
                None => {}
            }
        }
    });
    if let Err(е) = итог {
        tracing::error!("цикл событий: {е}");
    }
    // Разбирать всё по порядку незачем: программы и Xwayland сами увидят,
    // что сервера нет. А ждать чужие потоки при выходе — риск повиснуть
    std::process::exit(0);
}

/// glower-wm -e: попросить работающий сервер закончить, как labwc -e
fn выйти() {
    let Some(pid) = std::env::var("GLOWER_WM_PID").ok().and_then(|п| п.parse::<i32>().ok()) else {
        eprintln!("glower-wm: не знаю, какой сервер завершать (нет GLOWER_WM_PID)");
        std::process::exit(1);
    };
    // Safety: просто сигнал процессу
    unsafe {
        kill(pid, 15);
    }
}

extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}
