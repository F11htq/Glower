//! wlr-screencopy: снимок экрана для grim («Ножницы», PrintScreen).
//! Отдаём кадр в общую память программы (wl_shm), без dmabuf

use std::time::{SystemTime, UNIX_EPOCH};

use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            damage::OutputDamageTracker,
            element::RenderElement,
            gles::{GlesRenderer, GlesTexture},
            Bind, Color32F, ExportMem, Offscreen,
        },
    },
    output::Output,
    reexports::{
        wayland_protocols_wlr::screencopy::v1::server::{
            zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1},
            zwlr_screencopy_manager_v1::{self, ZwlrScreencopyManagerV1},
        },
        wayland_server::{
            protocol::{wl_buffer::WlBuffer, wl_shm},
            Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
        },
    },
    utils::{Physical, Rectangle, Scale, Size, Transform},
    wayland::shm::{with_buffer_contents, with_buffer_contents_mut},
};

use crate::state::Glower;

#[derive(Debug, Clone)]
pub struct ДанныеКадра {
    pub экран: Output,
    pub область: Rectangle<i32, Physical>,
    pub с_указателем: bool,
}

pub struct Запрос {
    pub кадр: ZwlrScreencopyFrameV1,
    pub буфер: WlBuffer,
    pub данные: ДанныеКадра,
}

pub struct ScreencopyState {
    pub ждут: Vec<Запрос>,
}

impl ScreencopyState {
    pub fn new(dh: &DisplayHandle) -> Self {
        dh.create_global::<Glower, ZwlrScreencopyManagerV1, _>(3, ());
        ScreencopyState { ждут: vec![] }
    }

    pub fn есть_для(&self, экран: &Output) -> bool {
        self.ждут.iter().any(|з| &з.данные.экран == экран)
    }
}

fn размер_экрана(экран: &Output) -> Size<i32, Physical> {
    экран.current_mode().map(|р| р.size).unwrap_or_default()
}

