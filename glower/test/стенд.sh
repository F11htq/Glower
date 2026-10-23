#!/bin/bash
# ==========================================================================
# Живой стенд: рабочий стол под настоящим оконным сервером
#
# Две попытки разделить поверхности провалились вслепую. Оболочку было не на
# чем запустить: образ уезжал человеку, а обратно приходило описание экрана.
# Ошибки вроде «селектор не тот» или «панель рисует обои второй раз» так не
# находятся — они не падают, они просто выглядят неправильно.
#
# Здесь рабочий стол поднимается по-настоящему: labwc без экрана, агент и
# glower-panel. Снимок делается grim, и его видно.
#
# Имена переменных латиницей: кириллические bash не принимает. Проверено
# дорого и не один раз.
#
#   bash glower/test/стенд.sh          — поднять
#   bash glower/test/стенд.sh снимок   — поднять и снять экран
# ==========================================================================
set -u

RT=${GLOWER_RT:-/tmp/glower-rt}
SRC=$(cd "$(dirname "$0")/.." && pwd)
TST=/tmp/glower-test/glower
command -v labwc >/dev/null || { echo "нет labwc — ставьте labwc"; exit 1; }

mkdir -p "$RT"; chmod 700 "$RT"
export XDG_RUNTIME_DIR="$RT"
export WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman
export LIBGL_ALWAYS_SOFTWARE=1

# Один только файл сокета ничего не значит: он остаётся лежать и после того,
# как сервер умер. Стенд на это попался — сокет от прошлого раза был на
# месте, labwc не запускался, а оболочка падала на пустом Gdk.Screen, и со
# стороны это выглядело как её собственная поломка. Спрашиваем про сервер.
if ! pgrep -x labwc >/dev/null 2>&1; then
  rm -f "$RT/wayland-0" "$RT/wayland-0.lock"
fi
if [ ! -S "$RT/wayland-0" ]; then
  # С боевыми настройками, а не с чужими умолчаниями. Стенд нужен, чтобы
  # видеть то же, что увидит человек; сервер с другими настройками — это
  # уже другая система, и проверка на ней стоит меньше, чем кажется.
  nohup labwc -C "$SRC/linux/labwc" > "$RT/labwc.log" 2>&1 &
  for i in $(seq 1 40); do [ -S "$RT/wayland-0" ] && break; sleep 0.25; done
fi
export WAYLAND_DISPLAY=wayland-0 GDK_BACKEND=wayland
# Размер экрана берём как у машины, на которой всё это проверяется:
# на другом разрешении раскладка панели и дока может лечь иначе.
command -v wlr-randr >/dev/null && \
  wlr-randr --output HEADLESS-1 --custom-mode "${GLOWER_SIZE:-1366x768}" >/dev/null 2>&1
[ -S "$RT/wayland-0" ] || { echo "labwc не поднялся"; tail -5 "$RT/labwc.log"; exit 1; }

pkill -x glower-panel >/dev/null 2>&1
pkill -f 'glower-test/glower/agent' >/dev/null 2>&1
sleep 1

rm -rf /tmp/glower-test && mkdir -p /tmp/glower-test
cp -r "$SRC" "$TST"
# Стенду нужно проскочить первоначальную настройку — своими настройками,
# в своей папке, а не в настоящих
mkdir -p "$RT/config/glower"
printf '{"setupDone": true}\n' > "$RT/config/glower/settings.json"

# С системным слоем — иначе стенд показывает не ту систему, что у человека.
# Без него панель честно пишет «нет на машине» про Wi-Fi, Bluetooth, звук и
# заряд, и проверять на таком стенде ровно эти вещи невозможно.
# Ключи — те же, что у настоящего сеанса. Урезанный набор показывает не ту
# систему: Магазин честно пишет «установка выключена», лоток пуст, и
# проверять на таком стенде ровно эти вещи негде.
GLOWER_NO_AUTO_WINE=1 nohup node "$TST/agent/server.mjs" --port 8124 --root "$RT/home" \
      --system --allow-open --allow-launch --allow-power \
      --allow-install --allow-net --allow-packages > "$RT/agent.log" 2>&1 &
for i in $(seq 1 40); do curl -sf http://localhost:8124/ >/dev/null && break; sleep 0.3; done

PANEL="$SRC/panel/target/release/glower-panel"
[ -x "$PANEL" ] || (cd "$SRC/panel" && cargo build --release --locked) || { echo "рабочий стол не собрался"; exit 1; }
XDG_CONFIG_HOME="$RT/config" GLOWER_ROOT="$RT/home" GLOWER_URL=http://localhost:8124/ \
  nohup "$PANEL" > "$RT/shell.log" 2>&1 &
sleep 6

if pgrep -x glower-panel >/dev/null; then
  echo "рабочий стол жив"
else
  echo "рабочий стол упал:"; tail -15 "$RT/shell.log"; exit 1
fi

if [ "${1:-}" = "снимок" ]; then
  out=${2:-$RT/снимок.png}
  grim "$out" && echo "снято: $out"
fi
