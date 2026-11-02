//! Зависшие программы — как в Windows.
//!
//! Раз в пару секунд сервер окликает каждую программу с окнами Wayland
//! (ping протокола xdg_wm_base). Не ответила за пять секунд — она «не
//! отвечает»: окно белеет, в заголовке на панели — «(не отвечает)».
//! Ответила — всё возвращается само.
//!
//! Крестик у зависшей программы ничего не сделает: она не читает просьбы.
//! Поэтому, если окно не закрылось через пять секунд после просьбы и
//! программа не отвечает (а у программ X11, которых окликнуть так нельзя, —
//! если окно всё ещё на месте и новых окон у программы не появилось, то
//! есть она не спрашивает «Сохранить изменения?»), сервер показывает окно
//! «Программа не отвечает» с выбором: закрыть принудительно или подождать.
//! Крестик по уже «не отвечающему» окну — сразу этот вопрос.
//!
//! Само окно вопроса рисует панель (glower-hang); закрывает программу тоже
//! она — сигналом по номеру процесса.

use std::time::{Duration, Instant};

use smithay::{
    desktop::Window,
    reexports::wayland_server::Resource,
    utils::SERIAL_COUNTER,
    wayland::shell::xdg::ShellClient,
};

use crate::state::Glower;

/// Сколько ждать ответа, прежде чем считать программу зависшей
pub const ЖДАТЬ: Duration = Duration::from_secs(5);
/// Как часто окликать
const ОКЛИК: Duration = Duration::from_secs(2);

/// Программу представляет одно её окно: ShellClient не копируется, а из
/// окна его можно достать в любой момент
struct Оклик {
    окно: Window,
    послан: Option<Instant>,
    последний: Instant,
}

struct Закрытие {
    окно: Window,
    когда: Instant,
    спрошено: bool,
    /// Окон X11 у этой программы в момент просьбы — новое окно после
    /// просьбы значит «Сохранить изменения?», а не зависание
    окон_было: usize,
}

#[derive(Default)]
pub struct Зависания {
    оклики: Vec<Оклик>,
    не_отвечают: Vec<Window>,
    закрытия: Vec<Закрытие>,
}

fn клиент(окно: &Window) -> Option<ShellClient> {
    окно.toplevel().map(|t| t.client())
}

impl Зависания {
    pub fn не_отвечает(&self, окно: &Window) -> bool {
        клиент(окно).is_some_and(|к| self.не_отвечают.iter().any(|н| клиент(н).as_ref() == Some(&к)))
    }

    /// Ответ на оклик пришёл
    pub fn ответил(&mut self, к: &ShellClient) -> bool {
        if let Some(о) = self.оклики.iter_mut().find(|о| клиент(&о.окно).as_ref() == Some(к)) {
            о.послан = None;
        }
        let было = self.не_отвечают.len();
        self.не_отвечают.retain(|н| клиент(н).as_ref() != Some(к));
        было != self.не_отвечают.len()
    }
}

fn pid_окна(с: &Glower, окно: &Window) -> Option<u32> {
    if let Some(t) = окно.toplevel() {
        let клиент = t.wl_surface().client()?;
        return клиент.get_credentials(&с.display_handle).ok().map(|к| к.pid as u32).filter(|п| *п > 1);
    }
    окно.x11_surface().and_then(|x| x.pid()).filter(|п| *п > 1)
}

fn заголовок(окно: &Window) -> String {
    if let Some(t) = окно.toplevel() {
        return smithay::wayland::compositor::with_states(t.wl_surface(), |с| {
            с.data_map
                .get::<smithay::wayland::shell::xdg::XdgToplevelSurfaceData>()
                .and_then(|д| д.lock().ok()?.title.clone())
                .unwrap_or_default()
        });
    }
    окно.x11_surface().map(|x| x.title()).unwrap_or_default()
}

fn окон_x11_у(с: &Glower, pid: Option<u32>) -> usize {
    let Some(pid) = pid else { return 0 };
    с.окна.iter().filter(|з| з.окно.x11_surface().and_then(|x| x.pid()) == Some(pid)).count()
}

impl Glower {
    /// Крестик, Alt+F4 или «Закрыть» на панели: просьбу шлём как обычно и
    /// следим, закрылось ли
    pub fn следи_за_закрытием(&mut self, окно: &Window) {
        if self.зависания.не_отвечает(окно) {
            self.спроси_про_зависшее(окно);
            return;
        }
        let pid = окно.x11_surface().and_then(|x| x.pid());
        let окон_было = окон_x11_у(self, pid);
        self.зависания.закрытия.retain(|з| &з.окно != окно);
        self.зависания.закрытия.push(Закрытие { окно: окно.clone(), когда: Instant::now(), спрошено: false, окон_было });
    }