impl GlobalDispatch<ZwlrScreencopyManagerV1, ()> for Glower {
    fn bind(_state: &mut Self, _dh: &DisplayHandle, _client: &Client, resource: New<ZwlrScreencopyManagerV1>, _g: &(), data_init: &mut DataInit<'_, Self>) {
        data_init.init(resource, ());
    }
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for Glower {
    fn request(
        state: &mut Self,
        _client: &Client,
        resource: &ZwlrScreencopyManagerV1,
        request: zwlr_screencopy_manager_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        use zwlr_screencopy_manager_v1::Request as З;
        let (кадр, курсор, вывод, область) = match request {
            З::CaptureOutput { frame, overlay_cursor, output } => (frame, overlay_cursor, output, None),
            З::CaptureOutputRegion { frame, overlay_cursor, output, x, y, width, height } => {
                (frame, overlay_cursor, output, Some(Rectangle::<i32, smithay::utils::Logical>::new((x, y).into(), (width, height).into())))
            }
            _ => return,
        };
        let Some(экран) = Output::from_resource(&вывод) else {
            let к = data_init.init(кадр, None);
            к.failed();
            return;
        };
        let размер = размер_экрана(&экран);
        let масштаб = экран.current_scale().fractional_scale();
        let mut прямоугольник = Rectangle::<i32, Physical>::from_size(размер);
        if let Some(о) = область {
            let ф = о.to_physical_precise_round(масштаб);
            match ф.intersection(прямоугольник) {
                Some(п) if !п.is_empty() => прямоугольник = п,
                _ => {
                    let к = data_init.init(кадр, None);
                    к.failed();
                    return;
                }
            }
        }
        let данные = ДанныеКадра { экран, область: прямоугольник, с_указателем: курсор != 0 };
        let к = data_init.init(кадр, Some(данные));
        let (ш, в) = (прямоугольник.size.w as u32, прямоугольник.size.h as u32);
        к.buffer(wl_shm::Format::Xrgb8888, ш, в, ш * 4);
        if resource.version() >= 3 {
            к.buffer_done();
        }
        let _ = state;
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, Option<ДанныеКадра>> for Glower {
    fn request(
        state: &mut Self,
        _client: &Client,
        resource: &ZwlrScreencopyFrameV1,
        request: zwlr_screencopy_frame_v1::Request,
        data: &Option<ДанныеКадра>,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        use zwlr_screencopy_frame_v1::Request as З;
        let буфер = match request {
            З::Copy { buffer } | З::CopyWithDamage { buffer } => buffer,
            _ => return,
        };
        let Some(данные) = data.clone() else {
            resource.failed();
            return;
        };
        let годится = with_buffer_contents(&буфер, |_, _, д| {
            д.width == данные.область.size.w
                && д.height == данные.область.size.h
                && д.stride >= д.width * 4
                && matches!(д.format, wl_shm::Format::Xrgb8888 | wl_shm::Format::Argb8888)
        })
        .unwrap_or(false);
        if !годится {
            resource.post_error(zwlr_screencopy_frame_v1::Error::InvalidBuffer, "буфер не того размера или вида");
            return;
        }
        state.screencopy_state.ждут.push(Запрос { кадр: resource.clone(), буфер, данные });
        state.нужен_кадр = true;
    }

    fn destroyed(state: &mut Self, _client: smithay::reexports::wayland_server::backend::ClientId, resource: &ZwlrScreencopyFrameV1, _data: &Option<ДанныеКадра>) {
        state.screencopy_state.ждут.retain(|з| &з.кадр != resource);
    }
}

/// Выполнить ждущие снимки этого экрана: рисуем кадр заново в текстуру и
/// копируем пиксели в буфер программы
pub fn сними<E: RenderElement<GlesRenderer>>(
    состояние: &mut Glower,
    renderer: &mut GlesRenderer,
    экран: &Output,
    с_указателем: &[E],
    без_указателя: &[E],
) {
    let запросы: Vec<Запрос> = {
        let (наши, прочие): (Vec<_>, Vec<_>) = std::mem::take(&mut состояние.screencopy_state.ждут).into_iter().partition(|з| &з.данные.экран == экран);
        состояние.screencopy_state.ждут = прочие;
        наши
    };
    if запросы.is_empty() {
        return;
    }
    let размер = размер_экрана(экран);
    let масштаб = Scale::from(экран.current_scale().fractional_scale());
    for з in запросы {
        let элементы = if з.данные.с_указателем { с_указателем } else { без_указателя };
        match сними_один(renderer, размер, масштаб, элементы, &з) {
            Ok(()) => {
                let сейчас = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
                з.кадр.flags(zwlr_screencopy_frame_v1::Flags::empty());
                let с = сейчас.as_secs();
                з.кадр.ready((с >> 32) as u32, (с & 0xffff_ffff) as u32, сейчас.subsec_nanos());
            }
            Err(е) => {
                tracing::warn!("снимок экрана не вышел: {е}");
                з.кадр.failed();
            }
        }
    }
}

fn сними_один<E: RenderElement<GlesRenderer>>(
    renderer: &mut GlesRenderer,
    размер: Size<i32, Physical>,
    масштаб: Scale<f64>,
    элементы: &[E],
    з: &Запрос,
) -> Result<(), String> {
    let mut текстура: GlesTexture = renderer
        .create_buffer(Fourcc::Abgr8888, размер.to_logical(1).to_buffer(1, Transform::Normal))
        .map_err(|е| е.to_string())?;
    let mut цель = renderer.bind(&mut текстура).map_err(|е| е.to_string())?;
    let mut трекер = OutputDamageTracker::new(размер, масштаб, Transform::Normal);
    трекер
        .render_output(renderer, &mut цель, 0, элементы, Color32F::new(0.0, 0.0, 0.0, 1.0))
        .map_err(|е| format!("{е:?}"))?;
    let о = з.данные.область;
    // Кадр в текстуре лежит сверху вниз, как на экране: smithay рисует в
    // текстуру без переворота. Его flipped() здесь всегда true — верить ему
    // нельзя: с таким переворотом снимки на настоящем железе выходили вверх
    // ногами (а на стенде это скрывал grim, переворачивая по Flipped180)
    let отображение = renderer
        .copy_framebuffer(&цель, Rectangle::new((о.loc.x, о.loc.y).into(), (о.size.w, о.size.h).into()), Fourcc::Abgr8888)
        .map_err(|е| е.to_string())?;
    let пиксели = renderer.map_texture(&отображение).map_err(|е| е.to_string())?;
    let (ш, в) = (о.size.w as usize, о.size.h as usize);
    with_buffer_contents_mut(&з.буфер, |указатель, длина, д| {
        let шаг = д.stride as usize;
        let начало = д.offset as usize;
        if начало + шаг * в > длина {
            return;
        }
        // Safety: границы проверены выше, буфер принадлежит программе и жив
        let цель = unsafe { std::slice::from_raw_parts_mut(указатель.add(начало), шаг * в) };
        for y in 0..в {
            let из = &пиксели[y * ш * 4..(y + 1) * ш * 4];
            let в_строку = &mut цель[y * шаг..y * шаг + ш * 4];
            // RGBA → BGRA: в памяти wl_shm XRGB8888 лежит как B, G, R, X
            for (п, к) in из.chunks_exact(4).zip(в_строку.chunks_exact_mut(4)) {
                к[0] = п[2];
                к[1] = п[1];
                к[2] = п[0];
                к[3] = 255;
            }
        }
    })
    .map_err(|е| format!("{е:?}"))?;
    Ok(())
}
