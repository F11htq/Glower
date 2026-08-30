#!/bin/bash
# ==========================================================================
#  Испытание образа так, как он ведёт себя на старой машине
#
#  Ядру запрещено управлять видеокартой (пункт меню «безопасная графика»).
#  Значит узла /dev/dri нет, Wayland не поднимется — и система обязана
#  показать рабочий стол через X-сервер, а не погаснуть чёрным экраном.
#
#  Запуск:  bash test/vm-старая-машина.sh [путь/к/glower.iso]
# ==========================================================================
set -u
ISO="${1:-/var/tmp/glower-build/glower.iso}"
DIR="${GLOWER_VM_DIR:-/var/tmp/glower-vm-старьё}"
PORT="${GLOWER_VM_PORT:-18291}"
rm -rf "$DIR"; mkdir -p "$DIR"

pass_n=0; fail_n=0
проверь(){ if [ "$2" = "да" ]; then pass_n=$((pass_n+1)); echo "  ✅ $1";
           else fail_n=$((fail_n+1)); echo "  ❌ $1${3:+ — $3}"; fi }
спроси(){ curl -s --max-time 25 -X POST "http://localhost:$PORT/rpc" \
  -H 'Content-Type: application/json' --data "$1" 2>/dev/null; }
монитор(){ printf '%s\n' "$1" | socat - UNIX-CONNECT:"$DIR/mon.sock" >/dev/null 2>&1; }

echo "Испытание образа как на старой машине"
echo "  образ: $ISO"
echo

qemu-system-x86_64 -m 3072 -smp 2 -display none -vga std \
  -netdev "user,id=n0,hostfwd=tcp::$PORT-:8123" -device e1000,netdev=n0 \
  -monitor "unix:$DIR/mon.sock,server,nowait" \
  -cdrom "$ISO" -boot d > "$DIR/qemu.log" 2>&1 &

# Меню держится восемь секунд: выбираем второй пункт — безопасную графику
sleep 12
монитор 'sendkey down'
sleep 1
монитор 'sendkey ret'

waited=0
while [ $waited -lt 600 ]; do
  sleep 15; waited=$((waited+15))
  спроси '{"method":"ping","params":{}}' | grep -q '"ok"' && break
done
[ $waited -lt 600 ] && r=да || r=нет
проверь "система грузится без управления видеокартой" "$r" "ждали ${waited} с"

sleep 45
procs=$(спроси '{"method":"sys.procs","params":{}}')
echo "$procs" | grep -q '"name":"Xorg"' && r=да || r=нет
проверь "рабочий стол показывает X-сервер" "$r"
echo "$procs" | grep -q 'WebKit' && r=да || r=нет
проверь "оболочку показывает своя программа на WebKit" "$r"
echo "$procs" | grep -q '"name":"openbox"' && r=да || r=нет
проверь "чужими окнами распоряжается оконный менеджер" "$r"

wins=$(спроси '{"method":"sys.windows","params":{}}')
echo "$wins" | grep -q '"через":"x"' && r=да || r=нет
проверь "панель задач читает окна через X" "$r" "$(echo "$wins" | head -c 160)"
echo "$wins" | grep -q '"оболочка":true' && r=да || r=нет
проверь "рабочий стол не считает себя чужим окном" "$r" "$(echo "$wins" | head -c 160)"

спроси '{"method":"sys.terminal","params":{}}' > /dev/null
sleep 12
wins2=$(спроси '{"method":"sys.windows","params":{}}')
echo "$wins2" | grep -qiE 'terminal|xterm' && r=да || r=нет
проверь "терминал открывается и виден системе" "$r" "$(echo "$wins2" | head -c 220)"

монитор "screendump $DIR/экран.ppm"
sleep 3
монитор 'quit'

echo
echo "  Пройдено: $pass_n · Провалено: $fail_n"
echo "  Снимок экрана: $DIR/экран.ppm"
[ "$fail_n" = 0 ] || exit 1
