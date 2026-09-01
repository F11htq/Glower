/* ==========================================================================
   Вход в систему

   Экран входа — это отдельная маленькая жизнь: своего рабочего стола ещё
   нет, есть только список людей и вопрос «кто вы и какой у вас пароль».
   Проверяет пароль и заводит сеанс не оболочка, а greetd — служба, которая
   для этого и существует: она говорит с PAM, заводит сеанс со всеми
   правами и запускает рабочий стол от имени человека.

   Здесь — только разговор с ней. Протокол простой: длина сообщения четырьмя
   байтами, затем JSON. Пароль проходит через нас и нигде не остаётся.
   ========================================================================== */
import { readFile } from 'node:fs/promises';
import { connect } from 'node:net';

/* Люди системы: те же, кого показывает раздел «Люди в системе» */
async function людиСистемы(){
  const текст = await readFile('/etc/passwd', 'utf8').catch(() => '');
  const list = [];
  for (const с of String(текст).split('\n')){
    const ч = с.split(':');
    if (ч.length < 7) continue;
    const uid = Number(ч[2]);
    if (!(uid >= 1000 && uid < 65000)) continue;
    if (/nologin|\/false$/.test(ч[6] || '')) continue;
    list.push({ имя:ч[0], полное:(ч[4] || '').split(',')[0] || ч[0], uid, дом:ч[5] });
  }
  list.sort((a, b) => a.uid - b.uid);
  return list;
}

/* Один разговор с greetd: посылаем сообщение, получаем ответ */
function скажи(сокет, что){
  return new Promise((готово, беда) => {
    const тело = Buffer.from(JSON.stringify(что), 'utf8');
    const длина = Buffer.alloc(4);
    длина.writeUInt32LE(тело.length, 0);

    let принято = Buffer.alloc(0);
    const наДанные = кусок => {
      принято = Buffer.concat([принято, кусок]);
      if (принято.length < 4) return;
      const н = принято.readUInt32LE(0);
      if (принято.length < 4 + н) return;
      сокет.off('data', наДанные);
      try { готово(JSON.parse(принято.slice(4, 4 + н).toString('utf8'))); }
      catch(e){ беда(new Error('greetd ответил непонятным: ' + e.message)); }
    };
    сокет.on('data', наДанные);
    сокет.write(Buffer.concat([длина, тело]), e => { if (e) беда(e); });
  });
}

export function login(){
  return {
    /* Кого показывать на экране входа */
    async 'login.users'(){
      const list = await людиСистемы();
      return { list, готов:!!process.env.GREETD_SOCK };
    },

    /* Войти: имя и пароль уходят greetd, он проверяет их у PAM и заводит
       настоящий сеанс. Своей проверки здесь нет намеренно — иначе получилось
       бы, что вход разрешает оболочка, а это не её дело. */
    async 'login.enter'({ имя, пароль, сеанс }){
      const сокетПуть = process.env.GREETD_SOCK;
      if (!сокетПуть) throw new Error('это не экран входа: greetd рядом нет');
      if (!/^[a-z_][a-z0-9_-]{0,31}$/.test(String(имя || '')))
        throw new Error('неверное имя пользователя');

      const команда = String(сеанс || '/usr/bin/glower-session');
      if (!/^\/[\w./-]{1,120}$/.test(команда)) throw new Error('неверная команда сеанса');

      const сокет = connect(сокетПуть);
      await new Promise((г, б) => { сокет.once('connect', г); сокет.once('error', б); });

      try {
        let ответ = await скажи(сокет, { type:'create_session', username:String(имя) });

        /* greetd спрашивает столько, сколько нужно PAM: обычно один пароль */
        let шагов = 0;
        while (ответ && ответ.type === 'auth_message' && шагов++ < 5){
          const это_пароль = ответ.auth_message_type === 'secret';
          ответ = await скажи(сокет, { type:'post_auth_message_response',
            response: это_пароль ? String(пароль == null ? '' : пароль) : '' });
        }

        if (!ответ || ответ.type !== 'success'){
          // greetd отвечает и по-человечески (error_type), и по-своему
          // (description: «pam_authenticate: AUTH_ERR»). Человеку у экрана
          // нужен первый ответ, а не внутренности PAM.
          const вид = ответ && ответ.error_type;
          const почему = вид === 'auth_error'
            ? 'Пароль не подошёл'
            : ((ответ && (ответ.description || вид)) || 'Войти не удалось');
          try { await скажи(сокет, { type:'cancel_session' }); } catch(e){}
          сокет.end();
          return { ok:false, почему };
        }

        const пуск = await скажи(сокет, { type:'start_session', cmd:[команда], env:[] });
        сокет.end();
        if (!пуск || пуск.type !== 'success')
          return { ok:false, почему:(пуск && (пуск.description || пуск.error_type)) || 'сеанс не запустился' };
        return { ok:true, имя:String(имя) };
      } catch(e){
        try { сокет.end(); } catch(e2){}
        throw new Error('разговор с greetd не задался: ' + e.message);
      }
    }
  };
}
