/* ==========================================================================
   Ножницы: снимок экрана и разметка поверх него

   Внутри страницы снять экран нельзя: окно видит только себя. Кадр даёт
   оконный сервер, и просит его агент — сюда картинка приходит уже готовой.
   Поэтому без системы приложение честно говорит, что снимать нечем, а не
   делает вид, что работает.
   ========================================================================== */
'use strict';

APPS.snip = {
  name:'Ножницы', glyph:'✂️', bg:'linear-gradient(140deg,#7dd3fc,#0ea5e9)',
  w:820, h:600, single:true,

  render(win, opts){
    const wrap = el('div', 'app snip'); win.body.appendChild(wrap);

    /* Состояние держим на окне: приложение одно, и снимок должен пережить
       переключение на другое окно и обратно. */
    const S2 = win.data.snip = win.data.snip || { кадр:null, штрихи:[], отменённые:[],
      инструмент:'перо', цвет:'#ef4444', толщина:4, имя:'' };

    const шапка = el('div', 'snip-top');
    const холст = el('canvas', 'snip-canvas');
    const место = el('div', 'snip-stage');
    место.appendChild(холст);
    const низ = el('div', 'snip-bottom');
    wrap.append(шапка, место, низ);

    const ctx = холст.getContext('2d');

    /* ---------- рисование ---------- */
    const перерисуй = () => {
      if (!S2.кадр) return;
      холст.width = S2.кадр.width; холст.height = S2.кадр.height;
      ctx.drawImage(S2.кадр, 0, 0);
      S2.штрихи.forEach(ш => нарисуйШтрих(ctx, ш));
    };

    const нарисуйШтрих = (c, ш) => {
      c.save();
      c.lineCap = 'round'; c.lineJoin = 'round';
      c.strokeStyle = ш.цвет; c.fillStyle = ш.цвет; c.lineWidth = ш.толщина;
      if (ш.вид === 'перо' || ш.вид === 'маркер'){
        if (ш.вид === 'маркер'){ c.globalAlpha = 0.35; c.lineWidth = ш.толщина * 4; }
        c.beginPath();
        ш.точки.forEach((т, i) => i ? c.lineTo(т.x, т.y) : c.moveTo(т.x, т.y));
        c.stroke();
      } else if (ш.вид === 'рамка'){
        c.strokeRect(ш.x1, ш.y1, ш.x2 - ш.x1, ш.y2 - ш.y1);
      } else if (ш.вид === 'стрелка'){
        const dx = ш.x2 - ш.x1, dy = ш.y2 - ш.y1;
        const угол = Math.atan2(dy, dx), длина = Math.max(12, ш.толщина * 4);
        c.beginPath(); c.moveTo(ш.x1, ш.y1); c.lineTo(ш.x2, ш.y2); c.stroke();
        c.beginPath();
        c.moveTo(ш.x2, ш.y2);
        c.lineTo(ш.x2 - длина * Math.cos(угол - 0.4), ш.y2 - длина * Math.sin(угол - 0.4));
        c.lineTo(ш.x2 - длина * Math.cos(угол + 0.4), ш.y2 - длина * Math.sin(угол + 0.4));
        c.closePath(); c.fill();
      } else if (ш.вид === 'размыть'){
        /* Замазать личное — самое частое, зачем вообще правят снимок.
           Рисуем плотный прямоугольник: это надёжнее размытия, из которого
           текст иногда восстанавливают. */
        c.globalAlpha = 1; c.fillStyle = '#111827';
        c.fillRect(ш.x1, ш.y1, ш.x2 - ш.x1, ш.y2 - ш.y1);
      } else if (ш.вид === 'текст'){
        c.font = (ш.толщина * 6) + 'px system-ui, sans-serif';
        c.textBaseline = 'top';
        c.strokeStyle = 'rgba(0,0,0,.55)'; c.lineWidth = 3;
        c.strokeText(ш.текст, ш.x1, ш.y1);
        c.fillText(ш.текст, ш.x1, ш.y1);
      }
      c.restore();
    };

    /* ---------- ввод мышью ---------- */
    let текущий = null;
    const вКадре = e => {
      const r = холст.getBoundingClientRect();
      return { x:(e.clientX - r.left) * (холст.width / r.width),
               y:(e.clientY - r.top) * (холст.height / r.height) };
    };

    холст.onpointerdown = async e => {
      if (!S2.кадр) return;
      /* Захват указателя нужен, чтобы линия не обрывалась, если мышь ушла
         за край окна. Но он есть не везде и умеет бросаться — рисование от
         этого зависеть не должно. */
      try { холст.setPointerCapture(e.pointerId); } catch(err){}
      const т = вКадре(e);

      if (S2.инструмент === 'текст'){
        const слово = await Dlg.prompt('Надпись', 'Что написать на снимке?', '', '🔤');
        if (!слово) return;
        S2.штрихи.push({ вид:'текст', текст:слово, x1:т.x, y1:т.y,
                         цвет:S2.цвет, толщина:S2.толщина });
        S2.отменённые = []; перерисуй(); обнови();
        return;
      }

      текущий = { вид:S2.инструмент, цвет:S2.цвет, толщина:S2.толщина,
                  точки:[т], x1:т.x, y1:т.y, x2:т.x, y2:т.y };
    };

    холст.onpointermove = e => {
      if (!текущий) return;
      const т = вКадре(e);
      текущий.точки.push(т); текущий.x2 = т.x; текущий.y2 = т.y;
      перерисуй(); нарисуйШтрих(ctx, текущий);
    };

    const закончи = () => {
      if (!текущий) return;
      const пусто = текущий.вид !== 'перо' && текущий.вид !== 'маркер'
        && Math.abs(текущий.x2 - текущий.x1) < 3 && Math.abs(текущий.y2 - текущий.y1) < 3;
      if (!пусто){ S2.штрихи.push(текущий); S2.отменённые = []; }
      текущий = null; перерисуй(); обнови();
    };
    холст.onpointerup = закончи;
    холст.onpointercancel = закончи;

    /* ---------- съёмка ---------- */
    const снимай = async (режим, задержка) => {
      if (!window.Platform || Platform.mode !== 'native'){
        Dlg.alert('Снимок экрана',
          'Снять экран может только система, а оболочка сейчас работает без неё — ' +
          'в обычном браузере. Здесь показывать нечего.', '✂️');
        return;
      }
      /* Своё окно в кадр не берём: человек снимает то, что за ним, а не
         инструмент. Прячем и ждём, пока оконный сервер успеет перерисовать. */
      const было = win.el.style.visibility;
      win.el.style.visibility = 'hidden';
      await new Promise(r => setTimeout(r, 260));

      try {
        const о = await Platform.rpc('sys.shot', { 'режим':режим, 'задержка':задержка || 0 });
        if (о && о['отменено']) return;
        if (!о || !о.dataUrl) throw new Error('система не вернула снимок');
        await новыйКадр(о.dataUrl);
        Shell.toast('Ножницы', 'Снимок готов — можно подписать и сохранить', '✂️');
      } catch(e){
        Dlg.alert('Не вышло снять экран', String(e.message || e), '⚠️');
      } finally {
        win.el.style.visibility = было;
      }
    };

    const новыйКадр = dataUrl => new Promise(готово => {
      const и = new Image();
      и.onload = () => {
        S2.кадр = и; S2.штрихи = []; S2.отменённые = [];
        S2.имя = 'Снимок ' + new Date().toLocaleString('ru-RU').replace(/[:.]/g, '-');
        перерисуй(); обнови(); готово();
      };
      и.src = dataUrl;
    });
    win.data.новыйКадр = новыйКадр;

    /* ---------- сохранение ---------- */
    const вФайл = async () => {
      if (!S2.кадр) return;
      const dataUrl = холст.toDataURL('image/png');
      const путь = ['Изображения', 'Снимки'];
      const имя = (S2.имя || 'Снимок') + '.png';
      try {
        if (window.Platform && Platform.mode === 'native'){
          await Platform.rpc('fs.mkdir', { path:путь });
          await Platform.rpc('fs.writeDataUrl', { path:[...путь, имя], dataUrl });
        } else {
          FS.mkdir(['Изображения'], 'Снимки');
          FS.write(путь, имя, dataUrl);
        }
        Shell.toast('Ножницы', 'Сохранено: Изображения → Снимки → ' + имя, '💾', 6000);
      } catch(e){ Dlg.alert('Не вышло сохранить', String(e.message || e), '⚠️'); }
    };

    const вБуфер = async () => {
      if (!S2.кадр) return;
      const dataUrl = холст.toDataURL('image/png');
      try {
        if (window.Platform && Platform.mode === 'native'){
          await Platform.rpc('sys.shot.вбуфер', { dataUrl });
        } else {
          const бинарь = await (await fetch(dataUrl)).blob();
          await navigator.clipboard.write([new ClipboardItem({ 'image/png':бинарь })]);
        }
        Shell.toast('Ножницы', 'Снимок в буфере — вставьте куда нужно', '📋');
      } catch(e){ Dlg.alert('Не вышло скопировать', String(e.message || e), '⚠️'); }
    };

    /* ---------- шапка: чем снимать ---------- */
    const кнопка = (текст, подпись, дело) => {
      const b = el('button', 'btn', текст);
      if (подпись) b.title = подпись;
      b.onclick = дело;
      return b;
    };

    const задержкаSel = el('select', 'inp');
    [['Без задержки', 0], ['3 секунды', 3], ['5 секунд', 5], ['10 секунд', 10]]
      .forEach(([н, в]) => { const o = el('option', '', н); o.value = в; задержкаSel.appendChild(o); });

    шапка.append(
      кнопка('✂️ Область', 'Выделить прямоугольник на экране',
        () => снимай('область', +задержкаSel.value)),
      кнопка('🖥 Весь экран', 'Снять экран целиком',
        () => снимай('экран', +задержкаSel.value)),
      задержкаSel
    );

    /* ---------- шапка: чем рисовать ---------- */
    const инструменты = [['перо', '✏️', 'Перо'], ['маркер', '🖍', 'Маркер'],
      ['стрелка', '➘', 'Стрелка'], ['рамка', '▭', 'Рамка'],
      ['размыть', '⬛', 'Замазать личное'], ['текст', '🔤', 'Надпись']];
    const рядИнстр = el('div', 'snip-tools');
    const кнопкиИнстр = {};
    инструменты.forEach(([id, знак, подпись]) => {
      const b = el('button', 'btn', знак);
      b.title = подпись;
      b.onclick = () => { S2.инструмент = id; обнови(); };
      кнопкиИнстр[id] = b;
      рядИнстр.appendChild(b);
    });

    const цвета = ['#ef4444', '#f59e0b', '#22c55e', '#3b82f6', '#a855f7', '#111827', '#ffffff'];
    const рядЦвет = el('div', 'snip-colors');
    const кнопкиЦвет = {};
    цвета.forEach(ц => {
      const b = el('button', 'snip-color');
      b.style.background = ц;
      b.title = 'Цвет';
      b.onclick = () => { S2.цвет = ц; обнови(); };
      кнопкиЦвет[ц] = b;
      рядЦвет.appendChild(b);
    });

    const толщинаIn = el('input', 'snip-width');
    толщинаIn.type = 'range'; толщинаIn.min = 2; толщинаIn.max = 16; толщинаIn.value = S2.толщина;
    толщинаIn.title = 'Толщина линии';
    толщинаIn.oninput = () => { S2.толщина = +толщинаIn.value; };

    const отменить = кнопка('↶', 'Отменить последнее (Ctrl+Z)', () => {
      const ш = S2.штрихи.pop(); if (ш) S2.отменённые.push(ш);
      перерисуй(); обнови();
    });
    const вернуть = кнопка('↷', 'Вернуть (Ctrl+Shift+Z)', () => {
      const ш = S2.отменённые.pop(); if (ш) S2.штрихи.push(ш);
      перерисуй(); обнови();
    });

    низ.append(рядИнстр, рядЦвет, толщинаIn, отменить, вернуть,
      кнопка('📋 Копировать', 'Положить снимок в буфер обмена', вБуфер),
      кнопка('💾 Сохранить', 'Сохранить в Изображения → Снимки', вФайл));
    низ.querySelector('button:last-child').classList.add('pri');

    /* ---------- обновление вида ---------- */
    const пусто = el('div', 'snip-empty',
      'Снимка пока нет.<br><span class="muted">Нажмите «Область» или «Весь экран» сверху. ' +
      'Быстрые клавиши: PrintScreen — весь экран, Win+Shift+S — область.</span>');
    место.appendChild(пусто);

    function обнови(){
      const есть = !!S2.кадр;
      пусто.style.display = есть ? 'none' : '';
      холст.style.display = есть ? '' : 'none';
      низ.style.display = есть ? '' : 'none';
      отменить.disabled = !S2.штрихи.length;
      вернуть.disabled = !S2.отменённые.length;
      Object.entries(кнопкиИнстр).forEach(([id, b]) => b.classList.toggle('pri', id === S2.инструмент));
      Object.entries(кнопкиЦвет).forEach(([ц, b]) => b.classList.toggle('вбран', ц === S2.цвет));
    }

    /* горячие клавиши внутри окна */
    win.body.tabIndex = 0;
    win.body.addEventListener('keydown', e => {
      if (!(e.ctrlKey || e.metaKey)) return;
      const k = e.key.toLowerCase();
      if (k === 'z' && !e.shiftKey){ e.preventDefault(); отменить.click(); }
      else if ((k === 'z' && e.shiftKey) || k === 'y'){ e.preventDefault(); вернуть.click(); }
      else if (k === 's'){ e.preventDefault(); вФайл(); }
      else if (k === 'c'){ e.preventDefault(); вБуфер(); }
    });

    обнови();

    /* Окно могли открыть уже с готовым снимком — так делают быстрые клавиши. */
    if (opts && opts.dataUrl) новыйКадр(opts.dataUrl);
    else if (opts && opts.снять) снимай(opts.снять, 0);
    else if (S2.кадр) перерисуй();
  },

  onReopen(win, opts){
    if (opts && opts.dataUrl && win.data.новыйКадр) win.data.новыйКадр(opts.dataUrl);
    else if (opts && opts.снять && win.data.новыйКадр) WM.open('snip', opts);
  }
};

