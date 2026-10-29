//! Анимации окон: появление, сворачивание к доку, закрытие, смена стола.
//! Окно не двигаем на самом деле — только рисуем его иначе, пока идёт ход

use std::time::{Duration, Instant};

use smithay::{
    backend::renderer::gles::GlesTexture,
    desktop::Window,
    utils::{Logical, Physical, Point, Rectangle},
};

pub const ПОЯВЛЕНИЕ: Duration = Duration::from_millis(200);
pub const СВОРАЧИВАНИЕ: Duration = Duration::from_millis(240);
pub const ЗАКРЫТИЕ: Duration = Duration::from_millis(180);
/// Смена стола: окна едут на всю ширину экрана — дольше прочих, иначе
/// глаз не успевает понять, куда делось старое и откуда пришло новое
pub const СТОЛ: Duration = Duration::from_millis(420);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Вид {
    Появление,
    Стол { вправо: bool },
}

#[derive(Debug)]
pub struct Ход {
    pub окно: Window,
    pub вид: Вид,
    pub начало: Instant,
}

/// Окно, которого уже нет на месте: свёрнутое летит к доку, закрытое гаснет
#[derive(Debug)]
pub struct Призрак {
    /// Свёрнутое ещё живо — рисуем его самого
    pub окно: Option<Window>,
    /// Закрытое — снимок последнего кадра
    pub снимок: Option<(GlesTexture, Rectangle<i32, Physical>)>,
    pub где: Rectangle<i32, Logical>,
    pub к_доку: bool,
    /// Окно ушедшего стола: уезжает за край (Some(true) — влево)
    pub уезжает: Option<bool>,
    pub начало: Instant,
}

#[derive(Debug, Default)]
pub struct Анимации {
    pub ходы: Vec<Ход>,
    pub призраки: Vec<Призрак>,
    /// Анимации можно выключить: GLOWER_WM_NO_ANIM=1 или настройка «без анимаций»
    pub выключены: bool,
}

/// Плавный выход в конце: быстро начинается, мягко останавливается
pub fn плавно(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Плавно с обеих сторон: разгон и торможение — для смены стола, где
/// окна проходят весь экран и резкий старт выглядит рывком
pub fn мягко(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Как нарисовать окно сейчас
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Вид2 {
    pub прозрачность: f32,
    pub масштаб: f64,
    pub сдвиг: Point<i32, Logical>,
}

impl Default for Вид2 {
    fn default() -> Self {
        Вид2 { прозрачность: 1.0, масштаб: 1.0, сдвиг: Point::default() }
    }
}

impl Анимации {
    fn добавь(&mut self, окно: &Window, вид: Вид) {
        if self.выключены {
            return;
        }
        self.ходы.retain(|х| &х.окно != окно);
        self.ходы.push(Ход { окно: окно.clone(), вид, начало: Instant::now() });
    }

    pub fn появилось(&mut self, окно: &Window) {
        self.добавь(окно, Вид::Появление);
    }

    pub fn сменился_стол(&mut self, окно: &Window, вправо: bool) {
        // Быстро вернулись на стол, откуда окно ещё уезжает, — второго
        // такого же, уезжающего, не рисуем
        self.призраки.retain(|п| п.уезжает.is_none() || п.окно.as_ref() != Some(окно));
        self.добавь(окно, Вид::Стол { вправо });
    }

    /// Окно стола, с которого уходим: его уже нет на экране, но пока он
    /// уезжает, рисуем его призраком. `влево` — новый стол справа
    pub fn ушло_со_стола(&mut self, окно: &Window, где: Rectangle<i32, Logical>, влево: bool) {
        if self.выключены {
            return;
        }
        self.призраки.retain(|п| п.окно.as_ref() != Some(окно));
        self.призраки.push(Призрак { окно: Some(окно.clone()), снимок: None, где, к_доку: false, уезжает: Some(влево), начало: Instant::now() });
    }

    pub fn переехало(&mut self, _окно: &Window) {}

    pub fn свернулось(&mut self, окно: &Window, где: Option<Rectangle<i32, Logical>>) {
        self.ходы.retain(|х| &х.окно != окно);
        if self.выключены {
            return;
        }
        if let Some(где) = где {
            self.призраки.push(Призрак { окно: Some(окно.clone()), снимок: None, где, к_доку: true, уезжает: None, начало: Instant::now() });
        }
    }

    pub fn закрылось(&mut self, снимок: GlesTexture, физ: Rectangle<i32, Physical>, где: Rectangle<i32, Logical>) {
        if self.выключены {
            return;
        }
        self.призраки.push(Призрак { окно: None, снимок: Some((снимок, физ)), где, к_доку: false, уезжает: None, начало: Instant::now() });
    }

    /// Вид окна в этот миг. Ширина экрана нужна для сдвига при смене стола
    pub fn вид(&self, окно: &Window, ширина: i32) -> Вид2 {
        let Some(х) = self.ходы.iter().find(|х| &х.окно == окно) else { return Вид2::default() };
        let прошло = х.начало.elapsed().as_secs_f64();
        match х.вид {
            Вид::Появление => {
                let t = плавно(прошло / ПОЯВЛЕНИЕ.as_secs_f64());
                Вид2 { прозрачность: t as f32, масштаб: 0.92 + 0.08 * t, сдвиг: Point::default() }
            }
            Вид::Стол { вправо } => {
                // Въезжает из-за края экрана: весь путь — ширина экрана
                let t = мягко(прошло / СТОЛ.as_secs_f64());
                let путь = (ширина as f64 * (1.0 - t)) as i32;
                Вид2 { прозрачность: 1.0, масштаб: 1.0, сдвиг: Point::from((if вправо { путь } else { -путь }, 0)) }
            }
        }
    }

    /// Вид призрака: доля пройденного пути (0..1)
    pub fn доля(призрак: &Призрак) -> f64 {
        let длится = if призрак.уезжает.is_some() {
            СТОЛ
        } else if призрак.к_доку {
            СВОРАЧИВАНИЕ
        } else {
            ЗАКРЫТИЕ
        };
        призрак.начало.elapsed().as_secs_f64() / длится.as_secs_f64()
    }

    /// Убрать законченные; true — что-то ещё идёт и нужен следующий кадр
    pub fn шаг(&mut self) -> bool {
        self.ходы.retain(|х| {
            let длится = match х.вид {
                Вид::Появление => ПОЯВЛЕНИЕ,
                Вид::Стол { .. } => СТОЛ,
            };
            х.начало.elapsed() < длится && х.окно.alive()
        });
        self.призраки.retain(|п| Self::доля(п) < 1.0);
        !self.ходы.is_empty() || !self.призраки.is_empty()
    }
}

use smithay::utils::IsAlive;

#[cfg(test)]
mod проверки {
    use super::*;

    #[test]
    fn мягкость_в_границах() {
        assert_eq!(мягко(0.0), 0.0);
        assert!((мягко(1.0) - 1.0).abs() < 1e-9);
        assert!((мягко(0.5) - 0.5).abs() < 1e-9, "середина пути — в середине времени");
        assert!(мягко(0.1) < 0.1, "разгон: в начале медленнее равномерного");
    }

    #[test]
    fn плавность_в_границах() {
        assert_eq!(плавно(0.0), 0.0);
        assert_eq!(плавно(1.0), 1.0);
        assert_eq!(плавно(2.0), 1.0);
        assert!(плавно(0.5) > 0.5, "к середине пройдено больше половины");
    }
}
