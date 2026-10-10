/* ==========================================================================
   Раскладки клавиатуры: индикатор в панели и переключение по Alt+Shift

   Что это делает по-настоящему: пока выбрана русская раскладка, буквы,
   набранные на латинской клавиатуре, превращаются в кириллицу внутри окон
   системы. Разложить клавиатуру самой операционной системы страница,
   разумеется, не может — и не притворяется.
   ========================================================================== */
'use strict';

const Layouts = {
  ALL:{
    ru:{ code:'РУС', name:'Русская', flag:'🇷🇺',
      map:{ q:'й',w:'ц',e:'у',r:'к',t:'е',y:'н',u:'г',i:'ш',o:'щ',p:'з','[':'х',']':'ъ',
            a:'ф',s:'ы',d:'в',f:'а',g:'п',h:'р',j:'о',k:'л',l:'д',';':'ж',"'":'э',
            z:'я',x:'ч',c:'с',v:'м',b:'и',n:'т',m:'ь',',':'б','.':'ю','`':'ё','/':'.' } },
    en:{ code:'ENG', name:'Английская', flag:'🇬🇧', map:null },
    de:{ code:'DEU', name:'Немецкая', flag:'🇩🇪',
      map:{ y:'z', z:'y', ';':'ö', "'":'ä', '[':'ü', '-':'ß' } }
  },

  /* Каталог раскладок — те, что выбирают чаще всего, с русскими именами.
     Имена ключей — имена раскладок XKB, кроме «en»: так у нас исторически
     зовётся английская (в XKB это «us»). Всё остальное, что есть в системе,
     приходит от агента из базы XKB и показывается с её именем. Буквы
     внутри окон оболочки подменяются только для тех, у кого есть таблица
     (русская, немецкая); остальные набирает сама система — оконный сервер
     отдаёт уже готовые буквы. */
  КАТАЛОГ:[
    ['ru','Русская','🇷🇺','Russian'], ['en','Английская','🇬🇧','English'], ['ua','Украинская','🇺🇦','Ukrainian'], ['by','Белорусская','🇧🇾','Belarusian'],
    ['kz','Казахская','🇰🇿','Kazakh'], ['de','Немецкая','🇩🇪','German'], ['fr','Французская','🇫🇷','French'], ['es','Испанская','🇪🇸','Spanish'],
    ['it','Итальянская','🇮🇹','Italian'], ['pt','Португальская','🇵🇹','Portuguese'], ['br','Португальская (Бразилия)','🇧🇷','Portuguese (Brazil)'],
    ['gb','Английская (Великобритания)','🇬🇧','English (UK)'], ['pl','Польская','🇵🇱','Polish'], ['cz','Чешская','🇨🇿','Czech'],
    ['sk','Словацкая','🇸🇰','Slovak'], ['tr','Турецкая','🇹🇷','Turkish'], ['gr','Греческая','🇬🇷','Greek'], ['il','Иврит','🇮🇱','Hebrew'],
    ['ara','Арабская','🇸🇦','Arabic'], ['ir','Персидская','🇮🇷','Persian'], ['in','Индийская (хинди)','🇮🇳','Indian (Hindi)'],
    ['cn','Китайская','🇨🇳','Chinese'], ['jp','Японская','🇯🇵','Japanese'], ['kr','Корейская','🇰🇷','Korean'], ['uz','Узбекская','🇺🇿','Uzbek'],
    ['az','Азербайджанская','🇦🇿','Azerbaijani'], ['am','Армянская','🇦🇲','Armenian'], ['ge','Грузинская','🇬🇪','Georgian'],
    ['kg','Киргизская','🇰🇬','Kyrgyz'], ['tj','Таджикская','🇹🇯','Tajik'], ['tm','Туркменская','🇹🇲','Turkmen'], ['mn','Монгольская','🇲🇳','Mongolian'],
    ['md','Молдавская','🇲🇩','Moldavian'], ['ro','Румынская','🇷🇴','Romanian'], ['bg','Болгарская','🇧🇬','Bulgarian'], ['rs','Сербская','🇷🇸','Serbian'],
    ['hr','Хорватская','🇭🇷','Croatian'], ['si','Словенская','🇸🇮','Slovenian'], ['hu','Венгерская','🇭🇺','Hungarian'], ['fi','Финская','🇫🇮','Finnish'],
    ['se','Шведская','🇸🇪','Swedish'], ['no','Норвежская','🇳🇴','Norwegian'], ['dk','Датская','🇩🇰','Danish'], ['nl','Нидерландская','🇳🇱','Dutch'],
    ['be','Бельгийская','🇧🇪','Belgian'], ['ch','Швейцарская','🇨🇭','Swiss'], ['at','Австрийская','🇦🇹','Austrian'], ['ee','Эстонская','🇪🇪','Estonian'],
    ['lv','Латышская','🇱🇻','Latvian'], ['lt','Литовская','🇱🇹','Lithuanian'], ['th','Тайская','🇹🇭','Thai'], ['vn','Вьетнамская','🇻🇳','Vietnamese'],
    ['latam','Испанская (Латинская Америка)','🌎','Spanish (Latin America)'], ['ca','Канадская','🇨🇦','Canadian'], ['ie','Ирландская','🇮🇪','Irish']
  ],
  /* Сколько раскладок XKB держит разом */
  МАКС:4,

  xkb(id){ return id === 'en' ? 'us' : id; },
  изXkb(x){ return x === 'us' ? 'en' : x; },

  /* Раскладки системы (из базы XKB) — дополняют каталог тем, чего в нём нет */
  дополни(список){
    (список || []).forEach(({ id, имя }) => {
      const наш = this.изXkb(id);
      if (this.ALL[наш] || !/^[a-z]{2,8}$/.test(id)) return;
      this.ALL[наш] = { code:id.toUpperCase().slice(0, 3), name:имя, en:имя, flag:'⌨️', map:null, системная:true };
    });
  },

  /* Все раскладки для выбора: сначала каталог в его порядке, потом
     остальные системные по алфавиту. */
  все(){
    const из_каталога = this.КАТАЛОГ.map(([id]) => id).filter(id => this.ALL[id]);
    const прочие = Object.keys(this.ALL).filter(id => !из_каталога.includes(id))
      .sort((a, b) => this.ALL[a].name.localeCompare(this.ALL[b].name));
    return [...из_каталога, ...прочие];
  },

  /* Включить раскладки: у себя и в самой системе. Возвращает ответ агента:
     применилось ли сразу или со следующего входа. */
  async применить(список){
    const l = [...new Set(список)].filter(id => this.ALL[id]).slice(0, this.МАКС);
    if (!l.length) l.push('en');
    KV.set('kb.enabled', l);
    if (!l.includes(this.current())) KV.set('kb.current', l[0]);
    this.paint();
    if (window.Platform && Platform.mode === 'native'){
      try { return await Platform.rpc('sys.kb.set', { 'раскладки':l.map(id => this.xkb(id)) }); }
      catch(e){ return { ошибка:String(e.message || e) }; }
    }
    return null;
  },

  /* Что включено в системе — главнее того, что помнит страница: раскладку
     могли сменить и в другой день, и на другом экране. */
  async изСистемы(){
    if (!window.Platform || Platform.mode !== 'native') return;
    try {
      const д = await Platform.rpc('sys.kb.list');
      this.дополни(д['список']);
      const выбрано = (д['выбрано'] || []).map(x => this.изXkb(x)).filter(id => this.ALL[id]);
      if (выбрано.length){
        KV.set('kb.enabled', выбрано);
        if (!выбрано.includes(this.current())) KV.set('kb.current', выбрано[0]);
        this.paint();
      }
    } catch(e){}
  },

  /* Список для выбора: поиск сверху, выбранные — плашками по порядку
     (первая — главная), ниже все раскладки системы с отметкой. Сам список
     перерисовывает только себя: поле поиска не теряет фокус на каждом
     щелчке. */
  список(место, { получить, задать, англ = false }){
    /* Надписи и имена — на языке мастера: настройка переводится сразу, не
       дожидаясь смены языка всей системы */
    const Т = англ
      ? { найти:'🔎 Find a layout', главная:'main', убрать:'Remove', одна:'At least one layout is needed',
          полно:м => `${м} enabled — the system can't hold more at once. Remove one to pick another.`,
          можно:м => `You can enable up to ${м}. Switch with Alt + Shift.`, нет:'No such layout' }
      : { найти:'🔎 Найти раскладку', главная:'главная', убрать:'Убрать', одна:'Хотя бы одна раскладка нужна',
          полно:м => `Включено ${м} — больше система держать разом не умеет. Уберите одну, чтобы выбрать другую.`,
          можно:м => `Можно включить до ${м}. Переключение — Alt + Shift.`, нет:'Такой раскладки нет' };
    const имя = д => (англ && д.en) || д.name;
    const корень = el('div', 'kb-выбор');
    const поиск = el('input', 'inp kb-поиск');
    поиск.placeholder = Т.найти;
    const выбранные = el('div', 'kb-плашки');
    const заметка = el('div', 'kb-заметка');
    const все = el('div', 'kb-список');
    корень.append(выбранные, поиск, заметка, все);
    место.appendChild(корень);

    const рисуй = () => {
      const сейчас = получить();
      выбранные.innerHTML = '';
      сейчас.forEach((id, i) => {
        const д = this.ALL[id]; if (!д) return;
        const п = el('button', 'kb-плашка', `<span>${д.flag}</span>${esc(имя(д))}${i === 0 ? ` <small>${Т.главная}</small>` : ''}
          ${сейчас.length > 1 ? '<i>×</i>' : ''}`);
        п.title = сейчас.length > 1 ? Т.убрать : Т.одна;
        п.onclick = () => { if (сейчас.length > 1){ задать(сейчас.filter(x => x !== id)); рисуй(); } };
        выбранные.appendChild(п);
      });
      const полно = сейчас.length >= this.МАКС;
      заметка.textContent = полно ? Т.полно(this.МАКС) : Т.можно(this.МАКС);
      const q = поиск.value.trim().toLowerCase();
      все.innerHTML = '';
      const список = this.все().filter(id => !q || this.ALL[id].name.toLowerCase().includes(q)
        || (this.ALL[id].en || '').toLowerCase().includes(q)
        || this.xkb(id).includes(q) || this.ALL[id].code.toLowerCase().includes(q));
      список.forEach(id => {
        const д = this.ALL[id], вкл = сейчас.includes(id);
        const с = el('button', 'kb-строка' + (вкл ? ' on' : ''),
          `<span class="kb-флаг">${д.flag}</span><b>${esc(имя(д))}</b><small>${esc(this.xkb(id))}</small><span class="kb-галка">${вкл ? '✓' : ''}</span>`);
        с.disabled = !вкл && полно;
        с.onclick = () => {
          const т = получить();
          /* Предел проверяем и здесь: заблокированная кнопка могла уже
             смениться новой, а старая — успеть получить щелчок */
          if (!т.includes(id) && т.length >= this.МАКС) return;
          задать(т.includes(id) ? (т.length > 1 ? т.filter(x => x !== id) : т) : [...т, id]);
          if (window.Snd && Snd.click) Snd.click();
          рисуй();
        };
        все.appendChild(с);
      });
      if (!список.length) все.appendChild(el('div', 'kb-пусто', Т.нет));
    };
    поиск.addEventListener('input', рисуй);
    рисуй();
    return { рисуй };
  },

  enabled(){ const l = KV.get('kb.enabled', ['en', 'ru']); return l.filter(x => this.ALL[x]); },
  /* по умолчанию латиница: система не трогает набор, пока её об этом не попросят */
  current(){ const c = KV.get('kb.current', 'en'); return this.ALL[c] ? c : 'en'; },
  def(){ return this.ALL[this.current()]; },

  set(id){
    if (!this.ALL[id]) return;
    KV.set('kb.current', id);
    this.paint();
    OSD && OSD.show(this.ALL[id].flag, 100, this.ALL[id].code);
  },
  next(){
    const l = this.enabled();
    if (l.length < 2) return;
    this.set(l[(l.indexOf(this.current()) + 1) % l.length]);
  },
  toggle(id){
    let l = this.enabled();
    l = l.includes(id) ? l.filter(x => x !== id) : [...l, id].slice(0, this.МАКС);
    return this.применить(l);
  },

  /* ---------- индикатор в трее ---------- */
  paint(){
    let b = $('#kb-badge');
    if (!b){
      b = el('button', 'tray-btn kb-badge');
      b.id = 'kb-badge';
      const tray = $('#tb-tray');
      if (!tray) return;
      tray.insertBefore(b, $('#tray-clock', tray));
      b.onclick = e => {
        e.stopPropagation();
        const r = b.getBoundingClientRect();
        Shell.ctx(r.left - 60, r.bottom + 8, [
          { t:'Раскладка клавиатуры', head:true },
          ...this.enabled().map(id => ({
            i:this.ALL[id].flag,
            t:this.ALL[id].name + (id === this.current() ? ' ✓' : ''),
            k:this.ALL[id].code,
            f:() => this.set(id)
          })),
          'hr',
          { i:'⚙️', t:'Параметры языка', f:() => WM.open('settings', { section:'time' }) }
        ]);
      };
    }
    const d = this.def();
    b.textContent = d.code;
    b.dataset.tip = `${d.name} · Alt+Shift для переключения`;
  },

  /* ---------- превращение латиницы в выбранную раскладку ---------- */
  install(){
    addEventListener('keydown', e => {
      if (e.key === 'Shift' && e.altKey) { e.preventDefault(); return this.next(); }
      if (e.key === 'Alt' && e.shiftKey) { e.preventDefault(); return this.next(); }

      const map = this.def().map;
      if (!map) return;                                  // латиница — ничего не трогаем
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      if (e.key.length !== 1) return;

      const t = e.target;
      const editable = t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable);
      if (!editable) return;
      if (t.type && ['password','number','color','range','file'].includes(t.type)) return;

      const lower = e.key.toLowerCase();
      const rep = map[lower];
      if (!rep) return;                                  // символа нет в раскладке
      if (!/[a-z\[\];'`,.\/\-]/.test(lower)) return;     // кириллицу с клавиатуры не трогаем

      e.preventDefault();
      const ch = e.key === lower ? rep : rep.toUpperCase();
      if (t.isContentEditable){ document.execCommand('insertText', false, ch); return; }
      const s = t.selectionStart, en = t.selectionEnd;
      t.setRangeText(ch, s, en, 'end');
      t.dispatchEvent(new Event('input', { bubbles:true }));
    }, true);
  }
};
/* Каталог — в общий список: у каждой раскладки есть имя, флаг и код */
Layouts.КАТАЛОГ.forEach(([id, имя, флаг, en]) => {
  if (!Layouts.ALL[id]) Layouts.ALL[id] = { code:id.toUpperCase().slice(0, 3), name:имя, flag:флаг, map:null };
  Layouts.ALL[id].en = en;
});
window.Layouts = Layouts;

Layouts.paint();
Layouts.install();