    fn спроси_про_зависшее(&mut self, окно: &Window) {
        let Some(pid) = pid_окна(self, окно) else {
            tracing::warn!("не отвечает окно без номера процесса — закрыть принудительно нечем");
            return;
        };
        // Себя и Xwayland не трогаем ни при каких условиях
        if pid == std::process::id() {
            return;
        }
        let имя = заголовок(окно);
        match std::process::Command::new("glower-hang").arg(pid.to_string()).arg(&имя).spawn() {
            Ok(_) => tracing::info!("программа {pid} «{имя}» не отвечает — спрашиваем"),
            Err(е) => tracing::warn!("glower-hang не запустился: {е}"),
        }
    }

    /// Раз в секунду: окликнуть, отметить зависших, спросить про незакрывшиеся
    pub fn проверь_зависания(&mut self) {
        let сейчас = Instant::now();
        // Кого окликать: по программе (одно окно на программу), а не по окну
        let mut представители: Vec<Window> = vec![];
        for з in &self.окна {
            if let Some(к) = клиент(&з.окно) {
                if к.alive() && !представители.iter().any(|п| клиент(п).as_ref() == Some(&к)) {
                    представители.push(з.окно.clone());
                }
            }
        }
        let есть = |о: &Window| клиент(о).is_some_and(|к| представители.iter().any(|п| клиент(п).as_ref() == Some(&к)));
        self.зависания.оклики.retain(|о| есть(&о.окно));
        self.зависания.не_отвечают.retain(|о| есть(о));
        let mut изменилось = false;
        for п in представители {
            let Some(к) = клиент(&п) else { continue };
            if !self.зависания.оклики.iter().any(|о| клиент(&о.окно).as_ref() == Some(&к)) {
                self.зависания.оклики.push(Оклик { окно: п.clone(), послан: None, последний: сейчас - ОКЛИК });
            }
            let о = self.зависания.оклики.iter_mut().find(|о| клиент(&о.окно).as_ref() == Some(&к)).expect("только что добавили");
            match о.послан {
                Some(когда) if сейчас - когда > ЖДАТЬ => {
                    if !self.зависания.не_отвечает(&п) {
                        self.зависания.не_отвечают.push(п.clone());
                        изменилось = true;
                    }
                }
                Some(_) => {}
                None if сейчас - о.последний >= ОКЛИК => {
                    if к.send_ping(SERIAL_COUNTER.next_serial()).is_ok() {
                        о.послан = Some(сейчас);
                        о.последний = сейчас;
                    }
                }
                None => {}
            }
        }

        // Незакрывшиеся окна
        let живые: Vec<Window> = self.окна.iter().map(|з| з.окно.clone()).collect();
        self.зависания.закрытия.retain(|з| живые.contains(&з.окно));
        let mut спросить = vec![];
        for i in 0..self.зависания.закрытия.len() {
            let (окно, когда, спрошено, окон_было) = {
                let з = &self.зависания.закрытия[i];
                (з.окно.clone(), з.когда, з.спрошено, з.окон_было)
            };
            if спрошено || сейчас - когда < ЖДАТЬ {
                continue;
            }
            let завис = if окно.toplevel().is_some() {
                self.зависания.не_отвечает(&окно)
            } else {
                let pid = окно.x11_surface().and_then(|x| x.pid());
                окон_x11_у(self, pid) <= окон_было
            };
            if завис {
                self.зависания.закрытия[i].спрошено = true;
                спросить.push(окно);
            } else if окно.toplevel().is_some() {
                // Отвечает, но не закрылась — спрашивает что-то у человека.
                // Больше не следим: решать ему
                self.зависания.закрытия[i].спрошено = true;
            }
        }
        for окно in спросить {
            self.спроси_про_зависшее(&окно);
        }
        if изменилось {
            self.отметь_зависшие();
        }
    }

    pub fn ответ_на_оклик(&mut self, к: &ShellClient) {
        if self.зависания.ответил(к) {
            self.отметь_зависшие();
        }
    }

    /// Окна зависших программ: бледнеют, на панели — «(не отвечает)»
    fn отметь_зависшие(&mut self) {
        let флаги: Vec<bool> = self.окна.iter().map(|з| self.зависания.не_отвечает(&з.окно)).collect();
        for (з, ф) in self.окна.iter_mut().zip(флаги) {
            з.не_отвечает = ф;
        }
        self.foreign_toplevel_state.обнови_все(&self.space, &self.окна);
        self.нужен_кадр = true;
    }
}
