/* ==========================================================================
   Движок браузера GlowerOS

   В окне системы должен показываться настоящий сайт, а не пересказ. Поэтому
   рядом с оболочкой поднимается второй Chromium без своего интерфейса: он
   рисует страницы, а оболочка забирает у него кадры и шлёт обратно мышь и
   клавиатуру по отладочному протоколу. Правила те же, что и у остального
   системного слоя:
     — запускается один заранее известный исполняемый файл, без строк оболочки;
     — слушает только 127.0.0.1, порт выбирает ядро, а не мы;
     — гасится вместе с агентом, чтобы не оставлять процессов за собой.
   ========================================================================== */
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { promisify } from 'node:util';
import { execFile } from 'node:child_process';

const run = promisify(execFile);

async function findBrowser(){
  if (process.env.GLOWER_BROWSER && existsSync(process.env.GLOWER_BROWSER))
    return process.env.GLOWER_BROWSER;
  for (const name of ['chromium', 'chromium-browser', 'google-chrome', 'google-chrome-stable']){
    try { const { stdout } = await run('which', [name]); const p = stdout.trim(); if (p) return p; }
    catch(e){}
  }
  return null;
}

/* Chromium сам записывает выбранный порт в файл — так надёжнее, чем гадать */
async function waitPort(dir, proc){
  const file = join(dir, 'DevToolsActivePort');
  for (let i = 0; i < 100; i++){
    if (proc.exitCode != null) throw new Error('движок завершился, код ' + proc.exitCode);
    try {
      const [port, path] = (await readFile(file, 'utf8')).trim().split('\n');
      if (port && path) return { port:+port, path };
    } catch(e){}
    await new Promise(r => setTimeout(r, 100));
  }
  throw new Error('движок не сообщил порт за 10 секунд');
}

export function browser(uiPort){
  let proc = null, dir = null, state = null, lastErr = '', exited = null;

  const stop = async () => {
    if (proc){ try { proc.kill('SIGTERM'); } catch(e){} }
    if (dir) await rm(dir, { recursive:true, force:true }).catch(() => {});
    proc = null; dir = null; state = null;
  };
  /* движок не должен пережить агента ни при каком способе завершения */
  const bury = () => { if (proc) try { proc.kill('SIGKILL'); } catch(e){} };
  process.on('exit', bury);
  ['SIGTERM', 'SIGINT', 'SIGHUP'].forEach(sig =>
    process.on(sig, () => { bury(); process.exit(0); }));

  return {
    async 'browser.start'(){
      if (state) return state;
      const exe = await findBrowser();
      if (!exe) throw new Error('на машине нет ни chromium, ни google-chrome — движку браузера неоткуда взяться');

      dir = await mkdtemp(join(tmpdir(), 'glower-web-'));
      const args = [
        '--headless=new',
        '--remote-debugging-port=0',
        /* к отладочному порту обращается сама оболочка, поэтому её адрес разрешаем явно */
        '--remote-allow-origins=http://localhost:' + uiPort + ',http://127.0.0.1:' + uiPort,
        '--user-data-dir=' + dir,
        '--no-first-run', '--no-default-browser-check', '--disable-search-engine-choice-screen',
        '--disable-sync', '--password-store=basic', '--use-mock-keychain',
        '--disable-features=Translate,TranslateUI,MediaRouter',
        '--window-size=1280,800', '--hide-scrollbars=false',
        /* Движок рисует страницы без экрана, ускорение ему не нужно: на
           машинах без настоящей видеокарты попытка его использовать даёт
           пустые кадры — то самое белое окно вместо сайта. */
        '--disable-gpu',
        /* В живой системе /dev/shm мал, и отрисовщик из-за этого падает */
        '--disable-dev-shm-usage'
      ];
      /* от имени root Chromium не стартует без этого ключа */
      if (process.getuid && process.getuid() === 0) args.push('--no-sandbox');

      proc = spawn(exe, args, { stdio:['ignore', 'ignore', 'pipe'] });
      let err = '';
      proc.stderr.on('data', d => { err = (err + d).slice(-2000); lastErr = err; });
      proc.on('exit', code => { exited = code; proc = null; state = null; });

      try {
        const { port, path } = await waitPort(dir, proc);
        state = { ok:true, exe, port, ws:`ws://127.0.0.1:${port}${path}` };
        return state;
      } catch(e){
        await stop();
        throw new Error(e.message + (err ? ' · ' + err.trim().split('\n').slice(-2).join(' ') : ''));
      }
    },

    async 'browser.state'(){
      /* Оболочке нужно не только «жив или нет», но и что именно сказал
         движок: белое окно без объяснения — худший из возможных ответов. */
      return Object.assign({ ok:false, running:!!proc, exitCode:exited,
        stderr:(lastErr || '').trim().split('\n').slice(-4).join('\n') }, state || {});
    },

    async 'browser.stop'(){ await stop(); return { ok:true }; }
  };
}
