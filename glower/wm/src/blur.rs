//! Размытие под панелью, доком и меню — как «акрил» в Windows.
//!
//! Всё, что лежит ниже слоёв панели (окна и обои), рисуем в отдельную
//! текстуру, уменьшаем вчетверо и размываем гауссом в два прохода. Под
//! каждым слоем панели рисуем эту размытую картинку — но только там, где сам
//! слой не прозрачен: маской служит его собственная альфа. Меню панели — это
//! прозрачное окно во весь экран с маленьким меню посередине, и прямоугольник
//! размыл бы весь экран; по альфе размывается ровно под меню.

use std::collections::HashMap;

use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            damage::OutputDamageTracker,
            element::{surface::WaylandSurfaceRenderElement, Element, Id, Kind, RenderElement, UnderlyingStorage},
            gles::{ffi, GlesError, GlesFrame, GlesRenderer, GlesTexProgram, GlesTexture, Uniform, UniformName, UniformType},
            utils::{with_renderer_surface_state, CommitCounter},
            Bind, Color32F, Frame, Offscreen, Renderer, Texture,
        },
    },
    reexports::wayland_server::{backend::ObjectId, protocol::wl_surface::WlSurface, Resource},
    utils::{Buffer, Physical, Point, Rectangle, Scale, Size, Transform},
    wayland::compositor::with_states,
};

const ГАУСС: &str = r#"#version 100
//_DEFINES_
#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif
precision mediump float;
#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif
uniform float alpha;
uniform vec2 shift;
varying vec2 v_coords;
#if defined(DEBUG_FLAGS)
uniform float tint;
#endif
void main() {
  vec4 c = texture2D(tex, v_coords) * 0.2270270270;
  c += texture2D(tex, v_coords + shift * 1.3846153846) * 0.3162162162;
  c += texture2D(tex, v_coords - shift * 1.3846153846) * 0.3162162162;
  c += texture2D(tex, v_coords + shift * 3.2307692308) * 0.0702702703;
  c += texture2D(tex, v_coords - shift * 3.2307692308) * 0.0702702703;
  gl_FragColor = c * alpha;
}
"#;

const МАСКА: &str = r#"#version 100
//_DEFINES_
#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif
precision mediump float;
#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif
uniform sampler2D blurred;
uniform vec2 screen;
uniform float flip;
uniform float alpha;
varying vec2 v_coords;
#if defined(DEBUG_FLAGS)
uniform float tint;
#endif
void main() {
#if defined(NO_ALPHA)
  float a = 1.0;
#else
  float a = texture2D(tex, v_coords).a;
#endif
  vec2 p = gl_FragCoord.xy / screen;
  if (flip > 0.5) p.y = 1.0 - p.y;
  vec3 b = texture2D(blurred, p).rgb;
  float k = smoothstep(0.03, 0.25, a);
  gl_FragColor = vec4(b, 1.0) * k * alpha;
}
"#;

pub struct Размыватель {
    гаусс: GlesTexProgram,
    маска: GlesTexProgram,
    полный: Option<GlesTexture>,
    половина: Option<GlesTexture>,
    а: Option<GlesTexture>,
    б: Option<GlesTexture>,
    трекер: Option<OutputDamageTracker>,
    размер: Size<i32, Physical>,
    версия: usize,
    /// Для каждого слоя — что видели в прошлый раз, чтобы знать, менялся ли он
    счётчики: HashMap<ObjectId, (usize, CommitCounter, CommitCounter)>,
    /// Рисовать размытое вверх ногами: зависит от того, как бэкенд кладёт кадр
    pub перевёрнуто: bool,
}

impl std::fmt::Debug for Размыватель {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Размыватель")
    }
}

fn сделай(renderer: &mut GlesRenderer, место: &mut Option<GlesTexture>, размер: Size<i32, Physical>) -> Result<(), GlesError> {
    let нужен = размер.to_logical(1).to_buffer(1, Transform::Normal);
    if место.as_ref().map(|т| т.size() != нужен).unwrap_or(true) {
        let т: GlesTexture = renderer.create_buffer(Fourcc::Abgr8888, нужен)?;
        // Края не повторяем: размытие у края экрана брало бы цвет с другого края
        let id = т.tex_id();
        renderer.with_context(|gl| unsafe {
            gl.BindTexture(ffi::TEXTURE_2D, id);
            gl.TexParameteri(ffi::TEXTURE_2D, ffi::TEXTURE_WRAP_S, ffi::CLAMP_TO_EDGE as i32);
            gl.TexParameteri(ffi::TEXTURE_2D, ffi::TEXTURE_WRAP_T, ffi::CLAMP_TO_EDGE as i32);
            gl.TexParameteri(ffi::TEXTURE_2D, ffi::TEXTURE_MIN_FILTER, ffi::LINEAR as i32);
            gl.TexParameteri(ffi::TEXTURE_2D, ffi::TEXTURE_MAG_FILTER, ffi::LINEAR as i32);
            gl.BindTexture(ffi::TEXTURE_2D, 0);
        })?;
        *место = Some(т);
    }
    Ok(())
}

