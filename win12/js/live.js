/* ==========================================================================
   Живая система: то, что срабатывает само

   Будильники, напоминания календаря и автоблокировка — не настройки ради
   настроек, а таймеры, которые действительно будят систему, пока она
   открыта. Поиск теперь заглядывает и внутрь файлов.
   ========================================================================== */
'use strict';

/* ==========================================================================
   1. Будильники и напоминания
   ========================================================================== */
const Alarms = {
  KEY:'alarms',

  list(){ return KV.get(this.KEY, []); },
  save(l){ KV.set(this.KEY, l); },

  add(a){
    const l = this.list();
    l.push({ id:'a' + Date.now().toString(36), h:7, m:0, days:[], on:true, label:'Будильник', ...a });
    this.save(l);
    return l[l.length - 1];
  },
  remove(id){ this.save(this.list().filter(a => a.id !== id)); },
  update(id, patch){
    const l = this.list();
    const a = l.find(x => x.id === id);
    if (a) Object.assign(a, patch);
    this.save(l);
  },

  /* когда сработает в следующий раз — для подписи «через 7 ч 20 мин» */
  next(a){
    const now = new Date();
    for (let d = 0; d < 8; d++){
      const t = new Date(now.getFullYear(), now.getMonth(), now.getDate() + d, a.h, a.m, 0, 0);
      if (t <= now) continue;
      const dow = (t.getDay() + 6) % 7;             // 0 — понедельник
      if (a.days.length && !a.days.includes(dow)) continue;
      return t;
    }
    return null;
  },

  fire(a){
    Snd.note();
    const t = `${pad2(a.h)}:${pad2(a.m)}`;
    Shell.toast('Будильник', `${t} · ${a.label}`, '⏰', 12000);
    Dlg.alert('Будильник · ' + t, a.label || 'Пора!', '⏰');
    if (!a.days.length) this.update(a.id, { on:false });   // разовый — выключается сам
  },

  tick(){
    const now = new Date();
    const key = `${now.getFullYear()}-${now.getMonth()}-${now.getDate()}-${now.getHours()}-${now.getMinutes()}`;
    const dow = (now.getDay() + 6) % 7;
    this.list().forEach(a => {
      if (!a.on || a.lastFired === key) return;
      if (a.h !== now.getHours() || a.m !== now.getMinutes()) return;
      if (a.days.length && !a.days.includes(dow)) return;
      this.update(a.id, { lastFired:key });
      this.fire(a);
    });
  }
};
window.Alarms = Alarms;

/* ---------- напоминания календаря ---------- */
const Reminders = {
  /* событие может быть строкой (старый формат) или { t, time } */
  norm(v){ return typeof v === 'string' ? { t:v, time:'' } : (v || { t:'', time:'' }); },

  tick(){
    const now = new Date();
    const key = `${now.getFullYear()}-${now.getMonth()}-${now.getDate()}`;
    const hm = `${pad2(now.getHours())}:${pad2(now.getMinutes())}`;
    const events = KV.get('cal.events', {});
    const ev = this.norm(events[key]);
    if (!ev.t || !ev.time || ev.time !== hm) return;
    const fired = KV.get('cal.fired', {});
    if (fired[key + ' ' + hm]) return;
    fired[key + ' ' + hm] = 1;
    KV.set('cal.fired', fired);
    Snd.note();
    Shell.toast('Календарь', `${ev.time} · ${ev.t}`, '📅', 12000);
  }
};
window.Reminders = Reminders;

setInterval(() => { Alarms.tick(); Reminders.tick(); }, 15000);

/* ==========================================================================
   2. Автоблокировка по бездействию
   ========================================================================== */
