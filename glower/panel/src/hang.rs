//! «Программа не отвечает» — как в Windows.
//!
//! Зовёт оконный сервер (glower-wm, hang.rs): программа не отвечает, а
//! человек пытается её закрыть. `glower-hang PID «Заголовок»`.
//!
//! Закрыть принудительно — сигнал KILL: зависшая программа просьбу
//! TERM не прочтёт, как не прочла крестик. Свои части системы, чужие
//! процессы и процессы других людей не трогаем. Программа очнулась и
//! закрылась сама — окно вопроса уходит.

use gtk::glib;
use gtk::prelude::*;
use std::time::Duration;

extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
    fn getuid() -> u32;
}

/// Наши части системы — их «закрыть принудительно» уронило бы сеанс
const СВОИ: &[&str] = &["glower-panel", "glower-wm", "Xwayland", "labwc", "node", "systemd", "greetd"];

fn жив(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

/// Можно ли закрывать: свой человек, не часть системы
pub fn можно_закрыть(pid: u32, мой_uid: u32) -> Result<(), String> {
    if pid <= 1 || pid == std::process::id() {
        return Err("это часть системы".into());
    }
    let статус = std::fs::read_to_string(format!("/proc/{pid}/status")).map_err(|_| "программы уже нет".to_string())?;
    let uid = статус
        .lines()
        .find_map(|с| с.strip_prefix("Uid:"))
        .and_then(|с| с.split_whitespace().next())
        .and_then(|с| с.parse::<u32>().ok())
        .ok_or("не узнать, чья программа")?;
    if uid != мой_uid {
        return Err("программа запущена не вами".into());
    }
    let имя = std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default();
    if СВОИ.iter().any(|с| имя.trim() == *с) {
        return Err("это часть системы".into());
    }
    Ok(())
}

pub fn запусти(доводы: Vec<String>) -> ! {
    let pid = доводы.first().and_then(|п| п.parse::<u32>().ok()).unwrap_or(0);
    let заголовок = доводы.get(1).cloned().unwrap_or_default();
    let мой = unsafe { getuid() };
    if let Err(е) = можно_закрыть(pid, мой) {
        eprintln!("glower-hang: {pid}: {е}");
        std::process::exit(1);
    }
    // Один вопрос на программу: второй крестик не плодит окна
    let замок = glib::user_runtime_dir().join(format!("glower-hang-{pid}"));
    if let Some(кто) = std::fs::read_to_string(&замок).ok().and_then(|т| т.trim().parse::<u32>().ok()) {
        if кто != std::process::id() && жив(кто) {
            std::process::exit(0);
        }
    }
    let _ = std::fs::write(&замок, std::process::id().to_string());

    crate::apps::подготовь("glower-hang", "Программа не отвечает");
    let имя = if заголовок.trim().is_empty() { "Программа".to_string() } else { format!("«{}»", заголовок.trim()) };
    let д = gtk::MessageDialog::new(None::<&gtk::Window>, gtk::DialogFlags::MODAL, gtk::MessageType::Warning, gtk::ButtonsType::None, &format!("{имя} не отвечает"));
    д.set_secondary_text(Some("Можно подождать — программа может очнуться сама. Если закрыть её сейчас, несохранённые данные пропадут."));
    д.add_button("Подождать", gtk::ResponseType::Cancel);
    д.add_button("Закрыть программу", gtk::ResponseType::Accept);
    д.set_default_response(gtk::ResponseType::Cancel);
    д.set_keep_above(true);
    д.set_position(gtk::WindowPosition::Center);
    // Программа очнулась и закрылась сама — спрашивать больше не о чем
    let д2 = д.clone();
    glib::timeout_add_local(Duration::from_secs(1), move || {
        if !жив(pid) {
            д2.response(gtk::ResponseType::Cancel);
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    });
    let ответ = д.run();
    if ответ == gtk::ResponseType::Accept && жив(pid) && можно_закрыть(pid, мой).is_ok() {
        unsafe {
            kill(pid as i32, 9);
        }
    }
    let _ = std::fs::remove_file(&замок);
    std::process::exit(0);
}

#[cfg(test)]
mod проверки {
    #[test]
    fn своё_и_чужое_не_закрывает() {
        let мой = unsafe { super::getuid() };
        assert!(super::можно_закрыть(1, мой).is_err());
        assert!(super::можно_закрыть(0, мой).is_err());
        assert!(super::можно_закрыть(std::process::id(), мой).is_err());
        assert!(super::можно_закрыть(u32::MAX - 7, мой).is_err());
        // Чужой человек: тот же процесс, но другой uid
        let mut ребёнок = std::process::Command::new("sleep").arg("5").spawn().unwrap();
        assert!(super::можно_закрыть(ребёнок.id(), мой).is_ok());
        assert!(super::можно_закрыть(ребёнок.id(), мой + 1).is_err());
        let _ = ребёнок.kill();
    }
}