/// Нарисовать текстуру в другую — целиком, своим шейдером или обычным
fn перерисуй(
    renderer: &mut GlesRenderer,
    откуда: &GlesTexture,
    куда: &mut GlesTexture,
    программа: Option<(&GlesTexProgram, [f32; 2])>,
) -> Result<(), GlesError> {
    let размер_откуда = откуда.size();
    let размер_куда = куда.size();
    let физ: Size<i32, Physical> = (размер_куда.w, размер_куда.h).into();
    let mut цель = renderer.bind(куда)?;
    let mut кадр = renderer.render(&mut цель, физ, Transform::Normal)?;
    let всё = Rectangle::from_size(физ);
    let src: Rectangle<f64, Buffer> = Rectangle::from_size(размер_откуда).to_f64();
    match программа {
        Some((п, сдвиг)) => кадр.render_texture_from_to(
            откуда,
            src,
            всё,
            &[всё],
            &[],
            Transform::Normal,
            1.0,
            Some(п),
            &[Uniform::new("shift", (сдвиг[0], сдвиг[1]))],
        )?,
        None => кадр.render_texture_from_to(откуда, src, всё, &[всё], &[], Transform::Normal, 1.0, None, &[])?,
    }
    let _ = кадр.finish()?;
    Ok(())
}

impl Размыватель {
    pub fn new(renderer: &mut GlesRenderer) -> Result<Self, GlesError> {
        let гаусс = renderer.compile_custom_texture_shader(ГАУСС, &[UniformName::new("shift", UniformType::_2f)])?;
        let маска = renderer.compile_custom_texture_shader(
            МАСКА,
            &[
                UniformName::new("blurred", UniformType::_1i),
                UniformName::new("screen", UniformType::_2f),
                UniformName::new("flip", UniformType::_1f),
            ],
        )?;
        Ok(Размыватель {
            гаусс,
            маска,
            полный: None,
            половина: None,
            а: None,
            б: None,
            трекер: None,
            размер: Size::default(),
            версия: 0,
            счётчики: HashMap::new(),
            перевёрнуто: false,
        })
    }

    /// Перерисовать фон и размыть, если он изменился с прошлого кадра
    pub fn обнови<E: RenderElement<GlesRenderer>>(
        &mut self,
        renderer: &mut GlesRenderer,
        размер: Size<i32, Physical>,
        масштаб: Scale<f64>,
        фон: &[E],
    ) -> Result<(), GlesError> {
        if размер.w < 8 || размер.h < 8 {
            return Ok(());
        }
        let новый = размер != self.размер;
        if новый {
            self.размер = размер;
            self.трекер = Some(OutputDamageTracker::new(размер, масштаб, Transform::Normal));
        }
        сделай(renderer, &mut self.полный, размер)?;
        let пол = Size::from(((размер.w / 2).max(1), (размер.h / 2).max(1)));
        let чет = Size::from(((размер.w / 4).max(1), (размер.h / 4).max(1)));
        сделай(renderer, &mut self.половина, пол)?;
        сделай(renderer, &mut self.а, чет)?;
        сделай(renderer, &mut self.б, чет)?;

        let трекер = self.трекер.as_mut().expect("трекер");
        let mut полный = self.полный.take().expect("текстура");
        let изменилось = {
            let mut цель = renderer.bind(&mut полный)?;
            let возраст = if новый { 0 } else { 1 };
            match трекер.render_output(renderer, &mut цель, возраст, фон, Color32F::new(0.0, 0.0, 0.0, 1.0)) {
                Ok(р) => р.damage.map(|д| !д.is_empty()).unwrap_or(false) || новый,
                Err(_) => true,
            }
        };
        if изменилось {
            let mut половина = self.половина.take().expect("текстура");
            let mut а = self.а.take().expect("текстура");
            let mut б = self.б.take().expect("текстура");
            перерисуй(renderer, &полный, &mut половина, None)?;
            перерисуй(renderer, &половина, &mut а, None)?;
            let (ш, в) = (1.0 / чет.w as f32, 1.0 / чет.h as f32);
            for сила in [1.0f32, 2.2] {
                перерисуй(renderer, &а, &mut б, Some((&self.гаусс, [ш * сила, 0.0])))?;
                перерисуй(renderer, &б, &mut а, Some((&self.гаусс, [0.0, в * сила])))?;
            }
            self.половина = Some(половина);
            self.а = Some(а);
            self.б = Some(б);
            self.версия += 1;
        }
        self.полный = Some(полный);
        Ok(())
    }

