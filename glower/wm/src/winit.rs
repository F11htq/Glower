//! Сервер окном внутри другого экрана: X11 или Wayland. Так его проверяют,
//! и так он работает на старых машинах, где есть только X-сервер

use smithay::{
    backend::{
        renderer::{damage::OutputDamageTracker, gles::GlesRenderer, Color32F},
        winit::{self, WinitEvent, WinitGraphicsBackend},
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::calloop::EventLoop,
    utils::Transform,
};

use crate::{
    backend::Бэкенд,
    blur::Размыватель,
    cursor::Указатели,
    render::{self, Для},
    state::Glower,
};

pub struct Winit {
    pub backend: WinitGraphicsBackend<GlesRenderer>,
    pub экран: Output,
    pub трекер: OutputDamageTracker,
    pub указатели: Указатели,
    pub размыватель: Option<Размыватель>,
}

const ЧАСТОТА: i32 = 60_000;

pub fn запусти(цикл: &mut EventLoop<'static, Glower>, состояние: &mut Glower) -> Result<(), Box<dyn std::error::Error>> {
    let атрибуты = smithay::reexports::winit::window::Window::default_attributes()
        .with_title("GlowerOS")
        .with_inner_size(smithay::reexports::winit::dpi::LogicalSize::new(1280.0, 800.0));
    let (mut backend, источник) = winit::init_from_attributes::<GlesRenderer>(атрибуты)?;
    // Свой указатель рисуем сами — указатель хозяина прячем
    backend.window().set_cursor_visible(false);
    let размер = backend.window_size();
    let экран = Output::new(
        "winit".into(),
        PhysicalProperties { size: (0, 0).into(), subpixel: Subpixel::Unknown, make: "GlowerOS".into(), model: "окно".into() },
    );
    let режим = Mode { size: размер, refresh: ЧАСТОТА };
    let _ = экран.create_global::<Glower>(&состояние.display_handle);
    экран.change_current_state(Some(режим), Some(Transform::Flipped180), None, Some((0, 0).into()));
    экран.set_preferred(режим);
    состояние.space.map_output(&экран, (0, 0));

    let размыватель = if std::env::var_os("GLOWER_WM_NO_BLUR").is_some() {
        None
    } else {
        match Размыватель::new(backend.renderer()) {
            Ok(mut р) => {
                р.перевёрнуто = false;
                // Панель увидит и сделает меню прозрачнее
                std::env::set_var("GLOWER_WM_BLUR", "1");
                Some(р)
            }
            Err(е) => {
                tracing::warn!("размытие не работает: {е:?}");
                None
            }
        }
    };
    let трекер = OutputDamageTracker::from_output(&экран);
    состояние.бэкенд = Some(Бэкенд::Winit(Winit { backend, экран, трекер, указатели: Указатели::new(), размыватель }));

    цикл.handle().insert_source(источник, |событие, _, с| match событие {
        WinitEvent::Resized { size, .. } => {
            if let Some(Бэкенд::Winit(w)) = с.бэкенд.as_ref() {
                let э = w.экран.clone();
                э.change_current_state(Some(Mode { size, refresh: ЧАСТОТА }), None, None, None);
                smithay::desktop::layer_map_for_output(&э).arrange();
            }
            с.нужен_кадр = true;
        }
        WinitEvent::Input(событие) => с.ввод(событие),
        WinitEvent::Redraw => кадр(с),
        WinitEvent::CloseRequested => с.loop_signal.stop(),
        _ => {}
    })?;
    Ok(())
}

fn кадр(с: &mut Glower) {
    let Some(Бэкенд::Winit(mut w)) = с.бэкенд.take() else { return };
    let идут = с.анимации.шаг();
    let экран = w.экран.clone();
    let элементы = {
        let Winit { backend, указатели, размыватель, .. } = &mut w;
        render::элементы(с, backend.renderer(), &экран, Для { указатели, размыватель: размыватель.as_mut(), с_указателем: true })
    };
    let возраст = w.backend.buffer_age().unwrap_or(0);
    let итог = match w.backend.bind() {
        Ok((renderer, mut цель)) => w
            .трекер
            .render_output(renderer, &mut цель, возраст, &элементы, Color32F::new(0.05, 0.06, 0.09, 1.0))
            .map(|р| р.damage.cloned())
            .map_err(|е| format!("{е:?}")),
        Err(е) => Err(format!("{е:?}")),
    };
    match итог {
        Ok(повреждено) => {
            if let Err(е) = w.backend.submit(повреждено.as_deref()) {
                tracing::warn!("кадр не показан: {е}");
            }
        }
        Err(е) => tracing::warn!("кадр не нарисован: {е}"),
    }
    if с.screencopy_state.есть_для(&экран) {
        let Winit { backend, указатели, размыватель, .. } = &mut w;
        let без = render::элементы(с, backend.renderer(), &экран, Для { указатели, размыватель: размыватель.as_mut(), с_указателем: false });
        crate::protocols::screencopy::сними(с, backend.renderer(), &экран, &элементы, &без);
    }
    с.кадр_показан(&экран);
    с.бэкенд = Some(Бэкенд::Winit(w));
    // Следующий кадр — только если что-то движется; иначе ждём перемен
    с.нужен_кадр = идут;
}
