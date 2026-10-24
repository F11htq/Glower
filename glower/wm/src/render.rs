//! Из чего состоит кадр экрана, сверху вниз: указатель, замок, меню и
//! панель (с размытием под ними), призраки закрытых окон, окна, обои

use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            element::{
                memory::MemoryRenderBufferRenderElement,
                solid::SolidColorRenderElement,
                surface::{render_elements_from_surface_tree, WaylandSurfaceRenderElement},
                texture::TextureRenderElement,
                utils::{Relocate, RelocateRenderElement, RescaleRenderElement},
                AsRenderElements, Id, Kind,
            },
            gles::{GlesRenderer, GlesTexture},
            utils::draw_render_elements,
            Bind, Color32F, Frame, Offscreen, Renderer,
        },
    },
    desktop::{layer_map_for_output, Window},
    input::pointer::{CursorImageStatus, CursorImageSurfaceData},
    output::Output,
    render_elements,
    utils::{Logical, Physical, Point, Rectangle, Scale, Size, Transform},
    wayland::{compositor::with_states, shell::wlr_layer::Layer},
};

use crate::{
    anim::{плавно, Анимации},
    blur::{Размытие, Размыватель},
    cursor::Указатели,
    state::Glower,
};

render_elements! {
    pub Элемент<=GlesRenderer>;
    Поверхность=WaylandSurfaceRenderElement<GlesRenderer>,
    Окно=RelocateRenderElement<RescaleRenderElement<WaylandSurfaceRenderElement<GlesRenderer>>>,
    Снимок=RelocateRenderElement<RescaleRenderElement<TextureRenderElement<GlesTexture>>>,
    Текстура=TextureRenderElement<GlesTexture>,
    Память=MemoryRenderBufferRenderElement<GlesRenderer>,
    Сплошной=SolidColorRenderElement,
    Размытие=Размытие,
}

impl std::fmt::Debug for Элемент {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Элемент")
    }
}

/// Пространства имён слоёв, под которыми размываем то, что ниже: панель,
/// док, меню, уведомления, просмотр задач
pub fn размывать_под(имя: &str) -> bool {
    имя.starts_with("glower-")
        && !matches!(имя, "glower-desktop" | "glower-lock" | "glower-snip" | "glower-dock-edge" | "glower-setup")
        && !имя.starts_with("glower-setup")
}

pub struct Для<'a> {
    pub указатели: &'a mut Указатели,
    pub размыватель: Option<&'a mut Размыватель>,
    pub с_указателем: bool,
}

