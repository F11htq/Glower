//! Мышь и тачпад: скорость, точность, левша, прокрутка, касание.
//!
//! Настройки — из раздела <libinput> того же rc.xml, что читает labwc, и в
//! его же виде: одно место на оба сервера. Пишет раздел агент, когда человек
//! меняет что-то в «Параметрах» → «Мышь», и шлёт SIGHUP — тогда настройки
//! перечитываются и применяются ко всем подключённым мышам и тачпадам
//! сразу, а к новым — при подключении.
//!
//!   <libinput>
//!     <device category="non-touch"> — мыши
//!     <device category="touchpad">  — тачпады
//!       <pointerSpeed>-1…1</pointerSpeed>  <accelProfile>flat|adaptive</accelProfile>
//!       <leftHanded>yes|no</leftHanded>    <naturalScroll>yes|no</naturalScroll>
//!       <scrollFactor>0.3…3</scrollFactor> <tap>yes|no</tap>
//!       <disableWhileTyping>yes|no</disableWhileTyping> <clickMethod>clickfinger|buttonAreas</clickMethod>

use smithay::reexports::input::{AccelProfile, ClickMethod, Device};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Устройство {
    pub скорость: Option<f64>,
    pub ровно: Option<bool>,
    pub левша: Option<bool>,
    pub обратная: Option<bool>,
    pub множитель: Option<f64>,
    pub касание: Option<bool>,
    pub без_набора: Option<bool>,
    pub щелчок_пальцами: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Мышь {
    pub мышь: Устройство,
    pub тачпад: Устройство,
}

fn тег<'a>(где: &'a str, имя: &str) -> Option<&'a str> {
    let н = где.find(&format!("<{имя}>"))? + имя.len() + 2;
    let к = где[н..].find(&format!("</{имя}>"))? + н;
    Some(где[н..к].trim())
}

fn да(т: &str) -> Option<bool> {
    match т.to_ascii_lowercase().as_str() {
        "yes" | "true" | "1" => Some(true),
        "no" | "false" | "0" => Some(false),
        _ => None,
    }
}

fn устройство(блок: &str) -> Устройство {
    Устройство {
        скорость: тег(блок, "pointerSpeed").and_then(|т| т.parse().ok()).map(|v: f64| v.clamp(-1.0, 1.0)),
        ровно: тег(блок, "accelProfile").map(|т| т.eq_ignore_ascii_case("flat")),
        левша: тег(блок, "leftHanded").and_then(да),
        обратная: тег(блок, "naturalScroll").and_then(да),
        множитель: тег(блок, "scrollFactor").and_then(|т| т.parse().ok()).map(|v: f64| v.clamp(0.1, 10.0)),
        касание: тег(блок, "tap").and_then(да),
        без_набора: тег(блок, "disableWhileTyping").and_then(да),
        щелчок_пальцами: тег(блок, "clickMethod").map(|т| т.eq_ignore_ascii_case("clickfinger")),
    }
}

pub fn разбери(rc: &str) -> Мышь {
    let mut м = Мышь::default();
    let Some(раздел) = тег(rc, "libinput") else { return м };
    for кусок in раздел.split("<device").skip(1) {
        let конец = кусок.find("</device>").unwrap_or(кусок.len());
        let блок = &кусок[..конец];
        let вид = блок.split('"').nth(1).unwrap_or("default");
        let у = устройство(блок);
        match вид {
            "touchpad" => м.тачпад = у,
            "non-touch" => м.мышь = у,
            // «для всех» — и мышам, и тачпадам, если для них не сказано своё
            _ => {
                if м.мышь == Устройство::default() {
                    м.мышь = у.clone();
                }
                if м.тачпад == Устройство::default() {
                    м.тачпад = у;
                }
            }
        }
    }
    м
}

/// Тачпад ли это: только у тачпада есть касание вместо щелчка
pub fn тачпад(у: &Device) -> bool {
    у.config_tap_finger_count() > 0
}

pub fn примени(у: &mut Device, м: &Мышь) {
    let тач = тачпад(у);
    let н = if тач { &м.тачпад } else { &м.мышь };
    if у.config_accel_is_available() {
        let _ = у.config_accel_set_speed(н.скорость.unwrap_or(0.0));
        let _ = у.config_accel_set_profile(if н.ровно == Some(true) { AccelProfile::Flat } else { AccelProfile::Adaptive });
    }
    if у.config_left_handed_is_available() {
        let _ = у.config_left_handed_set(н.левша.unwrap_or(false));
    }
    if у.config_scroll_has_natural_scroll() {
        // У тачпада, как в Windows, по умолчанию «движение вниз — вниз»
        let _ = у.config_scroll_set_natural_scroll_enabled(н.обратная.unwrap_or(тач));
    }
    if тач {
        let _ = у.config_tap_set_enabled(н.касание.unwrap_or(true));
        let _ = у.config_dwt_set_enabled(н.без_набора.unwrap_or(true));
        let методы = у.config_click_methods();
        let нужен = if н.щелчок_пальцами == Some(true) { ClickMethod::Clickfinger } else { ClickMethod::ButtonAreas };
        if методы.contains(&нужен) {
            let _ = у.config_click_set_method(нужен);
        }
    }
}

#[cfg(test)]
mod проверки {
    use super::*;

    #[test]
    fn раздел_libinput_разбирается() {
        let rc = r#"<labwc_config><libinput>
          <device category="non-touch"><pointerSpeed>-0.4</pointerSpeed><accelProfile>flat</accelProfile>
            <leftHanded>yes</leftHanded><naturalScroll>no</naturalScroll><scrollFactor>1.5</scrollFactor></device>
          <device category="touchpad"><tap>no</tap><naturalScroll>yes</naturalScroll><disableWhileTyping>no</disableWhileTyping>
            <clickMethod>clickfinger</clickMethod></device>
        </libinput></labwc_config>"#;
        let м = разбери(rc);
        assert_eq!(м.мышь.скорость, Some(-0.4));
        assert_eq!(м.мышь.ровно, Some(true));
        assert_eq!(м.мышь.левша, Some(true));
        assert_eq!(м.мышь.множитель, Some(1.5));
        assert_eq!(м.тачпад.касание, Some(false));
        assert_eq!(м.тачпад.обратная, Some(true));
        assert_eq!(м.тачпад.щелчок_пальцами, Some(true));
        assert_eq!(разбери("<labwc_config/>"), Мышь::default());
    }
}
