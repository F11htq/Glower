/* ==========================================================================
   Мост для нативных Параметров (glower-settings)

   Параметры теперь — настоящая программа на Rust. Системное (сеть, звук,
   принтеры, учётные записи, обновления) она спрашивает у агента сама. А то,
   что живёт в оболочке, — обои, тема, акцент, док, анимации, виджеты,
   раскладки — хранит страница, и только она умеет это применить.

   Поэтому страница рассказывает по шине, как у неё дела («параметры»), и
   выполняет просьбы («параметры-дело»). Рассказ повторяется после каждой
   перемены — откуда бы она ни пришла: из Параметров, из быстрых настроек
   панели или из начальной настройки.
   ========================================================================== */
'use strict';

const МостПараметров = {
  _ждём:null,

  состояние(){
    const есть = x => typeof x !== 'undefined';
    return {
      s:Store.s,
      kv:{
        ecoMode:KV.get('ecoMode', false), notif:KV.get('notif', true), autoLock:KV.get('autoLock', 0),
        mono:KV.get('mono', false), sticky:KV.get('sticky', false), zoom:KV.get('zoom', 1),
        'perf.lite':KV.get('perf.lite', false), 'perf.fps':KV.get('perf.fps', null),
        focusLen:KV.get('focusLen', '25')
      },
      'обои':WALLPAPERS.map(о => ({ id:о.id, 'имя':о.name, css:о.css })),
      'акценты':ACCENTS.map(а => ({ 'имя':а.n, a:а.a, b:а.b })),
      'программы':Object.entries(APPS).map(([id, а]) => ({ id, 'имя':а.name, 'знак':а.glyph, 'фон':а.bg || '' })),
      'виджеты':есть(WIDGETS) ? Object.entries(WIDGETS).map(([t, в]) => ({ t, 'имя':в.n, 'знак':в.e })) : [],
      'столов':window.WM ? WM.desks : 2,
      'языки':Object.entries(I18N.LANGS).map(([k, в]) => ({ k, 'имя':в.name })),
      'язык':I18N.lang(),
      'раскладки':Layouts.enabled(),
      'все_раскладки':Object.entries(Layouts.ALL).map(([id, р]) => ({ id, 'имя':р.name })),
      'открыватели':Assoc.openers().map(о => ({ id:о.id, 'имя':о.name, 'знак':о.glyph })),
      'связи':Assoc.map(),
      'бренд':{ 'имя':Brand.name, 'полное':Brand.full(), 'версия':Brand.versionLine() },
      'профиль':(() => { const я = Profiles.current() || {}; return { 'имя':я.name || S.userName, 'знак':я.emoji || '' }; })(),
      'фокус':this._фокус ? Math.max(0, Math.round((this._фокус - Date.now()) / 1000)) : 0
    };
  },

  скажи(){
    clearTimeout(this._ждём);
    this._ждём = setTimeout(() => Поверхности.скажи('параметры', this.состояние()), 150);
  },

  /* Что ещё сделать, когда сменилась настройка: применяет большинство их
     applySettings, но часть живёт в своих частях оболочки */
  ПОСЛЕ:{
    wallShuffle:() => Shell.wallShuffle(),
    showDeskWidgets:() => Shell.renderDeskWidgets(),
    autoTheme:() => Shell.autoTheme(),
    dnd:() => Shell.updateCC(),
    tz:() => { Shell.clock(); Shell.renderShell(); },
    city:() => { Shell.renderShell(); Weather.load(true); },
    showDesktopIcons:() => Shell.renderIcons()
  },

  ПОСЛЕ_KV:{
    ecoMode:() => applySettings(),
    zoom:v => { document.documentElement.style.fontSize = (16 * v) + 'px'; },
    mono:v => A11Y.applyMono(v),
    sticky:() => { A11Y.latched = {}; A11Y.badge(); },
    autoLock:() => AutoLock.touch(),
    'perf.lite':v => { KV.set('perf.userChoice', true); Perf.lite(v, true); }
  },

  фокус(минут){
    clearInterval(this._часы);
    if (!минут){
      this._фокус = 0;
      Store.set('dnd', false); Shell.updateCC();
      return;
    }
    this._фокус = Date.now() + минут * 60000;
    Store.set('dnd', true); Shell.updateCC();
    this._часы = setInterval(() => {
      if (Date.now() < this._фокус) return;
      this.фокус(0);
      Shell.toast('Фокусировка', 'Сессия завершена', '🎯'); Snd.note();
      this.скажи();
    }, 1000);
  },

  async дело(д){
    switch (д['что']){
      case 'set':
        Store.set(д.k, д.v);
        if (this.ПОСЛЕ[д.k]) this.ПОСЛЕ[д.k]();
        break;
      case 'kv':
        KV.set(д.k, д.v);
        if (this.ПОСЛЕ_KV[д.k]) this.ПОСЛЕ_KV[д.k](д.v);
        break;
      case 'список': {
        const с = (S[д.k] || []).slice();
        const i = с.indexOf(д.id);
        if (i >= 0) с.splice(i, 1); else с.push(д.id);
        Store.set(д.k, с);
        if (д.k === 'dockApps' && Shell.renderDock) Shell.renderDock();
        break;
      }
      case 'акцент':
        S.accentCustom = null; Store.set('accent', д.i);
        break;
      case 'акцент-свой':
        S.accentCustom = д['цвет']; Store.save(); applySettings();
        break;
      case 'виджет+': addWidget(д.t); break;
      case 'виджет-':
        S.deskWidgets.splice(д.i, 1); Store.save(); Shell.renderDeskWidgets();
        break;
      case 'столы': WM.desks = д.n; Shell.renderTaskview(); break;
      case 'мозаика': WM.tile(); break;
      case 'каскад': WM.cascade(); break;
      case 'звук': {
        const было = S.sounds; S.sounds = true;
        [0, 1, 2, 3].forEach(k => setTimeout(() => Snd.blip(440 * Math.pow(1.26, k), .18, 'triangle', .07), k * 150));
        setTimeout(() => { S.sounds = было; }, 800);
        break;
      }
      case 'уведомление': Shell.toast('Тестовое уведомление', 'Так выглядят баннеры ' + Brand.name, '🔔'); break;
      case 'фокус': this.фокус(+д['минут'] || 0); break;
      case 'имя': {
        const л = Profiles.list(), я = л.find(x => x.id === (Profiles.current() || {}).id);
        if (я){
          if (д['имя']) я.name = д['имя'];
          if (д['знак'] !== undefined) я.emoji = д['знак'] || я.name[0];
          Profiles.save(л);
        }
        if (д['имя']) Store.set('userName', д['имя']);
        Shell.renderShell();
        break;
      }
      case 'язык': I18N.set(д.k); break;
      case 'раскладки': {
        const о = await Layouts.применить(д['список'] || []);
        Поверхности.скажи('параметры-итог', { 'что':'раскладки', 'ошибка':о && о.ошибка || '', 'сразу':!!(о && о['сразу']) });
        break;
      }
      case 'связь': {
        const м = KV.get(Assoc.KEY, {});
        (д.exts || []).forEach(x => { м[x] = д.v; });
        KV.set(Assoc.KEY, м);
        break;
      }
      case 'связи-сброс': KV.set(Assoc.KEY, {}); break;
      case 'питание': Shell.power(д.action); break;
      case 'погода': await Weather.load(true); Shell.renderDeskWidgets(); break;
      case 'настройка': KV.set('setup.done', false); location.reload(); return;
      case 'сброс': Store.reset(); Shell.renderShell(); break;
    }
    this.скажи();
  },

  начни(){
    if (!window.Platform || Platform.mode !== 'native' || !window.Поверхности) return;
    /* Любая перемена настроек — повод пересказать: и та, что пришла отсюда,
       и та, что сделана на самой странице */
    const сохранить = Store.save.bind(Store);
    Store.save = () => { сохранить(); this.скажи(); };
    const записать = KV.set.bind(KV);
    KV.set = (k, v) => { записать(k, v); this.скажи(); };
    Поверхности.слушай(['параметры-дело'], м => {
      if (м.тема === 'параметры-дело' && м.что) this.дело(м.что).catch(e => console.warn('параметры:', e));
    });
    this.скажи();
  }
};
window.МостПараметров = МостПараметров;
addEventListener('DOMContentLoaded', () => setTimeout(() => МостПараметров.начни(), 300));