pub fn элементы(состояние: &mut Glower, renderer: &mut GlesRenderer, экран: &Output, для: Для<'_>) -> Vec<Элемент> {
    let масштаб = Scale::from(экран.current_scale().fractional_scale());
    let Some(гео) = состояние.space.output_geometry(экран) else { return vec![] };
    let mut верх: Vec<Элемент> = vec![];

    // Указатель
    if для.с_указателем {
        let где = состояние.мышь - гео.loc.to_f64();
        if Rectangle::from_size(гео.size).to_f64().contains(где) {
            верх.extend(указатель(состояние, renderer, где, масштаб, для.указатели));
        }
    }

    // Замок: кроме него ничего
    if let Some(з) = состояние.замок.as_ref() {
        match з.поверхности.iter().find(|(о, _)| о == экран) {
            Some((_, п)) => верх.extend(
                render_elements_from_surface_tree(renderer, п.wl_surface(), (0, 0), масштаб, 1.0, Kind::Unspecified)
                    .into_iter()
                    .map(Элемент::Поверхность),
            ),
            None => {}
        }
        верх.push(Элемент::Сплошной(SolidColorRenderElement::new(
            Id::new(),
            Rectangle::from_size(гео.size.to_physical_precise_round(масштаб)),
            0usize,
            Color32F::new(0.02, 0.03, 0.06, 1.0),
            Kind::Unspecified,
        )));
        return верх;
    }

    let карта = layer_map_for_output(экран);
    let mut низ: Vec<Элемент> = vec![];
    let mut слои_верх: Vec<(Элемент, Option<(smithay::reexports::wayland_server::protocol::wl_surface::WlSurface, Rectangle<i32, Physical>)>)> = vec![];

    // Окно во весь экран сверху — полоса панели уходит под него
    let весь_экран = состояние.во_весь_экран_сверху();
    let mut под_окном: Vec<Элемент> = vec![];
    for слой in [Layer::Overlay, Layer::Top] {
        if весь_экран && слой == Layer::Top {
            for л in карта.layers_on(слой).rev() {
                let Some(где) = карта.layer_geometry(л) else { continue };
                let физ = где.loc.to_physical_precise_round(масштаб);
                let элементы: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = л.render_elements(renderer, физ, масштаб, 1.0);
                под_окном.extend(элементы.into_iter().map(Элемент::Поверхность));
            }
            continue;
        }
        for л in карта.layers_on(слой).rev() {
            let Some(где) = карта.layer_geometry(л) else { continue };
            let физ = где.loc.to_physical_precise_round(масштаб);
            let элементы: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = л.render_elements(renderer, физ, масштаб, 1.0);
            for э in элементы {
                слои_верх.push((Элемент::Поверхность(э), None));
            }
            if размывать_под(л.namespace()) {
                let размер = где.size.to_physical_precise_round(масштаб);
                слои_верх.push((Элемент::Сплошной(пустышка()), Some((л.wl_surface().clone(), Rectangle::new(физ, размер)))));
            }
        }
    }
    for слой in [Layer::Bottom, Layer::Background] {
        for л in карта.layers_on(слой).rev() {
            let Some(где) = карта.layer_geometry(л) else { continue };
            let физ = где.loc.to_physical_precise_round(масштаб);
            let элементы: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = л.render_elements(renderer, физ, масштаб, 1.0);
            низ.extend(элементы.into_iter().map(Элемент::Поверхность));
        }
    }
    drop(карта);

    // Окна сверху вниз, с анимацией
    let mut окна_эл: Vec<Элемент> = vec![];
    for п in состояние.анимации.призраки.iter() {
        окна_эл.extend(призрак(п, renderer, гео, масштаб));
    }
    let окна: Vec<Window> = состояние.space.elements_for_output(экран).rev().cloned().collect();
    for о in &окна {
        let Some(место) = состояние.space.element_location(о) else { continue };
        let вид = состояние.анимации.вид(о, гео.size.w);
        // Место в Space — это угол геометрии окна; рисовать надо от угла
        // поверхности, а у окон GTK вокруг геометрии ещё тень
        let физ = (место - о.geometry().loc - гео.loc).to_physical_precise_round(масштаб);
        let элементы: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = о.render_elements(renderer, физ, масштаб, вид.прозрачность);
        let центр = {
            let г = о.geometry();
            let ц = место - гео.loc + Point::from((г.size.w / 2, г.size.h / 2));
            ц.to_physical_precise_round(масштаб)
        };
        let сдвиг: Point<i32, Physical> = вид.сдвиг.to_physical_precise_round(масштаб);
        окна_эл.extend(элементы.into_iter().map(|э| {
            Элемент::Окно(RelocateRenderElement::from_element(
                RescaleRenderElement::from_element(э, центр, вид.масштаб),
                сдвиг,
                Relocate::Relative,
            ))
        }));
        if let Some(x) = о.x11_surface() {
            окна_эл.extend(crate::xwayland::рамка(x, физ, масштаб, crate::state::активно(о)).into_iter().map(Элемент::Сплошной));
        }
    }

    // Размытие под слоями: из всего, что ниже слоя
    let mut всё: Vec<Элемент> = верх;
    if let Some(р) = для.размыватель {
        let нужно: Vec<(smithay::reexports::wayland_server::protocol::wl_surface::WlSurface, Rectangle<i32, Physical>)> =
            слои_верх.iter().filter_map(|(_, б)| б.clone()).collect();
        if !нужно.is_empty() {
            // Фон для размытия: окна и обои (сами слои панели в него не входят)
            let фон: Vec<&Элемент> = окна_эл.iter().chain(низ.iter()).collect();
            let размер = гео.size.to_physical_precise_round(масштаб);
            if let Err(е) = р.обнови(renderer, размер, масштаб, &фон) {
                tracing::debug!("размытие не вышло: {е:?}");
            }
        }
        for (э, б) in слои_верх {
            match б {
                Some((поверхность, прямоугольник)) => {
                    if let Some(эл) = р.элемент(renderer, &поверхность, прямоугольник) {
                        всё.push(Элемент::Размытие(эл));
                    }
                }
                None => всё.push(э),
            }
        }
    } else {
        всё.extend(слои_верх.into_iter().filter(|(_, б)| б.is_none()).map(|(э, _)| э));
    }
    всё.extend(окна_эл);
    всё.extend(под_окном);
    всё.extend(низ);
    всё
}

/// Пустой элемент-заглушка: его место в списке занимает размытие
fn пустышка() -> SolidColorRenderElement {
    SolidColorRenderElement::new(Id::new(), Rectangle::default(), 0usize, Color32F::TRANSPARENT, Kind::Unspecified)
}

fn указатель(
    состояние: &Glower,
    renderer: &mut GlesRenderer,
    где: Point<f64, Logical>,
    масштаб: Scale<f64>,
    указатели: &mut Указатели,
) -> Vec<Элемент> {
    match &состояние.cursor_status {
        CursorImageStatus::Hidden => vec![],
        CursorImageStatus::Surface(с) => {
            let остриё = with_states(с, |с| {
                с.data_map.get::<CursorImageSurfaceData>().map(|д| д.lock().unwrap().hotspot).unwrap_or_default()
            });
            let место = (где - остриё.to_f64()).to_physical_precise_round(масштаб);
            render_elements_from_surface_tree(renderer, с, место, масштаб, 1.0, Kind::Cursor)
                .into_iter()
                .map(Элемент::Поверхность)
                .collect()
        }
        CursorImageStatus::Named(форма) => {
            let целый = масштаб.x.ceil().max(1.0) as u32;
            let Some((буфер, остриё)) = указатели.форма(форма.name(), целый) else { return vec![] };
            let место = где.to_physical(масштаб) - остриё.to_f64().downscale(целый as f64 / масштаб.x);
            MemoryRenderBufferRenderElement::from_buffer(renderer, место, &буфер, None, None, None, Kind::Cursor)
                .map(|э| vec![Элемент::Память(э)])
                .unwrap_or_default()
        }
    }
}

