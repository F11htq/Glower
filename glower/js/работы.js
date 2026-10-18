/* ==========================================================================
   Ход работы: плашка справа внизу

   Установка, удаление, обновление — всё, что делает apt или flatpak, —
   показывается одной плашкой в углу экрана: значок и имя программы, что
   сейчас происходит, полоса и кнопка «Отменить». Плашка общая для всей
   системы: её видно, даже если Магазин закрыт, а работу начал не он —
   двойной щелчок по .deb, удаление из меню, Wine или перевод программ,
   которые система докачивает сама.

   Прежде ход показывал сам Магазин — карточкой над страницей. Он
   перерисовывал страницу целиком раз в секунду, карточки при этом заново
   проигрывали анимацию появления, а карточка хода то появлялась, то
   пропадала и толкала всё вниз: весь интерфейс трясся. Здесь плашка
   создаётся один раз, а дальше у неё меняются только текст и ширина
   полосы.
   ========================================================================== */
'use strict';

const Работы = {
  мета:{},              // имя пакета → { имя, значок } от того, кто начал работу
  узел:null,
  шла:false,            // в прошлый раз работа шла — значит, сейчас могла кончиться
  последняя:null,
  таймер:null,
  скрыть:null,

  /* Кто начал работу, тот и знает, как она называется по-человечески */
  следи(пакет, имя, значок){
    if (пакет) this.мета[пакет] = { имя:имя || пакет, значок:значок || '' };
    this.чаще();
  },

  /* Как назвать работу, если начавший о ней не рассказал */
  имя(j){
    const м = this.мета[j.name];
    if (м && м.имя) return м.имя;
    if (j.name === 'wine') return 'Поддержка программ Windows';
    if (j.name === 'система') return 'Обновление системы';
    if (/^language-pack-/.test(j.name || '')) return 'Перевод программ';
    if (j.action === 'update') return 'Списки программ';
    return j.name || 'Программа';
  },
  значок(j){
    const м = this.мета[j.name];
    if (м && м.значок) return м.значок;
    if (j.name === 'wine') return '🪟';
    if (j.name === 'система') return '✨';
    if (/^language-pack-/.test(j.name || '')) return '🌐';
    return j.source === 'flatpak' ? '🫙' : '📦';
  },
  действие(j){
    return { install:'Установка', remove:'Удаление', update:'Обновление списков',
             upgrade:'Обновление' }[j.action] || 'Работа';
  },

  /* Агента платформа находит не сразу — пробуем несколько раз */
  init(попытка = 0){
    if (!window.Platform || Platform.mode !== 'native'){
      if (попытка < 15) setTimeout(() => this.init(попытка + 1), 1000);
      return;
    }
    this.чаще(500);
  },

  /* Опрос: часто, пока работа идёт, и редко, когда тихо — чтобы заметить
     работу, начатую не нами (двойной щелчок по .deb, фоновая докачка). */
  чаще(через = 300){
    clearTimeout(this.таймер);
    this.таймер = setTimeout(() => this.шаг(), через);
  },
  async шаг(){
    let j = null;
    try { j = await Platform.rpc('pkg.job'); } catch(e){}
    if (j && j.running){
      this.шла = true; this.последняя = j;
      this.рисуй(j);
      return this.чаще(1000);
    }
    if (this.шла && j){ this.шла = false; this.конец(j); }
    this.чаще(4000);
  },

  /* Плашку строим один раз, дальше меняем только текст и полосу */
  сделай(){
    if (this.узел && this.узел.isConnected) return this.узел;
    const место = document.getElementById('toasts') || document.body;
    const н = el('div', 'работа');
    н.innerHTML = `
      <div class="работа-зн"></div>
      <div class="работа-текст"><b class="работа-имя"></b><small class="работа-шаг"></small></div>
      <button class="работа-отмена" title="Отменить">✕</button>
      <div class="работа-полоса"><i></i></div>`;
    н.querySelector('.работа-отмена').onclick = () => this.отмени();
    место.appendChild(н);
    requestAnimationFrame(() => н.classList.add('on'));
    this.узел = н;
    return н;
  },

  рисуй(j){
    clearTimeout(this.скрыть);
    const н = this.сделай();
    н.classList.remove('готово', 'ошибка', 'уходит');
    const зн = this.значок(j);
    const место = н.querySelector('.работа-зн');
    if (место.dataset.зн !== зн){ место.dataset.зн = зн; место.innerHTML = /^(data:|https?:)/.test(зн) ? `<img src="${esc(зн)}" alt="">` : esc(зн); }
    н.querySelector('.работа-имя').textContent = this.имя(j);
    const п = Math.max(0, Math.min(100, Math.round(j.percent || 0)));
    const точно = п > 2;
    const молчит = j.молчит > 60 ? ' · ждёт сеть' : '';
    н.querySelector('.работа-шаг').textContent =
      this.действие(j) + (j.step && j.step !== 'Начинаю' ? ' · ' + j.step : '') + (точно ? ' · ' + п + '%' : '') + молчит;
    н.classList.toggle('неточно', !точно);
    н.querySelector('.работа-полоса i').style.width = (точно ? п : 100) + '%';
    const отмена = н.querySelector('.работа-отмена');
    отмена.hidden = false; отмена.textContent = '✕'; отмена.title = 'Отменить';
  },

  конец(j){
    const н = this.сделай();
    const прежняя = this.последняя || j;
    н.classList.remove('неточно');
    н.classList.add(j.ok ? 'готово' : 'ошибка');
    н.querySelector('.работа-имя').textContent = this.имя(прежняя);
    н.querySelector('.работа-полоса i').style.width = '100%';
    н.querySelector('.работа-зн').innerHTML = j.ok ? '✓' : '!';
    н.querySelector('.работа-зн').dataset.зн = '';
    const итог = j.ok
      ? ({ install:'Установлено', remove:'Удалено', update:'Списки обновлены', upgrade:'Обновлено' }[прежняя.action] || 'Готово')
        + (j['заметка'] ? ' · ' + j['заметка'] : '')
      : (j.error === 'работа остановлена' ? 'Отменено' : 'Не вышло: ' + (j.error || 'причина неизвестна'));
    н.querySelector('.работа-шаг').textContent = итог;
    const кн = н.querySelector('.работа-отмена');
    кн.textContent = '✕'; кн.title = 'Закрыть';
    кн.onclick = () => this.убери();
    /* Удачное — уходит само; ошибка остаётся, пока её не закроют */
    if (j.ok || j.error === 'работа остановлена') this.скрыть = setTimeout(() => this.убери(), 5000);
    /* Список программ мог измениться — пусть меню узнает */
    if (j.ok && typeof обновиСписокМашины === 'function') обновиСписокМашины();
  },

  убери(){
    const н = this.узел;
    if (!н) return;
    н.classList.add('уходит');
    setTimeout(() => { if (н.classList.contains('уходит')){ н.remove(); if (this.узел === н) this.узел = null; } }, 350);
  },

  async отмени(){
    const j = this.последняя;
    if (!await Dlg.confirm('Отменить ' + (j ? this.действие(j).toLowerCase() : 'работу') + '?',
        'Работа будет прервана, а система приведена в порядок — недоделанного не останется.',
        { icon:'✕', okText:'Отменить', cancelText:'Продолжить', danger:true })) return;
    try { await Platform.rpc('pkg.cancel'); this.чаще(200); }
    catch(e){ Dlg.alert('Не удалось отменить', String(e.message || e), '⚠️'); }
  }
};
window.Работы = Работы;

/* Платформа узнаёт, есть ли агент, не сразу — подождём её */
addEventListener('load', () => setTimeout(() => Работы.init(), 1500));
