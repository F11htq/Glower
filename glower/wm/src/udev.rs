//! Рисование прямо на видеокарте: DRM/KMS, ввод через libinput, доступ к
//! устройствам — через libseat (seatd или logind), без прав root.
//!
//! Одна видеокарта — основная (с ней загружен экран); на каждый подключённый
//! экран свой CRTC и свой DrmCompositor. Экраны стоят слева направо.

use std::{path::PathBuf, time::Duration};

use smithay::{
    backend::{
        allocator::{
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
            Fourcc,
        },
        drm::{
            compositor::{DrmCompositor, FrameFlags},
            exporter::gbm::GbmFramebufferExporter,
            DrmDevice, DrmDeviceFd, DrmEvent, DrmNode, NodeType,
        },
        egl::{EGLContext, EGLDisplay},
        input::InputEvent,
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{gles::GlesRenderer, Color32F, ImportDma},
        session::{libseat::LibSeatSession, Event as SessionEvent, Session},
        udev::{all_gpus, primary_gpu, UdevBackend, UdevEvent},
    },
    delegate_dmabuf,
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::{
            timer::{TimeoutAction, Timer},
            EventLoop,
        },
        drm::control::{connector, crtc, Device as ControlDevice, ModeTypeFlags},
        input::Libinput,
        rustix::fs::OFlags,
    },
    utils::{DeviceFd, Transform},
    wayland::dmabuf::{DmabufFeedbackBuilder, DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier},
};

use crate::{
    backend::Бэкенд,
    blur::Размыватель,
    cursor::Указатели,
    render::{self, Для},
    state::Glower,
};

type Компоновщик = DrmCompositor<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, (), DrmDeviceFd>;

pub struct Экран {
    pub вывод: Output,
    pub crtc: crtc::Handle,
    pub разъём: connector::Handle,
    /// Имя разъёма («HDMI-A-1») и все режимы, которые назвал экран
    имя: String,
    режимы: Vec<smithay::reexports::drm::control::Mode>,
    компоновщик: Компоновщик,
    /// Кадр ушёл на экран, ждём, когда его покажут (vblank)
    ждёт: bool,
    гамма: u32,
}

pub struct Udev {
    сессия: LibSeatSession,
    drm: DrmDevice,
    gbm: GbmDevice<DrmDeviceFd>,
    renderer: GlesRenderer,
    экраны: Vec<Экран>,
    указатели: Указатели,
    размыватель: Option<Размыватель>,
    libinput: Libinput,
    /// Подключённые мыши и тачпады — чтобы применить к ним новые настройки
    устройства: Vec<smithay::reexports::input::Device>,
    pub dmabuf_state: DmabufState,
    _dmabuf_global: DmabufGlobal,
}

impl Udev {
    pub fn примени_мышь(&mut self, м: &crate::mouse::Мышь) {
        for у in self.устройства.iter_mut() {
            crate::mouse::примени(у, м);
        }
    }

    pub fn смени_указатели(&mut self, тема: &str, размер: u32) {
        self.указатели = Указатели::из(тема, размер);
    }

    pub fn renderer(&mut self) -> Option<&mut GlesRenderer> {
        Some(&mut self.renderer)
    }

    pub fn размер_гаммы(&self, экран: &Output) -> Option<u32> {
        self.экраны.iter().find(|э| &э.вывод == экран).map(|э| э.гамма).filter(|г| *г > 0)
    }

    pub fn задай_гамму(&mut self, экран: &Output, таблица: Option<&[u16]>) -> bool {
        let Some(э) = self.экраны.iter().find(|э| &э.вывод == экран) else { return false };
        let размер = э.гамма as usize;
        if размер == 0 {
            return false;
        }
        let (к, з, с): (Vec<u16>, Vec<u16>, Vec<u16>) = match таблица {
            Some(т) if т.len() == размер * 3 => (т[..размер].to_vec(), т[размер..2 * размер].to_vec(), т[2 * размер..].to_vec()),
            Some(_) => return false,
            None => {
                // Обычные цвета: прямая линия от чёрного до белого
                let прямая: Vec<u16> = (0..размер).map(|i| ((i * 0xffff) / (размер - 1).max(1)) as u16).collect();
                (прямая.clone(), прямая.clone(), прямая)
            }
        };
        self.drm.device_fd().set_gamma(э.crtc, &к, &з, &с).is_ok()
    }

