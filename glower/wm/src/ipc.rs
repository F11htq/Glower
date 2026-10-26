//! Просьбы панели к серверу — строками через сокет, а не нажатиями клавиш.
//! Под labwc панель «нажимала» служебные сочетания через wtype; здесь можно
//! просто сказать: «desk 2», «send 3».
//!
//!     desk N    — перейти на стол N (с единицы)
//!     send N    — отправить окно в фокусе на стол N
//!     layout    — текущая раскладка (ответом)
//!     layouts   — все раскладки: номер текущей (с единицы), затем имена,
//!                 через табуляцию — для меню раскладок на панели
//!     layout-set N — включить раскладку N (с единицы)
//!     paste     — вставить в окно в фокусе (Ctrl+V, в терминале —
//!                 Ctrl+Shift+V): так история буфера Win+V вставляет
//!                 выбранное. wtype тут не годится — модификаторы с его
//!                 виртуальной клавиатуры до программы не доходили
//!     reload    — перечитать rc.xml и environment
//!     quit      — завершить сеанс

use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    time::Duration,
};

use smithay::reexports::calloop::{generic::Generic, Interest, Mode, PostAction};

use crate::state::{папка_выполнения, Glower};

pub fn слушай(состояние: &mut Glower) -> Option<PathBuf> {
    let папка = папка_выполнения()?;
    std::fs::create_dir_all(&папка).ok()?;
    let mut путь = папка.join(format!("{}.sock", состояние.socket_name.to_string_lossy()));
    // Путь сокета в Linux — не длиннее 108 байт; в длинной папке берём /tmp
    if путь.as_os_str().len() >= 100 {
        путь = std::env::temp_dir().join(format!("glower-wm-{}.sock", std::process::id()));
    }
    let _ = std::fs::remove_file(&путь);
    let слушатель = UnixListener::bind(&путь).ok()?;
    слушатель.set_nonblocking(true).ok()?;
    состояние
        .loop_handle
        .insert_source(Generic::new(слушатель, Interest::READ, Mode::Level), |_, слушатель, с| {
            while let Ok((поток, _)) = слушатель.accept() {
                обслужи(с, поток);
            }
            Ok(PostAction::Continue)
        })
        .ok()?;
    Some(путь)
}

fn обслужи(с: &mut Glower, поток: UnixStream) {
    let _ = поток.set_nonblocking(false);
    let _ = поток.set_read_timeout(Some(Duration::from_millis(300)));
    let mut ответ_в = match поток.try_clone() {
        Ok(п) => п,
        Err(_) => return,
    };
    let mut строка = String::new();
    if BufReader::new(поток).read_line(&mut строка).is_err() {
        return;
    }
    let ответ = выполни(с, строка.trim());
    let _ = ответ_в.write_all(ответ.as_bytes());
    let _ = ответ_в.write_all(b"\n");
}

pub fn выполни(с: &mut Glower, команда: &str) -> String {
    let mut части = команда.split_whitespace();
    let что = части.next().unwrap_or("");
    let номер = части.next().and_then(|н| н.parse::<usize>().ok()).filter(|н| *н >= 1).map(|н| н - 1);
    match (что, номер) {
        ("desk", Some(н)) => {
            с.на_стол(н);
            "ok".into()
        }
        ("send", Some(н)) => match с.окно_в_фокусе() {
            Some(о) => {
                с.отправь_на_стол(&о, н);
                "ok".into()
            }
            None => "нет окна в фокусе".into(),
        },
        ("layout", _) => с.раскладка.clone(),
        ("layouts", _) => с.раскладки().join("\t"),
        ("layout-set", Some(н)) => {
            if с.включи_раскладку(н) {
                "ok".into()
            } else {
                "нет такой раскладки".into()
            }
        }
        ("paste", _) => {
            вставь(с);
            "ok".into()
        }
        ("reload", _) => {
            с.перечитай_настройки();
            "ok".into()
        }
        ("quit", _) => {
            с.loop_signal.stop();
            "ok".into()
        }
        _ => format!("не понимаю: {команда}"),
    }
}

/// Нажать Ctrl+V (или Ctrl+Shift+V в терминале) своей же клавиатурой —
/// тем же путём, что и настоящие клавиши, мимо своих сочетаний
fn вставь(с: &mut Glower) {
    use smithay::backend::input::KeyState;
    use smithay::input::keyboard::{FilterResult, Keycode};
    use smithay::utils::SERIAL_COUNTER;
    let терминал = с.окно_в_фокусе().and_then(|о| о.toplevel().cloned()).is_some_and(|t| {
        smithay::wayland::compositor::with_states(t.wl_surface(), |д| {
            д.data_map
                .get::<smithay::wayland::shell::xdg::XdgToplevelSurfaceData>()
                .and_then(|з| з.lock().ok()?.app_id.clone())
                .is_some_and(|id| ["foot", "terminal", "Terminal", "kitty", "alacritty", "konsole"].iter().any(|т| id.contains(т)))
        })
    });
    let Some(клавиатура) = с.seat.get_keyboard() else { return };
    // Коды evdev + 8: левый Ctrl, левый Shift, V
    let (ctrl, shift, v) = (Keycode::new(29 + 8), Keycode::new(42 + 8), Keycode::new(47 + 8));
    let время = с.start_time.elapsed().as_millis() as u32;
    let mut нажми = |код: Keycode, нажато: KeyState| {
        клавиатура.input::<(), _>(с, код, нажато, SERIAL_COUNTER.next_serial(), время, |_, _, _| FilterResult::Forward);
    };
    нажми(ctrl, KeyState::Pressed);
    if терминал {
        нажми(shift, KeyState::Pressed);
    }
    нажми(v, KeyState::Pressed);
    нажми(v, KeyState::Released);
    if терминал {
        нажми(shift, KeyState::Released);
    }
    нажми(ctrl, KeyState::Released);
}
