//! Указатель мыши из темы курсоров системы (Adwaita), по имени формы

use std::collections::HashMap;

use smithay::{
    backend::{allocator::Fourcc, renderer::element::memory::MemoryRenderBuffer},
    utils::{Point, Physical, Transform},
};

pub struct Указатели {
    тема: xcursor::CursorTheme,
    размер: u32,
    готовые: HashMap<String, Option<(MemoryRenderBuffer, Point<i32, Physical>)>>,
}

impl Указатели {
    pub fn new() -> Self {
        let тема = std::env::var("XCURSOR_THEME").unwrap_or_else(|_| "Adwaita".into());
        let размер = std::env::var("XCURSOR_SIZE").ok().and_then(|р| р.parse().ok()).unwrap_or(24);
        Self::из(&тема, размер)
    }

    /// Другая тема или размер — «Параметры» → «Мышь»
    pub fn из(тема: &str, размер: u32) -> Self {
        Указатели { тема: xcursor::CursorTheme::load(тема), размер, готовые: HashMap::new() }
    }

    fn загрузи(&self, имя: &str, масштаб: u32) -> Option<(MemoryRenderBuffer, Point<i32, Physical>)> {
        let путь = self.тема.load_icon(имя)?;
        let байты = std::fs::read(путь).ok()?;
        let картинки = xcursor::parser::parse_xcursor(&байты)?;
        let нужен = self.размер * масштаб;
        // Ближайший размер к нужному, первый кадр (анимированные — без анимации)
        let к = картинки.iter().min_by_key(|к| (к.size as i64 - нужен as i64).abs())?;
        let буфер = MemoryRenderBuffer::from_slice(
            &к.pixels_rgba,
            Fourcc::Abgr8888,
            (к.width as i32, к.height as i32),
            1,
            Transform::Normal,
            None,
        );
        Some((буфер, Point::from((к.xhot as i32, к.yhot as i32))))
    }

    /// Буфер формы и её «остриё». Нет такой формы в теме — обычная стрелка
    pub fn форма(&mut self, имя: &str, масштаб: u32) -> Option<(MemoryRenderBuffer, Point<i32, Physical>)> {
        let ключ = format!("{имя}@{масштаб}");
        if !self.готовые.contains_key(&ключ) {
            let найдено = self
                .загрузи(имя, масштаб)
                .or_else(|| self.загрузи("default", масштаб))
                .or_else(|| self.загрузи("left_ptr", масштаб))
                .or_else(|| Some(запасная_стрелка()));
            self.готовые.insert(ключ.clone(), найдено);
        }
        self.готовые.get(&ключ).cloned().flatten()
    }
}

/// Если темы курсоров нет вовсе — простая белая стрелка с чёрной каймой
fn запасная_стрелка() -> (MemoryRenderBuffer, Point<i32, Physical>) {
    const Р: usize = 20;
    let mut пиксели = vec![0u8; Р * Р * 4];
    for y in 0..Р {
        for x in 0..Р {
            // Треугольник: x ≤ y/1.6 и не ниже 18-й строки
            let внутри = (x as f32) <= y as f32 / 1.6 && y < 18;
            let кайма = внутри && ((x as f32) > y as f32 / 1.6 - 1.4 || x == 0 || y == 17);
            if внутри {
                let i = (y * Р + x) * 4;
                let ц = if кайма { 0 } else { 255 };
                пиксели[i..i + 4].copy_from_slice(&[ц, ц, ц, 255]);
            }
        }
    }
    let буфер = MemoryRenderBuffer::from_slice(&пиксели, Fourcc::Abgr8888, (Р as i32, Р as i32), 1, Transform::Normal, None);
    (буфер, Point::from((0, 0)))
}