    pub fn смени_консоль(&mut self, номер: i32) {
        if let Err(е) = self.сессия.change_vt(номер) {
            tracing::warn!("консоль {номер}: {е}");
        }
    }
}

fn открой_видеокарту(сессия: &mut LibSeatSession) -> Result<(PathBuf, DrmDeviceFd), Box<dyn std::error::Error>> {
    let место = сессия.seat();
    let пути: Vec<PathBuf> = primary_gpu(&место)?.into_iter().chain(all_gpus(&место)?).collect();
    let mut ошибка = String::from("видеокарт не найдено");
    for путь in пути {
        match сессия.open(&путь, OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK) {
            Ok(fd) => return Ok((путь, DrmDeviceFd::new(DeviceFd::from(fd)))),
            Err(е) => ошибка = format!("{}: {е}", путь.display()),
        }
    }
    Err(ошибка.into())
}

type Найденный = (connector::Handle, crtc::Handle, smithay::reexports::drm::control::Mode, connector::Info);

/// Подключённые экраны: разъём, CRTC и режим.
///
/// Монитор гаснет и заново ловит сигнал всякий раз, когда меняется режим
/// или канал (CRTC), через который он показывает. Поэтому, если экран уже
/// включён — заставкой загрузки или экраном входа, — берём тот же канал и
/// тот же режим: тогда переход от загрузки к экрану входа и от него к
/// рабочему столу проходит без мигания. Режим, выбранный человеком, — выше
/// (подними_экраны), а чтобы и загрузка шла в нём, система передаёт его
/// ядру (agent/screens.mjs)
fn найди_экраны(drm: &DrmDevice) -> Vec<Найденный> {
    let Ok(ресурсы) = drm.resource_handles() else { return vec![] };
    let mut занято: Vec<crtc::Handle> = vec![];
    let mut итог = vec![];
    for &разъём in ресурсы.connectors() {
        let Ok(инфо) = drm.get_connector(разъём, true) else { continue };
        if инфо.state() != connector::State::Connected || инфо.modes().is_empty() {
            continue;
        }
        // Чем экран показывает сейчас: канал и режим
        let нынешний = инфо
            .current_encoder()
            .and_then(|к| drm.get_encoder(к).ok())
            .and_then(|к| к.crtc())
            .filter(|c| !занято.contains(c));
        let нынешний_режим = нынешний
            .and_then(|c| drm.get_crtc(c).ok())
            .and_then(|и| и.mode())
            .filter(|м| инфо.modes().contains(м));
        let режим = нынешний_режим.unwrap_or_else(|| {
            инфо.modes()
                .iter()
                .find(|р| р.mode_type().contains(ModeTypeFlags::PREFERRED))
                .copied()
                .unwrap_or(инфо.modes()[0])
        });
        let crtc = нынешний.or_else(|| {
            инфо.encoders()
                .iter()
                .filter_map(|&к| drm.get_encoder(к).ok())
                .flat_map(|к| ресурсы.filter_crtcs(к.possible_crtcs()))
                .find(|c| !занято.contains(c))
        });
        if let Some(c) = crtc {
            занято.push(c);
            итог.push((разъём, c, режим, инфо));
        }
    }
    итог
}

