//! Сочетания клавиш. Берём их из того же rc.xml, что и labwc: один файл на
//! оба оконных сервера, и сменить сервер можно, не переписывая сочетаний

use smithay::input::keyboard::{keysyms, xkb, Keysym, ModifiersState};

#[derive(Debug, Clone, PartialEq)]
pub enum Дело {
    Запусти(String),
    Закрой,
    Следующее,
    ККраю(bool),
    Развернуть,
    Свернуть,
    НаСтол(usize),
    /// Ctrl+Alt+F1…F12 — на текстовую консоль (только на настоящем экране)
    Консоль(i32),
    ОтправьНаСтол(usize),
    Ничего,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Моды {
    pub logo: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl From<&ModifiersState> for Моды {
    fn from(м: &ModifiersState) -> Self {
        Моды { logo: м.logo, ctrl: м.ctrl, alt: м.alt, shift: м.shift }
    }
}

#[derive(Debug, Clone)]
pub struct Сочетание {
    pub моды: Моды,
    pub клавиша: Keysym,
    pub дела: Vec<Дело>,
    /// Одна клавиша-модификатор сама по себе (Super_L): срабатывает, когда её
    /// отпустили, ничего другого не нажав
    pub одинокая: bool,
}

fn раскрой(т: &str) -> String {
    т.replace("&quot;", "\"").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

fn свойство(тег: &str, имя: &str) -> Option<String> {
    let ключ = format!("{имя}=\"");
    let н = тег.find(&ключ)? + ключ.len();
    let к = тег[н..].find('"')?;
    Some(раскрой(&тег[н..н + к]))
}

fn клавиша_по_имени(имя: &str) -> Option<Keysym> {
    let с = xkb::keysym_from_name(имя, xkb::KEYSYM_NO_FLAGS);
    if с.raw() != keysyms::KEY_NoSymbol {
        return Some(с);
    }
    let с = xkb::keysym_from_name(имя, xkb::KEYSYM_CASE_INSENSITIVE);
    (с.raw() != keysyms::KEY_NoSymbol).then_some(с)
}

pub fn разбери(xml: &str) -> Vec<Сочетание> {
    // Комментарии выкидываем: в них тоже бывают примеры сочетаний
    let mut текст = String::with_capacity(xml.len());
    let mut ост = xml;
    while let Some(н) = ост.find("<!--") {
        текст.push_str(&ост[..н]);
        match ост[н..].find("-->") {
            Some(к) => ост = &ост[н + к + 3..],
            None => {
                ост = "";
                break;
            }
        }
    }
    текст.push_str(ост);

    let mut все = vec![];
    let mut ост = текст.as_str();
    while let Some(н) = ост.find("<keybind ") {
        ост = &ост[н..];
        let Some(конец_тега) = ост.find('>') else { break };
        let тег = &ост[..конец_тега];
        let самозакрытый = тег.ends_with('/');
        let (тело, дальше) = if самозакрытый {
            ("", &ост[конец_тега + 1..])
        } else {
            match ост.find("</keybind>") {
                Some(к) => (&ост[конец_тега + 1..к], &ост[к + 10..]),
                None => break,
            }
        };
        ост = дальше;
        let Some(ключ) = свойство(тег, "key") else { continue };
        let mut моды = Моды::default();
        let части: Vec<&str> = ключ.split('-').collect();
        let (имя, приставки) = части.split_last().unwrap();
        let mut годно = true;
        for п in приставки {
            match *п {
                "W" => моды.logo = true,
                "C" => моды.ctrl = true,
                "A" => моды.alt = true,
                "S" => моды.shift = true,
                _ => годно = false,
            }
        }
        let Some(клавиша) = клавиша_по_имени(имя) else { continue };
        if !годно {
            continue;
        }
        let одинокая = приставки.is_empty()
            && matches!(клавиша.raw(), keysyms::KEY_Super_L | keysyms::KEY_Super_R);
        let mut дела = vec![];
        let mut т = тело;
        while let Some(н) = т.find("<action") {
            let Some(к) = т[н..].find('>') else { break };
            let а = &т[н..н + к];
            т = &т[н + к + 1..];
            let имя = свойство(а, "name").unwrap_or_default();
            let номер = || свойство(а, "to").and_then(|н| н.parse::<usize>().ok()).unwrap_or(1).max(1) - 1;
            дела.push(match имя.as_str() {
                "Execute" => match свойство(а, "command") {
                    Some(к) => Дело::Запусти(к),
                    None => continue,
                },
                "Close" => Дело::Закрой,
                "NextWindow" => Дело::Следующее,
                "SnapToEdge" => match свойство(а, "direction").as_deref() {
                    Some("left") => Дело::ККраю(true),
                    Some("right") => Дело::ККраю(false),
                    Some("up") => Дело::Развернуть,
                    _ => continue,
                },
                "ToggleMaximize" | "Maximize" => Дело::Развернуть,
                "Iconify" => Дело::Свернуть,
                "GoToDesktop" => Дело::НаСтол(номер()),
                "SendToDesktop" => Дело::ОтправьНаСтол(номер()),
                "None" => Дело::Ничего,
                _ => continue,
            });
        }
        все.push(Сочетание { моды, клавиша, дела, одинокая });
    }
    все
}

/// Переменные среды из файла environment рядом с rc.xml — так же, как их
/// читает labwc: раскладки (XKB_DEFAULT_*) приходят оттуда
pub fn среда(текст: &str) -> Vec<(String, String)> {
    текст
        .lines()
        .map(str::trim)
        .filter(|с| !с.is_empty() && !с.starts_with('#'))
        .filter_map(|с| {
            let (к, з) = с.split_once('=')?;
            Some((к.trim().to_string(), з.trim().trim_matches('"').to_string()))
        })
        .collect()
}

#[cfg(test)]
mod проверки {
    use super::*;

    #[test]
    fn читает_сочетания_labwc() {
        let xml = r#"<keyboard>
          <!-- <keybind key="W-x"><action name="Close"/></keybind> -->
          <keybind key="Super_L"><action name="Execute" command="glower-menu" /></keybind>
          <keybind key="W-Tab">
            <action name="Execute" command="glower-menu отмени" />
            <action name="Execute" command="glower-menu задачи" />
          </keybind>
          <keybind key="A-F4"><action name="Close" /></keybind>
          <keybind key="W-Left"><action name="SnapToEdge" direction="left" /></keybind>
          <keybind key="W-C-A-S-F3"><action name="SendToDesktop" to="3" follow="no" /></keybind>
          <keybind key="XF86AudioPlay"><action name="Execute" command="a &amp;&amp; b" /></keybind>
          <keybind key="A-Up"><action name="None" /></keybind>
        </keyboard>"#;
        let в = разбери(xml);
        assert_eq!(в.len(), 7);
        assert!(в[0].одинокая);
        assert_eq!(в[1].дела.len(), 2);
        assert!(в[1].моды.logo && !в[1].моды.ctrl);
        assert_eq!(в[2].дела, vec![Дело::Закрой]);
        assert_eq!(в[3].дела, vec![Дело::ККраю(true)]);
        assert!(в[4].моды.logo && в[4].моды.ctrl && в[4].моды.alt && в[4].моды.shift);
        assert_eq!(в[4].дела, vec![Дело::ОтправьНаСтол(2)]);
        assert_eq!(в[5].дела, vec![Дело::Запусти("a && b".into())]);
        assert_eq!(в[6].дела, vec![Дело::Ничего]);
    }

    #[test]
    fn сочетания_системы_все_понятны() {
        let xml = include_str!("../../linux/labwc/rc.xml");
        let в = разбери(xml);
        assert!(в.len() >= 40, "прочитано {}", в.len());
        assert!(в.iter().any(|с| с.дела.contains(&Дело::НаСтол(3))));
    }

    #[test]
    fn среда_как_у_labwc() {
        let с = среда("# раскладки\nXKB_DEFAULT_LAYOUT=us,ru\nXKB_DEFAULT_OPTIONS=\"grp:alt_shift_toggle\"\n");
        assert_eq!(с, vec![("XKB_DEFAULT_LAYOUT".into(), "us,ru".into()), ("XKB_DEFAULT_OPTIONS".into(), "grp:alt_shift_toggle".into())]);
    }
}
