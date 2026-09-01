/* ==========================================================================
   Экран входа

   Появляется до всякого рабочего стола: система уже загрузилась, но ещё не
   знает, кто за ней сидит. Показывает людей этой машины, спрашивает пароль
   и передаёт ответ службе входа — она проверяет пароль у самой системы и
   заводит настоящий сеанс.

   Свои проверки здесь были бы обманом: оболочка не может и не должна решать,
   пускать человека или нет. Её дело — спросить и показать ответ.
   ========================================================================== */
(function(){
  const это_вход = new URLSearchParams(location.search).get('login') === '1';
  if (!это_вход) return;

  document.documentElement.classList.add('вход-в-систему');

  const рпц = (метод, доводы) => Platform.rpc(метод, доводы || {});

  const экран = document.createElement('div');
  экран.className = 'вход';
  экран.innerHTML = `
    <div class="вход-часы"><div class="вход-время"></div><div class="вход-дата"></div></div>
    <div class="вход-окно">
      <div class="вход-лого"></div>
      <div class="вход-имя"></div>
      <form class="вход-строка" autocomplete="off">
        <input class="вход-пароль" type="password" placeholder="Пароль" autocomplete="off">
        <button class="вход-да" type="submit" title="Войти">→</button>
      </form>
      <div class="вход-беда"></div>
      <div class="вход-люди"></div>
    </div>
    <div class="вход-низ">
      <button class="вход-кнопка" data-что="reboot" title="Перезагрузить" aria-label="Перезагрузить">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M20 12a8 8 0 1 1-2.34-5.66"/>
          <path d="M20 4v4.4h-4.4"/>
        </svg>
      </button>
      <button class="вход-кнопка" data-что="poweroff" title="Выключить" aria-label="Выключить">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M12 3.6v8"/>
          <path d="M7.1 6.6a7.4 7.4 0 1 0 9.8 0"/>
        </svg>
      </button>
    </div>`;
  document.body.appendChild(экран);

  const $ = с => экран.querySelector(с);
  const поле = $('.вход-пароль');
  const беда = $('.вход-беда');
  const людиМесто = $('.вход-люди');
  let люди = [], выбран = null, занят = false;

  /* Часы: человеку полезно видеть время ещё до входа */
  const часы = () => {
    const т = new Date();
    $('.вход-время').textContent = т.toLocaleTimeString('ru-RU', { hour:'2-digit', minute:'2-digit' });
    $('.вход-дата').textContent = т.toLocaleDateString('ru-RU',
      { weekday:'long', day:'numeric', month:'long' });
  };
  часы(); setInterval(часы, 10000);

  const покажиЧеловека = ч => {
    выбран = ч;
    $('.вход-имя').textContent = ч ? (ч['полное'] || ч['имя']) : 'Никого нет';
    $('.вход-лого').textContent = ч ? (ч['полное'] || ч['имя']).trim().charAt(0).toUpperCase() : '?';
    поле.value = '';
    беда.textContent = '';
    setTimeout(() => поле.focus(), 50);
    [...людиМесто.children].forEach(к => к.classList.toggle('он', к.dataset.имя === (ч && ч['имя'])));
  };

  const нарисуйЛюдей = () => {
    людиМесто.innerHTML = '';
    if (люди.length < 2) return;                 // одному человеку выбирать не из кого
    люди.forEach(ч => {
      const к = document.createElement('button');
      к.className = 'вход-человек';
      к.dataset.имя = ч['имя'];
      к.textContent = ч['полное'] || ч['имя'];
      к.onclick = () => покажиЧеловека(ч);
      людиМесто.appendChild(к);
    });
  };

  const войти = async () => {
    if (занят || !выбран) return;
    занят = true;
    беда.textContent = 'Проверяю…';
    try {
      const о = await рпц('login.enter', { 'имя':выбран['имя'], 'пароль':поле.value });
      if (о && о.ok){ беда.textContent = 'Входим…'; return; }   // сеанс запущен, экран сейчас исчезнет
      беда.textContent = (о && о['почему']) || 'Войти не удалось';
      поле.value = '';
      поле.focus();
    } catch(e){
      беда.textContent = String(e.message || e);
    }
    занят = false;
  };

  $('.вход-строка').onsubmit = e => { e.preventDefault(); войти(); };

  экран.querySelectorAll('.вход-кнопка').forEach(к => {
    к.onclick = async () => {
      try { await рпц('sys.power', { action:к.dataset.что }); }
      catch(e){ беда.textContent = 'Система отказала: ' + (e.message || e); }
    };
  });

  /* Агент поднимается не мгновенно: до того, как оболочка нашла его адрес,
     любой запрос уходил в никуда, и человек видел «Не удалось получить
     список людей». Ждём подключения — и только потом спрашиваем. */
  const дождись = async () => {
    try { await Platform.подключение; } catch(e){}
    for (let i = 0; i < 40 && !Platform.url; i++)
      await new Promise(r => setTimeout(r, 250));
    if (!Platform.url) throw new Error('Система не отвечает');
  };

  дождись().then(() => рпц('login.users')).then(d => {
    люди = (d && d.list) || [];
    нарисуйЛюдей();
    покажиЧеловека(люди[0] || null);
    if (d && d.готов === false)
      беда.textContent = 'Служба входа не отвечает — сеанс не запустится';
  }).catch(e => {
    беда.textContent = 'Не удалось получить список людей: ' + (e.message || e);
  });
})();