fn подними_экраны(с: &mut Glower, u: &mut Udev) {
    let найдено = найди_экраны(&u.drm);
    // Отключённые — убрать
    let живые: Vec<connector::Handle> = найдено.iter().map(|(р, ..)| *р).collect();
    let ушли: Vec<Output> = u.экраны.iter().filter(|э| !живые.contains(&э.разъём)).map(|э| э.вывод.clone()).collect();
    for вывод in ушли {
        с.space.unmap_output(&вывод);
        u.экраны.retain(|э| э.вывод != вывод);
    }
    let mut x = с.space.outputs().filter_map(|о| с.space.output_geometry(о)).map(|г| г.loc.x + г.size.w).max().unwrap_or(0);
    for (разъём, crtc, режим, инфо) in найдено {
        if u.экраны.iter().any(|э| э.разъём == разъём) {
            continue;
        }
        let имя = format!("{}-{}", инфо.interface().as_str(), инфо.interface_id());
        // Режим и масштаб, выбранные человеком в Параметрах, — если экран их ещё умеет
        let сохранено = выбор_для(&имя);
        let режим = сохранено
            .as_ref()
            .and_then(|з| з.режим)
            .and_then(|(w, h, mhz)| инфо.modes().iter().copied().find(|м| совпадает(м, w, h, mhz)))
            .unwrap_or(режим);
        let (ш, в) = инфо.size().unwrap_or((0, 0));
        let вывод = Output::new(
            имя.clone(),
            PhysicalProperties { size: (ш as i32, в as i32).into(), subpixel: Subpixel::Unknown, make: "GlowerOS".into(), model: имя.clone() },
        );
        let режим_вывода = Mode::from(режим);
        let масштаб = сохранено.as_ref().and_then(|з| з.масштаб).unwrap_or_else(|| подбери_масштаб(режим_вывода.size.w, ш));
        вывод.change_current_state(
            Some(режим_вывода),
            Some(Transform::Normal),
            Some(smithay::output::Scale::Fractional(масштаб)),
            Some((x, 0).into()),
        );
        вывод.set_preferred(режим_вывода);
        let _ = вывод.create_global::<Glower>(&с.display_handle);
        let поверхность = match u.drm.create_surface(crtc, режим, &[разъём]) {
            Ok(п) => п,
            Err(е) => {
                tracing::warn!("{имя}: не вышло занять экран: {е}");
                continue;
            }
        };
        let распределитель = GbmAllocator::new(u.gbm.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
        let экспорт = GbmFramebufferExporter::new(u.gbm.clone());
        let форматы = u.renderer.egl_context().dmabuf_render_formats().clone();
        let компоновщик = match DrmCompositor::new(
            &вывод,
            поверхность,
            None,
            распределитель,
            экспорт,
            [Fourcc::Argb8888, Fourcc::Xrgb8888],
            форматы,
            u.drm.cursor_size(),
            Some(u.gbm.clone()),
        ) {
            Ok(к) => к,
            Err(е) => {
                tracing::warn!("{имя}: {е:?}");
                continue;
            }
        };
        let гамма = u.drm.get_crtc(crtc).map(|и| и.gamma_length()).unwrap_or(0);
        let логическая = (режим_вывода.size.w as f64 / масштаб).round() as i32;
        с.space.map_output(&вывод, (x, 0));
        x += логическая;
        tracing::info!("экран {имя}: {}x{} @{}, масштаб {масштаб}", режим_вывода.size.w, режим_вывода.size.h, режим_вывода.refresh);
        u.экраны.push(Экран { вывод, crtc, разъём, имя: имя.clone(), режимы: инфо.modes().to_vec(), компоновщик, ждёт: false, гамма });
    }
    for э in &u.экраны {
        smithay::desktop::layer_map_for_output(&э.вывод).arrange();
    }
    с.нужен_кадр = true;
}

/// Выбор человека: экран, режим (ширина, высота, частота в мГц) и масштаб.
/// Хранится строками «HDMI-A-1 1920x1080@75000 1» в ~/.config/glower/screens
pub struct Сохранённый {
    pub имя: String,
    pub режим: Option<(u16, u16, i32)>,
    pub масштаб: Option<f64>,
}

fn файл_экранов() -> Option<PathBuf> {
    let папка = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|д| PathBuf::from(д).join(".config")))?;
    Some(папка.join("glower").join("screens"))
}

pub fn разбери_режим(текст: &str) -> Option<(u16, u16, i32)> {
    let (размер, частота) = текст.split_once('@')?;
    let (w, h) = размер.split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?, частота.parse().ok()?))
}

pub fn разбери_экраны(текст: &str) -> Vec<Сохранённый> {
    текст
        .lines()
        .filter_map(|с| {
            let mut ч = с.split_whitespace();
            let имя = ч.next()?.to_string();
            let режим = ч.next().and_then(разбери_режим);
            let масштаб = ч.next().and_then(|м| м.parse::<f64>().ok()).filter(|м| (0.5..=4.0).contains(м));
            Some(Сохранённый { имя, режим, масштаб })
        })
        .collect()
}