const AutoLock = {
  last:Date.now(),

  minutes(){ return +KV.get('autoLock', 0); },      // 0 — выключено

  touch(){ this.last = Date.now(); },

  tick(){
    const m = this.minutes();
    if (!m) return;
    if ($('#lock').classList.contains('on')) return;
    if (!$('#desktop').classList.contains('on')) return;
    if (Date.now() - this.last < m * 60000) return;
    this.touch();
    Shell.closePanels();
    Profiles.lock();
  },

  install(){
    ['pointerdown', 'keydown', 'wheel', 'pointermove'].forEach(t =>
      addEventListener(t, () => this.touch(), { passive:true }));
    setInterval(() => this.tick(), 10000);
  }
};
window.AutoLock = AutoLock;
AutoLock.install();

/* ==========================================================================
   3. Поиск внутри файлов
   ========================================================================== */
const Search = {
  /* совпадение по имени или по содержимому текстового файла */
  scan(q){
    const ql = q.toLowerCase();
    const res = [];
    (function walk(node, p){
      Object.values(node.children || {}).forEach(c => {
        const byName = c.name.toLowerCase().includes(ql);
        let hit = '';
        if (!byName && c.type === 'file' && typeof c.body === 'string' && c.body){
          const i = c.body.toLowerCase().indexOf(ql);
          if (i >= 0) hit = c.body.slice(Math.max(0, i - 30), i + q.length + 40).replace(/\s+/g, ' ').trim();
        }
        if (byName || hit) res.push({ node:c, path:p, hit });
        if (c.type === 'dir') walk(c, [...p, c.name]);
      });
    })(FS.root, []);
    return res;
  }
};
window.Search = Search;

/* ==========================================================================
   4. Список будильников — рисуется вкладкой приложения «Часы»
   ========================================================================== */
Alarms.DOW = ['Пн','Вт','Ср','Чт','Пт','Сб','Вс'];

Alarms.ui = function(body){
  const draw = () => {
    body.innerHTML = '';
    const list = this.list();

    const add = el('button', 'btn pri', '+ Новый будильник');
    add.onclick = () => { this.add({ h:new Date().getHours(), m:0 }); draw(); };
    body.appendChild(add);

    if (!list.length) body.appendChild(el('div', 'empty', 'Будильников нет'));

    list.forEach(a => {
      const c = el('div', 'card alarm-card');

      const time = el('input', 'inp alarm-time');
      time.type = 'time';
      time.lang = window.I18N ? I18N.locale() : 'ru-RU';   // 24-часовой вид для русского
      time.value = `${pad2(a.h)}:${pad2(a.m)}`;
      time.onchange = () => {
        const [h, m] = time.value.split(':').map(Number);
        this.update(a.id, { h, m, lastFired:'' }); draw();
      };

      const label = el('input', 'inp');
      label.value = a.label;
      label.placeholder = 'Название';
      label.onchange = () => this.update(a.id, { label:label.value });

      const on = el('button', 'btn' + (a.on ? ' pri' : ''), a.on ? 'Вкл' : 'Выкл');
      on.onclick = () => { this.update(a.id, { on:!a.on, lastFired:'' }); draw(); };

      const del = el('button', 'btn', 'Удалить');
      del.onclick = () => { this.remove(a.id); draw(); };

      const days = el('div', 'row alarm-days');
      this.DOW.forEach((d, i) => {
        const db = el('button', 'btn' + (a.days.includes(i) ? ' pri' : ''), d);
        db.onclick = () => {
          const set = a.days.includes(i) ? a.days.filter(x => x !== i) : [...a.days, i];
          this.update(a.id, { days:set.sort() }); draw();
        };
        days.appendChild(db);
      });

      const nx = this.next(a);
      const when = !a.on ? 'выключен'
        : nx ? 'сработает ' + nx.toLocaleString('ru-RU', { weekday:'short', hour:'2-digit', minute:'2-digit' })
             : 'дни недели не выбраны';

      const top = el('div', 'row'); top.append(time, label, on, del);
      c.append(top, days, el('div', 'muted tiny', when));
      body.appendChild(c);
    });

    body.appendChild(el('div', 'muted tiny',
      'Будильник звонит, пока система открыта: закрытую вкладку страница разбудить не может.'));
  };
  draw();
};
