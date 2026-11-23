#!/usr/bin/env bash
# Linux motorunu uçtan uca dener (root olarak, systemd olan bir makinede):  sudo scripts/linux-smoke.sh
#
#   1. Motor programıyla (robzdpi-engine --helper) motoru kurar ve başlatır: arayüzün pkexec ile yaptığı iş
#   2. systemd biriminin çalıştığını ve nftables kurallarının yüklendiğini denetler
#   3. Bölünmüş paketlerle gerçek bir siteye bağlanır: paketler geçerli olmalı, bağlantı kurulmalı
#   4. Sahte istekleri bu makinedeki yerel bir sunucuya karşı dener: sunucu onları atmalı, bağlantı kurulmalı
#   5. Motoru durdurur ve kaldırır; geride birim, kural ya da klasör kalmamalı
#
# Sahte isteklerin dış bir siteye karşı sonucu aradaki ağa bağlıdır (hangi yöntemin nerede çalıştığı ağdan ağa
# değişir); o yüzden burada yerel bir sunucuyla, sonucu belli bir düzende denenir.
# CI bunu her gönderimde çalıştırır. Bilgisayarının ağ ayarlarına kalıcı bir şey bırakmaz.
set -euo pipefail

BIN="${1:-/usr/lib/robz-dpi/robzdpi-engine}"
SITE="${SMOKE_SITE:-example.com}"
STATE=/var/lib/robzdpi
WORK="$(mktemp -d)"

# Yerel sunucu: ayrı bir ağ ad alanında, sanal bir kabloyla (veth) bu makineye bağlı.
NS=robzsmoke
LOCAL_HOST=smoke.robzdpi.test
LOCAL_IP=10.199.77.2

