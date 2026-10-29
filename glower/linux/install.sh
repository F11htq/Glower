#!/bin/sh
# ==========================================================================
#  Установка GlowerOS как сеанса рабочего стола на уже работающий Linux.
#
#  Что делает:
#    1. кладёт агент в /usr/share/glower/ui, а рабочий стол (glower-panel,
#       собранный из panel/) и его программы — в /usr/bin
#    2. ставит скрипт сеанса /usr/bin/glower-session
#    3. регистрирует сеанс в списке дисплей-менеджера
#    4. по желанию включает автозапуск агента через systemd --user
#
#  Что НЕ делает: не трогает загрузчик, не удаляет ваш рабочий стол,
#  не заменяет системные компоненты. Выйти из GlowerOS — выбрать другой
#  сеанс на экране входа.
# ==========================================================================
set -e

PREFIX="${PREFIX:-/usr}"
SHARE="$PREFIX/share/glower/ui"
SRC="$(cd "$(dirname "$0")/.." && pwd)"

say(){ printf '  %s\n' "$1"; }

[ "$(id -u)" = "0" ] || { echo "Запустите с правами root: sudo sh linux/install.sh"; exit 1; }

echo
echo "  Установка GlowerOS"
echo

# --- зависимости ---
MISSING=""
for c in node curl labwc; do command -v "$c" >/dev/null 2>&1 || MISSING="$MISSING $c"; done
PANEL_BIN="$SRC/panel/target/release/glower-panel"
[ -x "$PANEL_BIN" ] || MISSING="$MISSING glower-panel"
if [ -n "$MISSING" ]; then
  say "не хватает:$MISSING"
  say "поставьте их и повторите, например:"
  say "  apt install nodejs curl labwc wtype wlsunset swayidle libgtk-layer-shell0   (Debian, Ubuntu)"
  say "рабочий стол собирается так:  cd panel && cargo build --release --locked"
  exit 1
fi

# --- агент и рабочий стол ---
install -d "$SHARE"
for d in agent assets linux; do
  [ -d "$SRC/$d" ] && cp -r "$SRC/$d" "$SHARE/"
done
say "агент: $SHARE"
install -d "$PREFIX/bin" "$PREFIX/share/glower/labwc"
install -m 755 "$PANEL_BIN" "$PREFIX/bin/glower-panel"
for prog in glower-lock glower-login glower-notepad glower-calc glower-files glower-settings \
            glower-store glower-taskmgr glower-photos glower-snip glower-installer glower-music \
            glower-paint glower-calendar glower-clock glower-todo glower-props glower-scan; do
  ln -sf glower-panel "$PREFIX/bin/$prog"
done
for prog in glower-menu glower-desk glower-shot glower-open-package glower-exe glower-tray; do
  install -m 755 "$SRC/linux/$prog" "$PREFIX/bin/$prog"
done
# Свой оконный сервер — если собран (cd wm && cargo build --release --locked)
[ -x "$SRC/wm/target/release/glower-wm" ] && install -m 755 "$SRC/wm/target/release/glower-wm" "$PREFIX/bin/glower-wm"
install -m 644 "$SRC/linux/labwc/rc.xml" "$PREFIX/share/glower/labwc/rc.xml"
say "рабочий стол: $PREFIX/bin/glower-panel"

# --- сеанс ---
install -m 755 "$SRC/linux/glower-session" "$PREFIX/bin/glower-session"
install -d "$PREFIX/share/wayland-sessions" "$PREFIX/share/xsessions"
install -m 644 "$SRC/linux/glower.desktop" "$PREFIX/share/wayland-sessions/glower.desktop"
install -m 644 "$SRC/linux/glower.desktop" "$PREFIX/share/xsessions/glower.desktop"
say "сеанс: доступен на экране входа как «GlowerOS»"

# --- служба агента (по желанию) ---
install -d "$PREFIX/lib/systemd/user"
install -m 644 "$SRC/linux/glower-agent.service" "$PREFIX/lib/systemd/user/glower-agent.service"
say "служба: systemctl --user enable --now glower-agent"

echo
say "Готово. Выйдите из сеанса и выберите GlowerOS на экране входа."
say "Или просто запустите:  glower-session"
echo