fn призрак(п: &crate::anim::Призрак, renderer: &mut GlesRenderer, экран: Rectangle<i32, Logical>, масштаб: Scale<f64>) -> Vec<Элемент> {
    let t = плавно(Анимации::доля(п));
    let где = п.где;
    let центр: Point<i32, Logical> = где.loc - экран.loc + Point::from((где.size.w / 2, где.size.h / 2));
    let (м, сдвиг, прозр) = if п.к_доку {
        // К доку: вниз к середине экрана, уменьшаясь
        let цель: Point<i32, Logical> = Point::from((экран.size.w / 2, экран.size.h - 20));
        let путь = цель - центр;
        (1.0 - 0.85 * t, Point::from(((путь.x as f64 * t) as i32, (путь.y as f64 * t) as i32)), (1.0 - t) as f32)
    } else {
        (1.0 - 0.08 * t, Point::default(), (1.0 - t) as f32)
    };
    let центр_ф = центр.to_physical_precise_round(масштаб);
    let сдвиг_ф: Point<i32, Physical> = сдвиг.to_physical_precise_round(масштаб);
    if let Some(о) = п.окно.as_ref() {
        let место = (где.loc - экран.loc - о.geometry().loc).to_physical_precise_round(масштаб);
        let элементы: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = о.render_elements(renderer, место, масштаб, прозр);
        return элементы
            .into_iter()
            .map(|э| Элемент::Окно(RelocateRenderElement::from_element(RescaleRenderElement::from_element(э, центр_ф, м), сдвиг_ф, Relocate::Relative)))
            .collect();
    }
    if let Some((текстура, физ)) = п.снимок.as_ref() {
        let место = физ.loc - экран.loc.to_physical_precise_round(масштаб);
        let э = TextureRenderElement::from_static_texture(
            Id::new(),
            renderer.context_id(),
            место.to_f64(),
            текстура.clone(),
            1,
            Transform::Normal,
            Some(прозр),
            None,
            Some(физ.size.to_logical(1)),
            None,
            Kind::Unspecified,
        );
        return vec![Элемент::Снимок(RelocateRenderElement::from_element(RescaleRenderElement::from_element(э, центр_ф, м), сдвиг_ф, Relocate::Relative))];
    }
    vec![]
}

/// Снимок окна в текстуру — для анимации закрытия: после закрытия самого
/// окна рисовать уже нечего
pub fn снимок_окна(renderer: &mut GlesRenderer, окно: &Window, масштаб: Scale<f64>, где: Point<i32, Logical>) -> Option<(GlesTexture, Rectangle<i32, Physical>)> {
    let рамка = окно.bbox();
    if рамка.is_empty() {
        return None;
    }
    let размер: Size<i32, Physical> = рамка.size.to_physical_precise_round(масштаб);
    let элементы: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
        окно.render_elements(renderer, Point::<i32, Logical>::from((-рамка.loc.x, -рамка.loc.y)).to_physical_precise_round(масштаб), масштаб, 1.0);
    let mut текстура: GlesTexture = renderer.create_buffer(Fourcc::Abgr8888, размер.to_logical(1).to_buffer(1, Transform::Normal)).ok()?;
    {
        let mut цель = renderer.bind(&mut текстура).ok()?;
        let mut кадр = renderer.render(&mut цель, размер, Transform::Normal).ok()?;
        let всё = [Rectangle::from_size(размер)];
        кадр.clear(Color32F::TRANSPARENT, &всё).ok()?;
        draw_render_elements(&mut кадр, масштаб, &элементы, &всё).ok()?;
        let _ = кадр.finish().ok()?;
    }
    Some((текстура, Rectangle::new((где + рамка.loc).to_physical_precise_round(масштаб), размер)))
}

impl Glower {
    /// Окно закрывается — снимаем его последний кадр для анимации
    pub fn сними_на_прощание(&mut self, окно: &Window) {
        if self.анимации.выключены {
            return;
        }
        let Some(где) = self.space.element_location(окно) else { return };
        let где = где - окно.geometry().loc;
        let Some(гео) = self.space.element_geometry(окно) else { return };
        let масштаб = self.главный_экран().map(|э| Scale::from(э.current_scale().fractional_scale())).unwrap_or(Scale::from(1.0));
        let Some(renderer) = self.бэкенд.as_mut().and_then(|б| б.renderer()) else { return };
        if let Some((текстура, физ)) = снимок_окна(renderer, окно, масштаб, где) {
            self.анимации.закрылось(текстура, физ, гео);
            self.нужен_кадр = true;
        }
    }
}