fn сохранённые_экраны() -> Vec<Сохранённый> {
    файл_экранов().and_then(|ф| std::fs::read_to_string(ф).ok()).map(|т| разбери_экраны(&т)).unwrap_or_default()
}

/// Выбор человека, которым система делится со всеми: с экраном входа (у
/// него нет доступа к домашней папке) и с загрузкой. Пишет его агент
/// (agent/screens.mjs) из того же ~/.config/glower/screens
fn общий_файл_экранов() -> PathBuf {
    std::env::var_os("GLOWER_SCREENS_SYS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/etc/glower/screens"))
}

/// Свой выбор — главнее; нет своего — общий
fn выбор_для(имя: &str) -> Option<Сохранённый> {
    сохранённые_экраны().into_iter().find(|з| з.имя == имя).or_else(|| {
        std::fs::read_to_string(общий_файл_экранов()).ok().and_then(|т| разбери_экраны(&т).into_iter().find(|з| з.имя == имя))
    })
}

fn частота_мгц(м: &smithay::reexports::drm::control::Mode) -> i32 {
    Mode::from(*м).refresh
}

/// Тот же режим: размер точно, частота — с точностью до полугерца
fn совпадает(м: &smithay::reexports::drm::control::Mode, w: u16, h: u16, mhz: i32) -> bool {
    let (мш, мв) = м.size();
    мш == w && мв == h && (частота_мгц(м) - mhz).abs() <= 500
}