    /// Элемент размытия под поверхностью слоя
    pub fn элемент(&mut self, renderer: &mut GlesRenderer, поверхность: &WlSurface, где: Rectangle<i32, Physical>) -> Option<Размытие> {
        let размытое = self.а.clone()?;
        let маска = with_renderer_surface_state(поверхность, |с| с.texture::<GlesTexture>(renderer.context_id()).cloned()).flatten()?;
        let эл = with_states(поверхность, |states| {
            WaylandSurfaceRenderElement::<GlesRenderer>::from_surface(renderer, поверхность, states, где.loc.to_f64(), 1.0, Kind::Unspecified)
        })
        .ok()
        .flatten()?;
        let коммит_слоя = Element::current_commit(&эл);
        let запись = self.счётчики.entry(поверхность.id()).or_insert((usize::MAX, коммит_слоя, CommitCounter::default()));
        if запись.0 != self.версия || запись.1 != коммит_слоя {
            запись.0 = self.версия;
            запись.1 = коммит_слоя;
            запись.2.increment();
        }
        let коммит = запись.2;
        Some(Размытие {
            id: Id::from_wayland_resource(поверхность),
            подпись: Id::new(),
            коммит,
            где,
            src: Element::src(&эл),
            поворот: Element::transform(&эл),
            маска,
            размытое,
            экран: self.размер,
            программа: self.маска.clone(),
            перевёрнуто: self.перевёрнуто,
        })
    }
}

pub struct Размытие {
    #[allow(dead_code)]
    id: Id,
    /// Своё имя, не совпадающее с поверхностью: иначе трекер счёл бы размытие
    /// и сам слой одним элементом
    подпись: Id,
    коммит: CommitCounter,
    где: Rectangle<i32, Physical>,
    src: Rectangle<f64, Buffer>,
    поворот: Transform,
    маска: GlesTexture,
    размытое: GlesTexture,
    экран: Size<i32, Physical>,
    программа: GlesTexProgram,
    перевёрнуто: bool,
}

impl Element for Размытие {
    fn id(&self) -> &Id {
        &self.подпись
    }
    fn current_commit(&self) -> CommitCounter {
        self.коммит
    }
    fn src(&self) -> Rectangle<f64, Buffer> {
        self.src
    }
    fn geometry(&self, _scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.где
    }
    fn transform(&self) -> Transform {
        self.поворот
    }
    fn location(&self, _scale: Scale<f64>) -> Point<i32, Physical> {
        self.где.loc
    }
}

impl RenderElement<GlesRenderer> for Размытие {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        _opaque_regions: &[Rectangle<i32, Physical>],
    ) -> Result<(), GlesError> {
        let id = self.размытое.tex_id();
        frame.with_context(|gl| unsafe {
            gl.ActiveTexture(ffi::TEXTURE1);
            gl.BindTexture(ffi::TEXTURE_2D, id);
            gl.ActiveTexture(ffi::TEXTURE0);
        })?;
        let итог = frame.render_texture_from_to(
            &self.маска,
            src,
            dst,
            damage,
            &[],
            self.поворот,
            1.0,
            Some(&self.программа),
            &[
                Uniform::new("blurred", 1),
                Uniform::new("screen", (self.экран.w as f32, self.экран.h as f32)),
                Uniform::new("flip", if self.перевёрнуто { 1.0f32 } else { 0.0 }),
            ],
        );
        frame.with_context(|gl| unsafe {
            gl.ActiveTexture(ffi::TEXTURE1);
            gl.BindTexture(ffi::TEXTURE_2D, 0);
            gl.ActiveTexture(ffi::TEXTURE0);
        })?;
        итог
    }

    fn underlying_storage(&self, _renderer: &mut GlesRenderer) -> Option<UnderlyingStorage<'_>> {
        None
    }
}
