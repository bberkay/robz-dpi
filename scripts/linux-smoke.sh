#!/usr/bin/env bash
# Kurulu Linux paketini uçtan uca dener (root olarak, systemd olan bir makinede):  sudo scripts/linux-smoke.sh
#
#   1. Motor programıyla (robzdpi-engine --helper) motoru kurar ve başlatır: arayüzün pkexec ile yaptığı iş
#   2. systemd biriminin çalıştığını ve nftables kurallarının yüklendiğini denetler
#   3. Motorun değiştirdiği paketlerle gerçek bir siteye bağlanır: paketler geçerli olmalı, bağlantı kurulmalı
#   4. Motoru durdurur ve kaldırır; geride birim, kural ya da klasör kalmamalı
#
# CI bunu her gönderimde çalıştırır. Bilgisayarının ağ ayarlarına kalıcı bir şey bırakmaz.
set -euo pipefail

BIN="${1:-/usr/lib/robz-dpi/robzdpi-engine}"
SITE="${SMOKE_SITE:-example.com}"
STATE=/var/lib/robzdpi
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

say() { printf '\n== %s\n' "$*"; }
fail() { printf 'HATA: %s\n' "$*" >&2; [ -f "$STATE/engine.log" ] && tail -20 "$STATE/engine.log" >&2; exit 1; }

[ "$(id -u)" = 0 ] || fail "root olarak çalıştırılmalı"
[ -x "$BIN" ] || fail "$BIN yok"

job() { # ad, strateji alanları
  cat > "$WORK/job.json" <<EOF
{"nonce":"smoke-$1","request":{"strategy":{"split":2,"splitAtSni":false,"reverse":true,"httpSplit":2,"fakeTtlMode":"off","fakeTtl":5,"fakeBadChecksum":$2,"fakeBadSeq":$3,"fakeRepeats":1,"fakeHost":"www.w3.org","blockQuic":false,"noSni":"known"},"dns":null,"domains":["$SITE"]}}
EOF
}

# Motor üzerinden siteye bağlanır. Sonuç dış bir siteye ve aradaki ağa bağlı olduğu için üç kez denenir: tek bir
# bağlantının kurulamaması denemeyi düşürmez, üçünün de kurulamaması düşürür.
check_site() { # ad
  local before after code attempt
  before=$(sed -E 's/.*"tls":([0-9]+).*//' "$STATE/stats.json" 2>/dev/null || echo 0)
  for attempt in 1 2 3; do
    code=$(curl -s -o /dev/null -m 15 -w '%{http_code}' "https://$SITE/" || true)
    [ "$code" = 200 ] && break
    echo "$1: $attempt. deneme sonuç vermedi ($code)"
    sleep 2
  done
  sleep 2.5 # sayaç dosyası birkaç saniyede bir yazılır
  after=$(sed -E 's/.*"tls":([0-9]+).*//' "$STATE/stats.json")
  echo "$1: https://$SITE/ -> $code ($attempt. denemede), işlenen bağlantı $before -> $after"
  [ "$code" = 200 ] || fail "$1: site açılmadı ($code)"
  [ "$after" -gt "$before" ] || fail "$1: motor bağlantıyı görmedi"
}

say "kurulum: yalnız bölme"
job split false false
"$BIN" --helper apply "$WORK/job.json" || fail "yardımcı başarısız: $(cat /run/robzdpi-result.json 2>/dev/null)"
[ "$(systemctl is-active robzdpi.service)" = active ] || fail "birim çalışmıyor"
nft list table inet robzdpi | grep -q "queue" || fail "kurallar yüklenmedi"
systemctl show robzdpi.service --property MainPID --property ActiveState
check_site "yalnız bölme"

# Sahte istek yalnız sağlama toplamı bozuk olanıdır: her alıcı onu sessizce atar. Eski sıra numaralı sahte isteğe
# bazı sunucular ve ağ geçitleri farklı tepki verdiği için dış bir siteyle yapılan bu denemede kullanılmaz.
say "ayar değişikliği: bölme + sağlama toplamı bozuk sahte istek"
job fakes true false
"$BIN" --helper apply "$WORK/job.json" || fail "yardımcı başarısız: $(cat /run/robzdpi-result.json 2>/dev/null)"
[ "$(systemctl is-active robzdpi.service)" = active ] || fail "birim çalışmıyor"
check_site "bölme + sahte istek"

say "geçersiz istek reddedilmeli"
printf '{"nonce":"smoke-bad","request":{"strategy":{"split":2,"splitAtSni":false,"reverse":true,"httpSplit":2,"fakeTtlMode":"off","fakeTtl":5,"fakeBadChecksum":false,"fakeBadSeq":false,"fakeRepeats":1,"fakeHost":"a b","blockQuic":false,"noSni":"known"},"dns":null,"domains":["%s"]}}' "$SITE" > "$WORK/job.json"
if "$BIN" --helper apply "$WORK/job.json"; then fail "geçersiz istek kabul edildi"; fi
grep -q "smoke-bad" /run/robzdpi-result.json || fail "hata sonucu yazılmadı"
echo "reddedildi: $(cat /run/robzdpi-result.json)"

say "durdurma"
"$BIN" --helper stop "$WORK/job.json" || fail "durdurulamadı"
[ "$(systemctl is-active robzdpi.service || true)" != active ] || fail "birim hâlâ çalışıyor"
if nft list table inet robzdpi >/dev/null 2>&1; then fail "kurallar geride kaldı"; fi
tail -3 "$STATE/engine.log"

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
  "$BIN" --helper apply "$WORK/job.json" || fail "yardımcı başarısız"
  apt-get remove -y "$SMOKE_PACKAGE" >/dev/null
  for path in /etc/systemd/system/robzdpi.service /opt/robzdpi "$STATE"; do
    [ ! -e "$path" ] || fail "paket kaldırıldı ama geride kaldı: $path"
  done
  if nft list table inet robzdpi >/dev/null 2>&1; then fail "paket kaldırıldı ama kurallar kaldı"; fi
  echo "paketle birlikte motor da kaldırıldı"
fi

say "TAMAM"