impl Udev {
    /// Экраны словами для Параметров: имя, режим сейчас, масштаб и все режимы
    /// через запятую; строка на экран, поля через табуляцию
    pub fn экраны_словами(&self) -> String {
        self.экраны
            .iter()
            .map(|э| {
                let сейчас = э.вывод.current_mode().map(|м| format!("{}x{}@{}", м.size.w, м.size.h, м.refresh)).unwrap_or_default();
                let масштаб = э.вывод.current_scale().fractional_scale();
                let mut все: Vec<String> = Vec::new();
                for м in &э.режимы {
                    let (w, h) = м.size();
                    let т = format!("{w}x{h}@{}", частота_мгц(м));
                    if !все.contains(&т) {
                        все.push(т);
                    }
                }
                let модель = э.вывод.physical_properties().model;
                format!("{}\t{}\t{}\t{}\t{}", э.имя, модель, сейчас, масштаб, все.join(","))
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Сменить режим экрана (размер и частоту) и запомнить выбор
    pub fn смени_режим(&mut self, с: &mut Glower, имя: &str, w: u16, h: u16, mhz: i32) -> Result<(), String> {
        let э = self.экраны.iter_mut().find(|э| э.имя == имя).ok_or("нет такого экрана")?;
        let режим = э.режимы.iter().copied().find(|м| совпадает(м, w, h, mhz)).ok_or("экран такого режима не умеет")?;
        э.компоновщик.use_mode(режим).map_err(|е| format!("{е:?}"))?;
        let новый = Mode::from(режим);
        э.вывод.change_current_state(Some(новый), None, None, None);
        э.вывод.set_preferred(новый);
        разложи(с, self);
        запомни(self);
        Ok(())
    }

    /// Сменить масштаб экрана и запомнить выбор
    pub fn смени_масштаб(&mut self, с: &mut Glower, имя: &str, масштаб: f64) -> Result<(), String> {
        if !(0.5..=4.0).contains(&масштаб) {
            return Err("такой масштаб не годится".into());
        }
        let э = self.экраны.iter_mut().find(|э| э.имя == имя).ok_or("нет такого экрана")?;
        э.вывод.change_current_state(None, None, Some(smithay::output::Scale::Fractional(масштаб)), None);
        разложи(с, self);
        запомни(self);
        Ok(())
    }
}

/// Экраны в ряд слева направо — после смены режима или масштаба ширина меняется
fn разложи(с: &mut Glower, u: &Udev) {
    let mut x = 0;
    for э in &u.экраны {
        с.space.map_output(&э.вывод, (x, 0));
        let ширина = э.вывод.current_mode().map(|м| м.size.w).unwrap_or(0);
        x += (ширина as f64 / э.вывод.current_scale().fractional_scale()).round() as i32;
        smithay::desktop::layer_map_for_output(&э.вывод).arrange();
    }
    с.нужен_кадр = true;
}

fn запомни(u: &Udev) {
    let Some(файл) = файл_экранов() else { return };
    let mut строки: Vec<Сохранённый> = сохранённые_экраны();
    for э in &u.экраны {
        let режим = э.вывод.current_mode().map(|м| (м.size.w as u16, м.size.h as u16, м.refresh));
        let масштаб = Some(э.вывод.current_scale().fractional_scale());
        строки.retain(|з| з.имя != э.имя);
        строки.push(Сохранённый { имя: э.имя.clone(), режим, масштаб });
    }
    let текст: String = строки
        .iter()
        .map(|з| {
            let р = з.режим.map(|(w, h, f)| format!("{w}x{h}@{f}")).unwrap_or_else(|| "-".into());
            format!("{} {} {}\n", з.имя, р, з.масштаб.unwrap_or(1.0))
        })
        .collect();
    if let Some(папка) = файл.parent() {
        let _ = std::fs::create_dir_all(папка);
    }
    let _ = std::fs::write(файл, текст);
}

/// Мелкий текст на маленьком экране с высоким разрешением не прочесть:
/// больше ~170 точек на дюйм — масштаб 1.5, больше ~220 — 2
fn подбери_масштаб(точек: i32, мм: u32) -> f64 {
    if let Some(м) = std::env::var("GLOWER_WM_SCALE").ok().and_then(|м| м.parse::<f64>().ok()).filter(|м| (0.5..=4.0).contains(м)) {
        return м;
    }
    if мм < 100 {
        return 1.0;
    }
    let на_дюйм = точек as f64 / (мм as f64 / 25.4);
    if на_дюйм > 220.0 {
        2.0
    } else if на_дюйм > 170.0 {
        1.5
    } else {
        1.0
    }
}

pub fn запусти(цикл: &mut EventLoop<'static, Glower>, с: &mut Glower) -> Result<(), Box<dyn std::error::Error>> {
    let (mut сессия, вести_сессии) = LibSeatSession::new().map_err(|е| format!("libseat: {е} (нужен seatd или logind)"))?;
    let (путь, fd) = открой_видеокарту(&mut сессия)?;
    tracing::info!("видеокарта: {}", путь.display());
    let (drm, вести_drm) = DrmDevice::new(fd.clone(), true)?;
    let gbm = GbmDevice::new(fd.clone())?;
    let дисплей = unsafe { EGLDisplay::new(gbm.clone())? };
    let контекст = EGLContext::new(&дисплей)?;
    let mut renderer = unsafe { GlesRenderer::new(контекст)? };
    // Программы с аппаратным ускорением отдают кадры прямо в видеопамяти.
    //
    // Вместе с форматами говорим, на какой видеокарте рисовать (dmabuf v4,
    // «основное устройство»). Без этого Mesa и Xwayland не находят карту и
    // молча рисуют процессором: у человека Minecraft шёл 8 кадров в секунду
    // вместо двухсот в Windows, и так же медленно — всё с OpenGL и Vulkan.
    let mut dmabuf_state = DmabufState::new();
    let форматы_dmabuf: Vec<_> = renderer.dmabuf_formats().into_iter().collect();
    let узел = DrmNode::from_file(fd.device_fd())
        .ok()
        .map(|у| у.node_with_type(NodeType::Render).and_then(Result::ok).unwrap_or(у));
    let отзыв = узел.and_then(|у| DmabufFeedbackBuilder::new(у.dev_id(), форматы_dmabuf.clone()).build().ok());
    let dmabuf_global = match &отзыв {
        Some(о) => {
            tracing::info!("видеокарта для программ: {:?}", узел.and_then(|у| у.dev_path()));
            dmabuf_state.create_global_with_default_feedback::<Glower>(&с.display_handle, о)
        }
        None => {
            tracing::warn!("не узнать узел видеокарты — программы могут рисовать процессором");
            dmabuf_state.create_global::<Glower>(&с.display_handle, форматы_dmabuf)
        }
    };

    let размыватель = if std::env::var_os("GLOWER_WM_NO_BLUR").is_some() {
        None
    } else {
        match Размыватель::new(&mut renderer) {
            Ok(р) => {
                std::env::set_var("GLOWER_WM_BLUR", "1");
                Some(р)
            }
            Err(е) => {
                tracing::warn!("размытие не работает: {е:?}");
                None
            }
        }
    };

    let mut libinput = Libinput::new_with_udev(LibinputSessionInterface::from(сессия.clone()));
    libinput.udev_assign_seat(&сессия.seat()).map_err(|_| "libinput: не назначить место")?;
    let ввод = LibinputInputBackend::new(libinput.clone());

    let mut u = Udev {
        сессия: сессия.clone(),
        drm,
        gbm,
        renderer,
        экраны: vec![],
        указатели: Указатели::new(),
        размыватель,
        libinput,
        устройства: Vec::new(),
        dmabuf_state,
        _dmabuf_global: dmabuf_global,
    };
    подними_экраны(с, &mut u);
    if u.экраны.is_empty() {
        return Err("ни одного подключённого экрана".into());
    }
    // Мышь — посередине первого экрана
    if let Some(г) = u.экраны.first().and_then(|э| с.space.output_geometry(&э.вывод)) {
        с.мышь = (г.loc + г.size.to_point().downscale(2)).to_f64();
    }
    с.бэкенд = Some(Бэкенд::Udev(u));

    let ручка = цикл.handle();
    ручка.insert_source(ввод, |событие, _, с| {
        match &событие {
            // Новая мышь или тачпад — сразу по настройкам «Параметров». По
            // умолчанию у тачпада касание — щелчок, как в Windows, и пока
            // печатаешь, он не дёргает указатель
            InputEvent::DeviceAdded { device } => {
                let mut устройство = device.clone();
                crate::mouse::примени(&mut устройство, &с.настройки_мыши);
                if let Some(Бэкенд::Udev(u)) = с.бэкенд.as_mut() {
                    u.устройства.push(устройство);
                }
            }
            InputEvent::DeviceRemoved { device } => {
                if let Some(Бэкенд::Udev(u)) = с.бэкенд.as_mut() {
                    u.устройства.retain(|у| у != device);
                }
            }
            _ => {}
        }
        с.ввод(событие);
    })?;

    ручка.insert_source(вести_drm, |событие, _, с| match событие {
        DrmEvent::VBlank(crtc) => {
            let mut показан = None;
            if let Some(Бэкенд::Udev(u)) = с.бэкенд.as_mut() {
                if let Some(э) = u.экраны.iter_mut().find(|э| э.crtc == crtc) {
                    let _ = э.компоновщик.frame_submitted();
                    э.ждёт = false;
                    показан = Some(э.вывод.clone());
                }
            }
            if let Some(в) = показан {
                с.кадр_показан(&в);
            }
            нарисуй(с);
        }
        DrmEvent::Error(е) => tracing::warn!("DRM: {е}"),
    })?;

    ручка.insert_source(вести_сессии, |событие, _, с| match событие {
        SessionEvent::PauseSession => {
            if let Some(Бэкенд::Udev(u)) = с.бэкенд.as_mut() {
                u.libinput.suspend();
                u.drm.pause();
            }
        }
        SessionEvent::ActivateSession => {
            if let Some(Бэкенд::Udev(u)) = с.бэкенд.as_mut() {
                if u.libinput.resume().is_err() {
                    tracing::warn!("libinput не вернулся после смены консоли");
                }
                let _ = u.drm.activate(false);
                for э in u.экраны.iter_mut() {
                    let _ = э.компоновщик.reset_state();
                    э.ждёт = false;
                }
            }
            с.нужен_кадр = true;
            нарисуй(с);
        }
    })?;

    // Экраны подключают и отключают на ходу: следим за видеокартой
    if let Ok(udev) = UdevBackend::new(сессия.seat()) {
        ручка.insert_source(udev, |событие, _, с| {
            if let UdevEvent::Changed { .. } = событие {
                let Some(Бэкенд::Udev(mut u)) = с.бэкенд.take() else { return };
                подними_экраны(с, &mut u);
                с.бэкенд = Some(Бэкенд::Udev(u));
                нарисуй(с);
            }
        })?;
    }

    // Страховка: раз в полсекунды проверить, не ждёт ли кто-то кадра
    ручка.insert_source(Timer::from_duration(Duration::from_millis(500)), |_, _, с| {
        нарисуй(с);
        TimeoutAction::ToDuration(Duration::from_millis(500))
    })?;
    Ok(())
}

/// Нарисовать кадры на экранах, которые его не ждут, если что-то изменилось
pub fn нарисуй(с: &mut Glower) {
    if !с.нужен_кадр && с.screencopy_state.ждут.is_empty() {
        return;
    }
    let Some(Бэкенд::Udev(mut u)) = с.бэкенд.take() else { return };
    if !u.drm.is_active() {
        с.бэкенд = Some(Бэкенд::Udev(u));
        return;
    }
    let идут = с.анимации.шаг();
    let mut без_изменений = vec![];
    for i in 0..u.экраны.len() {
        if u.экраны[i].ждёт {
            continue;
        }
        let вывод = u.экраны[i].вывод.clone();
        let элементы = {
            let Udev { renderer, указатели, размыватель, .. } = &mut u;
            render::элементы(с, renderer, &вывод, Для { указатели, размыватель: размыватель.as_mut(), с_указателем: true })
        };
        {
            let Udev { renderer, экраны, .. } = &mut u;
            let э = &mut экраны[i];
            match э.компоновщик.render_frame(renderer, &элементы, Color32F::new(0.05, 0.06, 0.09, 1.0), FrameFlags::DEFAULT) {
                Ok(итог) => {
                    if итог.is_empty {
                        без_изменений.push(вывод.clone());
                    } else {
                        match э.компоновщик.queue_frame(()) {
                            Ok(()) => э.ждёт = true,
                            Err(е) => tracing::warn!("кадр не ушёл на экран: {е:?}"),
                        }
                    }
                }
                Err(е) => tracing::warn!("кадр не нарисован: {е:?}"),
            }
        }
        if с.screencopy_state.есть_для(&вывод) {
            let без = {
                let Udev { renderer, указатели, .. } = &mut u;
                render::элементы(с, renderer, &вывод, Для { указатели, размыватель: None, с_указателем: false })
            };
            crate::protocols::screencopy::сними(с, &mut u.renderer, &вывод, &элементы, &без);
        }
    }
    с.бэкенд = Some(Бэкенд::Udev(u));
    // Ничего не поменялось — программам всё равно сказать «кадр был»,
    // иначе те, что ждут его, остановятся
    for в in без_изменений {
        с.кадр_показан(&в);
    }
    с.нужен_кадр = идут;
}

impl DmabufHandler for Glower {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        match self.бэкенд.as_mut() {
            Some(Бэкенд::Udev(u)) => &mut u.dmabuf_state,
            _ => unreachable!("dmabuf есть только на видеокарте"),
        }
    }

    fn dmabuf_imported(&mut self, _global: &DmabufGlobal, dmabuf: smithay::backend::allocator::dmabuf::Dmabuf, notifier: ImportNotifier) {
        let годится = self.бэкенд.as_mut().and_then(|б| б.renderer()).map(|r| r.import_dmabuf(&dmabuf, None).is_ok()).unwrap_or(false);
        if годится {
            let _ = notifier.successful::<Glower>();
        } else {
            notifier.failed();
        }
    }
}

delegate_dmabuf!(Glower);

#[cfg(test)]
mod проверки {
    #[test]
    fn выбор_экранов_читается() {
        let с = super::разбери_экраны("HDMI-A-1 1920x1080@75000 1\nDP-1 - 1.5\nмусор\nDP-2 2560x1440@144001 9\n");
        assert_eq!(с.len(), 4);
        assert_eq!(с[0].режим, Some((1920, 1080, 75000)));
        assert_eq!(с[0].масштаб, Some(1.0));
        assert_eq!(с[1].режим, None);
        assert_eq!(с[1].масштаб, Some(1.5));
        assert_eq!(с[3].масштаб, None);
        assert_eq!(super::разбери_режим("1920x1080@60000"), Some((1920, 1080, 60000)));
        assert_eq!(super::разбери_режим("1920x1080"), None);
    }
}
