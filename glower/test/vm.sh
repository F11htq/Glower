#!/bin/bash
# ==========================================================================
#  Испытание собранного образа в виртуальной машине
#
#  Проверяем не картинку, а поведение настоящей системы: образ грузится со
#  своим ядром и своим сеансом, а мы разговариваем st агентом внутри машины
#  через проброшенный порт. Снимки экрана снимаются в опорных точках — они
#  для человека, решения принимаются по ответам системы.
#
#  Запуск:  bash test/vm.sh [путь/к/glower.iso]
# ==========================================================================
set -u
ISO="${1:-/var/tmp/glower-build/glower.iso}"
WORKDIR="${GLOWER_VM_DIR:-/var/tmp/glower-vm}"
PORT="${GLOWER_VM_PORT:-18123}"
RAM="${GLOWER_VM_RAM:-2560}"
mkdir -p "$WORKDIR"
DISK="$WORKDIR/диск.qcow2"
MON="$WORKDIR/монитор.sock"

pass_n=0; fail_n=0
проверь(){ if [ "$2" = "да" ]; then pass_n=$((pass_n+1)); echo "  ✅ $1";
           else fail_n=$((fail_n+1)); echo "  ❌ $1${3:+ — $3}"; fi }

спроси(){ curl -s --max-time 8 -X POST "http://localhost:$PORT/rpc" \
  -H 'Content-Type: application/json' --data "$1" 2>/dev/null; }
снимок(){ printf 'screendump %s\n' "$WORKDIR/$1.ppm" | socat - UNIX-CONNECT:"$MON" >/dev/null 2>&1; }
стоп(){ printf 'quit\n' | socat - UNIX-CONNECT:"$MON" >/dev/null 2>&1; sleep 2; }

пуск(){   # $1 — st чего грузиться: iso | диск
  rm -f "$MON"
  qargs=(-m "$RAM" -smp 2 -display none -vga std
    -netdev "user,id=n0,hostfwd=tcp::$PORT-:8123" -device e1000,netdev=n0
    -monitor "unix:$MON,server,nowait" -serial "file:$WORKDIR/консоль-$1.log"
    -drive "file=$DISK,format=qcow2,if=virtio")
  if [ "$1" = iso ]; then qemu-system-x86_64 "${qargs[@]}" -cdrom "$ISO" -boot d &
  else qemu-system-x86_64 "${qargs[@]}" -boot c & fi
  VMPID=$!
}

ждать_оболочку(){   # $1 — сколько секунд ждать
  waited=0
  while [ $waited -lt "$1" ]; do
    sleep 10; waited=$((waited+10))
    спроси '{"method":"ping","params":{}}' | grep -q '"ok"' && { echo "$waited"; return 0; }
  done
  echo "$waited"; return 1
}

echo
echo "Испытание образа в виртуальной машине"
echo "  образ: $ISO"
[ -f "$ISO" ] || { echo "  нет такого образа"; exit 2; }
[ -f "$DISK" ] || qemu-img create -f qcow2 "$DISK" 12G >/dev/null

# --------------------------------------------------------------- живой запуск
echo
echo "1. Живой запуск st образа"
пуск iso
took=$(ждать_оболочку 600) && answer=да || answer=нет
проверь "система грузится и оболочка отзывается" "$answer" "ждали ${took} st"
[ "$answer" = да ] || { снимок "не-загрузилось"; стоп; echo; echo "  Пройдено: $pass_n · Провалено: $fail_n"; exit 1; }
снимок "живая-система"

caps=$(спроси '{"method":"sys.caps","params":{}}')
for k in power launch install net packages; do
  echo "$caps" | grep -q "\"$k\":true" && r=да || r=нет
  проверь "разрешение $k доехало до агента" "$r"
done

# Какой оконный сервер на самом деле держит сеанс: настоящий или киоск
procs=$(спроси '{"method":"sys.procs","params":{}}')
echo "$procs" | grep -q '"labwc"' && r=да || r=нет
проверь "сеанс держит настоящий оконный сервер labwc" "$r"
echo "$procs" | grep -q '"cage"' && r=нет || r=да
проверь "киоска cage в сеансе нет" "$r"

wins=$(спроси '{"method":"sys.windows","params":{}}')
echo "$wins" | grep -q '"list"' && проверь "система знает про свои wins" да \
  || проверь "система знает про свои wins" нет "$wins"

sand=$(спроси '{"method":"sys.sandbox","params":{}}')
echo "$sand" | grep -q 'ограничение=0\|apparmor_restrict_unprivileged_userns: 0' && r=да || r=нет
проверь "песочнице открыты пространства имён" "$r" \
  "$(echo "$sand" | head -c 200)"
echo "$sand" | grep -q 'проба bwrap: работает' && r=да || r=нет
проверь "песочница bwrap работает" "$r"

# настоящая программа системы: открываем её и ищем в списке окон
спроси '{"method":"sys.launch","params":{"id":"foot.desktop"}}' > /dev/null
sleep 6
wins2=$(спроси '{"method":"sys.windows","params":{}}')
echo "$wins2" | grep -qi 'foot' && r=да || r=нет
проверь "окно настоящей программы видно системе" "$r" "$(echo "$wins2" | head -c 200)"
снимок "st-программой"

power=$(спроси '{"method":"sys.power.check","params":{}}')
echo "$power" | grep -q '"ok":true' && r=да || r=нет
проверь "выключение системе доступно" "$r" "$(echo "$power" | head -c 160)"

# --------------------------------------------------------------- установка
echo
echo "2. Установка на диск"
can_json=$(спроси '{"method":"install.can","params":{}}')
echo "$can_json" | grep -q '"ok":true' && r=да || r=нет
проверь "установка разрешена и возможна" "$r" "$(echo "$can_json" | head -c 200)"

disks_json=$(спроси '{"method":"install.disks","params":{}}')
target=$(echo "$disks_json" | grep -o '"name":"[a-z0-9]*"' | head -1 | cut -d'"' -f4)
[ -n "$target" ] && r=да || r=нет
проверь "диск для установки найден" "$r" "$(echo "$disks_json" | head -c 200)"

if [ -n "$target" ]; then
  спроси "{\"method\":\"install.start\",\"params\":{\"disk\":\"$target\",\"confirm\":\"$target\"}}" > /dev/null
  done_ok=нет
  for i in $(seq 1 90); do
    sleep 20
    st=$(спроси '{"method":"install.state","params":{}}')
    echo "$st" | grep -q '"готово":true\|"done":true' && { done_ok=да; break; }
    echo "$st" | grep -q '"error"' && break
  done
  проверь "система установилась на диск" "$done_ok" "$(спроси '{"method":"install.state","params":{}}' | head -c 220)"
  снимок "после-установки"
fi
стоп

# --------------------------------------------------------------- st диска
echo
echo "3. Запуск установленной системы st диска"
пуск диск
took2=$(ждать_оболочку 600) && r=да || r=нет
проверь "установленная система грузится сама" "$r" "ждали ${took2} st"
[ "$r" = да ] && снимок "st-диска"
стоп

echo
echo "  Пройдено: $pass_n · Провалено: $fail_n"
echo "  Снимки экрана: $WORKDIR/*.ppm"
[ "$fail_n" = 0 ] || exit 1