/* ==========================================================================
   Съёмка по требованию: клавишами, кнопкой или по просьбе оконного сервера

   Сочетания клавиш ловит оконный сервер, а не страница. Причина простая:
   страница получает клавиши, только пока оболочка в фокусе. Стоило открыть
   любую программу — и Win+Shift+S уходил ей, а у нас не происходило
   ничего. Поэтому сочетание висит на оконном сервере, тот зовёт маленькую
   программу glower-shot, она говорит агенту, а агент — сюда.

   Здесь же остаётся и свой обработчик клавиш: он нужен, когда оболочка в
   фокусе, и в браузере, где никакого оконного сервера нет вовсе.
   ========================================================================== */
const Снимки = {
  занят:false,

  /* Доставить недостающее. Тем же путём, каким ставится всё остальное:
     своего способа ставить программы у нас нет и быть не должно. */
  async доставь(чего){
    try {
      await Platform.rpc('pkg.install', { name:чего, source:'apt' });
      Shell.toast('Ножницы', 'Ставлю «' + чего + '»…', '📦', 6000);
      for (let i = 0; i < 400; i++){
        const j = await Platform.rpc('pkg.job').catch(() => ({ running:false }));
        if (!j.running){
          if (j.ok) Shell.toast('Ножницы', '«' + чего + '» поставлена — снимки заработают', '✅', 8000);
          else Dlg.alert('Не вышло поставить', String(j.error || 'причина неизвестна'), '⚠️');
          return;
        }
        await new Promise(r => setTimeout(r, 1500));
      }
    } catch(e){ Dlg.alert('Не вышло поставить', String(e.message || e), '⚠️'); }
  },

  /* ---------- своё выделение области ----------

     Как в Windows: экран замирает, темнеет, и человек тянет по нему
     рамку. Отличие одно — у Windows это делает сама система, а у нас
     делала системная программа slurp, и делала она это по-разному на
     разных машинах: на этой рамка не появлялась вовсе. Человек жал
     сочетание, тянул мышью и не получал ничего.

     Теперь выделение — наше. Сперва снимаем весь экран (это умеет grim, и
     он работает), показываем кадр во весь экран, и рамку человек тянет уже
     по нашей картинке. Никакой второй программы в этом больше не
     участвует, а значит, и ломаться нечему. Обрезаем сами, по холсту.

     Кадр — застывший: под ним уже ничего не меняется, и это тоже как в
     Windows. Подвинуть окно, пока тянешь рамку, нельзя — и не нужно. */
  выдели(dataUrl){
    return new Promise(готово => {
      /* Полоса наверху — отдельная поверхность, и наше затемнение до неё
         не достаёт: на застывшем кадре она одна остаётся яркой, да ещё и
         с живыми часами поверх застывших. Просим её притемниться самой. */
      const скажиПанели = как => {
        try { if (window.Поверхности && Поверхности.скажи) Поверхности.скажи('затемни', как); }
        catch(e){}
      };
      скажиПанели(true);
      const слой = el('div', 'выделение');
      const кадр = el('img', 'выд-кадр');
      кадр.src = dataUrl; кадр.draggable = false;
      const рамка = el('div', 'выд-рамка');
      const подпись = el('div', 'выд-подпись', 'Выделите область мышью · Esc — отмена');
      /* Затемнение — четырьмя полосами вокруг рамки, а не дыркой в одном
         слое: так оно работает всюду одинаково и ничего не стоит. */
      const тени = ['в', 'н', 'л', 'п'].map(к => el('div', 'выд-тень выд-' + к));
      слой.append(кадр, ...тени, рамка, подпись);
      document.body.appendChild(слой);

      let x0 = 0, y0 = 0, тянем = false;
      const область = () => {
        const r = рамка.getBoundingClientRect();
        return { x:Math.round(r.left), y:Math.round(r.top),
                 w:Math.round(r.width), h:Math.round(r.height) };
      };
      const покажи = (x, y, w, h) => {
        Object.assign(рамка.style, { left:x + 'px', top:y + 'px',
                                     width:w + 'px', height:h + 'px' });
        const W = innerWidth, H = innerHeight;
        Object.assign(тени[0].style, { left:0, top:0, width:W + 'px', height:y + 'px' });
        Object.assign(тени[1].style, { left:0, top:(y + h) + 'px', width:W + 'px', height:Math.max(0, H - y - h) + 'px' });
        Object.assign(тени[2].style, { left:0, top:y + 'px', width:x + 'px', height:h + 'px' });
        Object.assign(тени[3].style, { left:(x + w) + 'px', top:y + 'px', width:Math.max(0, W - x - w) + 'px', height:h + 'px' });
        подпись.textContent = w > 8 && h > 8 ? (w + ' × ' + h) : 'Выделите область мышью · Esc — отмена';
      };
      покажи(0, 0, 0, 0);

      const конец = что => {
        скажиПанели(false);
        слой.remove();
        removeEventListener('keydown', поКлавише, true);
        готово(что);
      };
      const поКлавише = е => {
        if (е.key === 'Escape'){ е.preventDefault(); е.stopPropagation(); конец(null); }
      };
      addEventListener('keydown', поКлавише, true);

      слой.onmousedown = е => {
        if (е.button !== 0) return конец(null);   // правой — отмена, как везде
        тянем = true; x0 = е.clientX; y0 = е.clientY;
        рамка.classList.add('видна');
        покажи(x0, y0, 0, 0);
      };
      слой.onmousemove = е => {
        if (!тянем) return;
        покажи(Math.min(x0, е.clientX), Math.min(y0, е.clientY),
               Math.abs(е.clientX - x0), Math.abs(е.clientY - y0));
      };
      слой.onmouseup = () => {
        if (!тянем) return;
        тянем = false;
        const о = область();
        /* Щелчок без протяжки — это отмена, а не снимок в один пиксель. */
        if (о.w < 6 || о.h < 6) return конец(null);
        конец(this.обрежь(кадр, о));
      };
    });
  },

  /* Вырезать кусок кадра. Кадр может быть крупнее экрана — на машинах с
     увеличением картинка приходит в точках устройства, а рамку человек
     тянул в точках страницы. Считаем по отношению одного к другому. */
  обрежь(картинка, о){
    const кx = картинка.naturalWidth / innerWidth;
    const кy = картинка.naturalHeight / innerHeight;
    const холст = document.createElement('canvas');
    холст.width = Math.max(1, Math.round(о.w * кx));
    холст.height = Math.max(1, Math.round(о.h * кy));
    холст.getContext('2d').drawImage(картинка,
      Math.round(о.x * кx), Math.round(о.y * кy),
      холст.width, холст.height, 0, 0, холст.width, холст.height);
    return холст.toDataURL('image/png');
  },

  async сними(режим){
    if (this.занят) return;
    if (!window.Platform || Platform.mode !== 'native'){
      Shell.toast('Ножницы', 'Снять экран может только система — здесь нечего снимать', '✂️');
      return;
    }
    this.занят = true;
    try {
      /* Область или весь экран — снимаем всегда весь: обрезать умеем сами,
         и это единственный способ показать человеку застывший кадр, по
         которому он и тянет рамку. */
      const о = await Platform.rpc('sys.shot', { 'режим':'экран' });
      if (!о || !о.dataUrl) throw new Error('система не вернула снимок');

      if (режим === 'область'){
        document.body.classList.add('ждём-область');
        const кусок = await this.выдели(о.dataUrl);
        document.body.classList.remove('ждём-область');
        /* Отмену проговариваем: молчание в ответ на нажатие человек читает
           как поломку, а не как «я же сам отменил». */
        if (!кусок){ Shell.toast('Ножницы', 'Снимок отменён', '✂️', 3000); return; }
        о.dataUrl = кусок;
      }
      /* Как в Windows: снимок сразу в буфере, а окно открывается, чтобы его
         можно было подписать или сохранить. */
      Platform.rpc('sys.shot.вбуфер', { dataUrl:о.dataUrl }).catch(() => {});
      WM.open('snip', { dataUrl:о.dataUrl });
      Shell.toast('Ножницы', 'Снимок сделан и уже в буфере обмена', '✂️');
    } catch(e){
      const текст = String(e.message || e);
      /* Не хватает программы — случай особый и поправимый.
      
         Полоска в углу тут не годится вдвойне: она гаснет через семь
         секунд, и человек, нажавший Win+Shift+S и не увидевший рамки, о
         ней даже не узнает. А сделать надо всего одно действие — поставить
         недостающее. Спрашиваем прямо и ставим по кнопке. */
      const чего = (текст.match(/нет (grim|slurp|maim|slop)/) || [])[1];
      if (чего){
        const ставить = await Dlg.open({ type:'confirm', icon:'✂️',
          title:'Для снимков не хватает программы',
          text:'Экран снимает системная программа «' + чего
             + '», а её нет на машине. Поставить её сейчас? Это небольшой пакет.',
          okText:'Поставить', cancelText:'Не сейчас' });
        if (ставить) await this.доставь(чего);
        return;
      }
      Dlg.alert('Не вышло снять экран', текст, '⚠️');
    } finally {
      document.body.classList.remove('ждём-область');
      this.занят = false;
    }
  }
};
window.Снимки = Снимки;

addEventListener('keydown', e => {
  if (e.key === 'PrintScreen'){ e.preventDefault(); Снимки.сними('экран'); return; }
  if ((e.metaKey || (e.ctrlKey && e.altKey)) && e.shiftKey && e.key.toLowerCase() === 's'){
    e.preventDefault(); Снимки.сними('область');
  }
});
