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

# Часть вопросов система обдумывает подолгу: осмотр песочницы делает пробный
# запуск, первый значок обходит тысячи файлов. Восьми секунд им не хватало, и
# молчание выглядело как поломка — ждём дольше.
спроси(){ curl -s --max-time 30 -X POST "http://localhost:$PORT/rpc" \
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

# Агент отвечает раньше, чем поднимается сеанс: сперва дожидаемся оболочки,
# иначе спрашивать про оконный сервер и WebKit бессмысленно — их ещё нет.
дождись(){   # $1 — что ищем в списке процессов, $2 — сколько секунд ждать
  waited_p=0
  while [ $waited_p -lt "$2" ]; do
    спроси '{"method":"sys.procs","params":{}}' | grep -qi "$1" && return 0
    sleep 10; waited_p=$((waited_p+10))
  done
  return 1
}
дождись 'labwc' 240 || true
дождись 'WebKit' 240 || true

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
# Имя ярлыка у настоящей программы своё: берём его из списка программ машины
label=$(спроси '{"method":"sys.apps","params":{}}' | grep -o '"id":"[^"]*foot[^"]*"' | head -1 | cut -d'"' -f4)
[ -n "$label" ] && спроси "{\"method\":\"sys.launch\",\"params\":{\"id\":\"$label\"}}" > /dev/null
sleep 6
wins2=$(спроси '{"method":"sys.windows","params":{}}')
echo "$wins2" | grep -qi 'foot' && r=да || r=нет
проверь "окно настоящей программы видно системе" "$r" "$(echo "$wins2" | head -c 200)"
снимок "st-программой"

# Оболочку должна показывать своя программа на WebKit, а не сборка Chromium:
# в образе её нет, и раньше сеанс из-за этого не поднимался вовсе.
proc=$(спроси '{"method":"sys.procs","params":{}}')
echo "$proc" | grep -qi 'WebKit' && r=да || r=нет
проверь "оболочку показывает своя программа на WebKit" "$r" "$(echo "$proc" | head -c 160)"

# Состояние чужого окна: панель задач должна знать, развёрнуто ли оно
спроси '{"method":"sys.window","params":{"action":"maximize","appId":"foot"}}' > /dev/null
sleep 3
wins3=$(спроси '{"method":"sys.windows","params":{}}')
echo "$wins3" | grep -q '"развёрнуто":true' && r=да || r=нет
проверь "система видит развёрнутое чужое окно" "$r" "$(echo "$wins3" | head -c 200)"
echo "$wins3" | grep -q '"занятЭкран":true' && r=да || r=нет
проверь "панель знает, что экран занят окном" "$r" "$(echo "$wins3" | head -c 200)"

# Браузер и типы файлов: нажатая ссылка должна открываться, а не пропадать
apps_json=$(спроси '{"method":"sys.apps","params":{}}')
echo "$apps_json" | grep -qiE 'firefox|epiphany' && r=да || r=нет
проверь "браузер стоит в системе" "$r" "$(echo "$apps_json" | head -c 200)"

mime=$(спроси '{"method":"sys.mime","params":{"тип":"x-scheme-handler/https"}}')
echo "$mime" | grep -qiE 'firefox|epiphany' && r=да || r=нет
проверь "ссылки открываются браузером" "$r" "$(echo "$mime" | head -c 200)"

# Скачанный файл-установщик должен открываться установщиком системы
mimedeb=$(спроси '{"method":"sys.mime","params":{"тип":"application/vnd.debian.binary-package"}}')
echo "$mimedeb" | grep -q 'glower-package' && r=да || r=нет
проверь "скачанный .deb открывает установщик системы" "$r" "$(echo "$mimedeb" | head -c 160)"

badfile=$(спроси '{"method":"pkg.file.info","params":{"путь":"/etc/passwd"}}')
echo "$badfile" | grep -q '"ok":false' && r=да || r=нет
проверь "вместо пакета чужой файл система не берёт" "$r" "$(echo "$badfile" | head -c 160)"

# Установка программы из скачанного файла — от начала до конца. Пробный пакет
# собираем здесь же и кладём в машину так же, как это делает браузер: файлом
# в папку человека.
pkgdir=/tmp/glower-proba-vm
rm -rf "$pkgdir"; mkdir -p "$pkgdir/DEBIAN" "$pkgdir/usr/bin"
cat > "$pkgdir/DEBIAN/control" <<'CTRL'
Package: glower-proba
Version: 1.0
Section: utils
Priority: optional
Architecture: all
Maintainer: GlowerOS <glower@localhost>
Installed-Size: 24
Description: Пробная программа для проверки установки из файла
CTRL
printf '#!/bin/sh\necho проба\n' > "$pkgdir/usr/bin/glower-proba"
chmod 755 "$pkgdir/usr/bin/glower-proba"

if dpkg-deb --build "$pkgdir" /tmp/glower-proba.deb >/dev/null 2>&1; then
  b64=$(base64 -w0 /tmp/glower-proba.deb)
  printf '{"method":"fs.writeDataUrl","params":{"path":"Загрузки/glower-proba.deb","dataUrl":"data:application/vnd.debian.binary-package;base64,%s"}}' "$b64" > /tmp/glower-pkg.json
  curl -s --max-time 30 -X POST "http://localhost:$PORT/rpc" \
    -H 'Content-Type: application/json' --data @/tmp/glower-pkg.json > /dev/null

  pkgfile="/home/glower/GlowerOS/Загрузки/glower-proba.deb"
  about=$(спроси "{\"method\":\"pkg.file.info\",\"params\":{\"путь\":\"$pkgfile\"}}")
  echo "$about" | grep -q '"имя":"glower-proba"' && r=да || r=нет
  проверь "система читает скачанный пакет" "$r" "$(echo "$about" | head -c 160)"

  спроси "{\"method\":\"pkg.file.install\",\"params\":{\"путь\":\"$pkgfile\"}}" > /dev/null
  done_pkg=нет
  job=''
  for i in $(seq 1 30); do
    sleep 10
    job=$(спроси '{"method":"pkg.job","params":{}}')
    if echo "$job" | grep -q '"running":false'; then
      echo "$job" | grep -q '"ok":true' && done_pkg=да
      break
    fi
  done
  проверь "программа из файла ставится" "$done_pkg" "$(echo "$job" | head -c 200)"

  have=$(спроси '{"method":"pkg.info","params":{"name":"glower-proba"}}')
  echo "$have" | grep -q '"installed":"1.0"' && r=да || r=нет
  проверь "поставленная программа числится в системе" "$r" "$(echo "$have" | head -c 160)"
fi

# Значок настоящей программы: человек узнаёт программу по её картинке
icon=$(спроси '{"method":"sys.icon","params":{"имя":"org.xfce.thunar"}}')
echo "$icon" | grep -q '"есть":true' && r=да || r=нет
проверь "у настоящей программы есть свой значок" "$r" "$(echo "$icon" | head -c 120)"

# Язык системы: программы должны говорить с человеком по-русски
caps2=$(спроси '{"method":"sys.caps","params":{}}')
echo "$caps2" | grep -qi 'ru_RU' && r=да || r=нет
проверь "система говорит по-русски" "$r" "$(echo "$caps2" | head -c 200)"

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
# Установщику нужен полный путь диска (/dev/vda), а не короткое имя:
# по короткому он честно отвечает «такого диска на машине нет».
target=$(echo "$disks_json" | grep -o '"dev":"/dev/[a-z0-9]*"' | head -1 | cut -d'"' -f4)
[ -n "$target" ] && r=да || r=нет
проверь "диск для установки найден" "$r" "$(echo "$disks_json" | head -c 200)"

if [ -n "$target" ]; then
  спроси "{\"method\":\"install.start\",\"params\":{\"disk\":\"$target\",\"confirm\":\"$target\"}}" > /dev/null
  done_ok=нет
  # Под виртуальной машиной без ускорения перенос системы на диск идёт
  # долго: ждём до часа и не считаем медленную работу провалом.
  for i in $(seq 1 180); do
    sleep 20
    st=$(спроси '{"method":"install.state","params":{}}')
    echo "$st" | grep -q '"percent":100\|"готово":true\|"done":true' && { done_ok=да; break; }
    echo "$st" | grep -q '"error":"[^"]' && break
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