cleanup() {
  [ -n "${SERVER_PID:-}" ] && kill "$SERVER_PID" 2>/dev/null || true
  ip netns del "$NS" 2>/dev/null || true
  ip link del "${NS}0" 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT

say() { printf '\n== %s\n' "$*"; }
fail() { printf 'HATA: %s\n' "$*" >&2; [ -f "$STATE/engine.log" ] && tail -20 "$STATE/engine.log" >&2; exit 1; }

[ "$(id -u)" = 0 ] || fail "root olarak çalıştırılmalı"
[ -x "$BIN" ] || fail "$BIN yok"

job() { # ad, bozuk sağlama toplamlı sahte istek, eski sıra numaralı sahte istek
  cat > "$WORK/job.json" <<EOF
{"nonce":"smoke-$1","request":{"strategy":{"split":2,"splitAtSni":false,"reverse":true,"httpSplit":2,"fakeTtlMode":"off","fakeTtl":5,"fakeBadChecksum":$2,"fakeBadSeq":$3,"fakeRepeats":1,"fakeHost":"www.w3.org","blockQuic":false,"noSni":"known"},"dns":null,"domains":["$SITE","robzdpi.test"]}}
EOF
}

apply() {
  "$BIN" --helper apply "$WORK/job.json" || fail "yardımcı başarısız: $(cat /run/robzdpi-result.json 2>/dev/null)"
  [ "$(systemctl is-active robzdpi.service)" = active ] || fail "birim çalışmıyor"
}

# Motorun işlediği bağlantı sayısı (stats.json).
handled() { grep -o '"tls":[0-9]*' "$STATE/stats.json" 2>/dev/null | cut -d: -f2 || echo 0; }

# Motor üzerinden bir adrese bağlanır ve motorun bağlantıyı işlediğini denetler. Dış siteler için üç kez denenir:
# tek bir bağlantının kurulamaması denemeyi düşürmez, üçünün de kurulamaması düşürür.
check() { # ad, deneme sayısı, curl bağımsız değişkenleri…
  local name="$1" tries="$2" before after code attempt
  shift 2
  before=$(handled)
  for attempt in $(seq 1 "$tries"); do
    code=$(curl -s -o /dev/null -m 15 -w '%{http_code}' "$@" || true)
    [ "$code" = 200 ] && break
    echo "$name: $attempt. deneme sonuç vermedi ($code)"
    sleep 2
  done
  sleep 2.5 # sayaç dosyası birkaç saniyede bir yazılır
  after=$(handled)
  echo "$name: $code ($attempt. denemede), işlenen bağlantı ${before:-0} -> ${after:-0}"
  [ "$code" = 200 ] || fail "$name: bağlantı kurulamadı ($code)"
  [ "${after:-0}" -gt "${before:-0}" ] || fail "$name: motor bağlantıyı görmedi"
}

say "yerel sunucu"
ip netns add "$NS"
ip link add "${NS}0" type veth peer name "${NS}1"
ip link set "${NS}1" netns "$NS"
ip addr add 10.199.77.1/30 dev "${NS}0"
ip link set "${NS}0" up
ip netns exec "$NS" ip addr add "$LOCAL_IP/30" dev "${NS}1"
ip netns exec "$NS" ip link set "${NS}1" up
ip netns exec "$NS" ip link set lo up
openssl req -x509 -newkey rsa:2048 -nodes -keyout "$WORK/key.pem" -out "$WORK/cert.pem" -days 1 -subj "/CN=$LOCAL_HOST" >/dev/null 2>&1
ip netns exec "$NS" openssl s_server -accept 443 -www -cert "$WORK/cert.pem" -key "$WORK/key.pem" -quiet >/dev/null 2>&1 &
SERVER_PID=$!
sleep 1
LOCAL=(-k --resolve "$LOCAL_HOST:443:$LOCAL_IP" "https://$LOCAL_HOST/")
code=$(curl -s -o /dev/null -m 10 -w '%{http_code}' "${LOCAL[@]}" || true)
[ "$code" = 200 ] || fail "yerel sunucu yanıt vermiyor ($code)"
echo "https://$LOCAL_HOST/ ($LOCAL_IP) -> $code"

say "kurulum: yalnız bölme"
job split false false
apply
nft list table inet robzdpi | grep -q "queue" || fail "kurallar yüklenmedi"
systemctl show robzdpi.service --property MainPID --property ActiveState
check "dış site, yalnız bölme (https://$SITE/)" 3 "https://$SITE/"
check "yerel sunucu, yalnız bölme" 1 "${LOCAL[@]}"

say "ayar değişikliği: bölme + sunucunun attığı sahte istekler (bozuk sağlama toplamı, eski sıra numarası)"
job fakes true true
apply
check "yerel sunucu, bölme + sahte istekler" 1 "${LOCAL[@]}"

say "geçersiz istek reddedilmeli"
printf '{"nonce":"smoke-bad","request":{"strategy":{"split":2,"splitAtSni":false,"reverse":true,"httpSplit":2,"fakeTtlMode":"off","fakeTtl":5,"fakeBadChecksum":false,"fakeBadSeq":false,"fakeRepeats":1,"fakeHost":"a b","blockQuic":false,"noSni":"known"},"dns":null,"domains":["%s"]}}' "$SITE" > "$WORK/job.json"
if "$BIN" --helper apply "$WORK/job.json"; then fail "geçersiz istek kabul edildi"; fi
grep -q "smoke-bad" /run/robzdpi-result.json || fail "hata sonucu yazılmadı"
echo "reddedildi: $(cat /run/robzdpi-result.json)"

say "durdurma"
"$BIN" --helper stop "$WORK/job.json" || fail "durdurulamadı"
[ "$(systemctl is-active robzdpi.service || true)" != active ] || fail "birim hâlâ çalışıyor"
if nft list table inet robzdpi >/dev/null 2>&1; then fail "kurallar geride kaldı"; fi
tail -1 "$STATE/engine.log"

say "kaldırma"
"$BIN" --helper remove "$WORK/job.json" || fail "kaldırılamadı"
for path in /etc/systemd/system/robzdpi.service /etc/polkit-1/rules.d/49-robzdpi.rules /opt/robzdpi "$STATE"; do
  [ ! -e "$path" ] || fail "geride kaldı: $path"
done
code=$(curl -s -o /dev/null -m 15 -w '%{http_code}' "https://$SITE/" || true)
[ "$code" = 200 ] || fail "kaldırdıktan sonra site açılmadı ($code)"
echo "geride bir şey kalmadı; site motor olmadan da açılıyor ($code)"

# Paket yöneticisiyle kaldırma: paketin kaldırma betiği motoru da silmeli (SMOKE_PACKAGE=robz-dpi ile denenir).
if [ -n "${SMOKE_PACKAGE:-}" ]; then
  say "paket kaldırılınca motor da gitmeli"
  job split false false
  apply
  apt-get remove -y "$SMOKE_PACKAGE" >/dev/null
  for path in /etc/systemd/system/robzdpi.service /opt/robzdpi "$STATE"; do
    [ ! -e "$path" ] || fail "paket kaldırıldı ama geride kaldı: $path"
  done
  if nft list table inet robzdpi >/dev/null 2>&1; then fail "paket kaldırıldı ama kurallar kaldı"; fi
  echo "paketle birlikte motor da kaldırıldı"
fi

say "TAMAM"
