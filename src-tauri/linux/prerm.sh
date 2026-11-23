#!/bin/sh
# Robz DPI paketi kaldırılırken motor da kaldırılır: systemd birimi, nftables kuralları, polkit kuralı ve motorun
# klasörleri. Bunları uygulama yönetici onayıyla kurmuştu; paket yöneticisi onları kendiliğinden silmez.
# Güncellemede dokunulmaz: motor çalışmayı sürdürür (deb: "upgrade", rpm: 1).
case "$1" in
  upgrade | 1) exit 0 ;;
esac

systemctl stop robzdpi.service 2>/dev/null || true
nft delete table inet robzdpi 2>/dev/null || true
rm -f /etc/systemd/system/robzdpi.service /etc/polkit-1/rules.d/49-robzdpi.rules /run/robzdpi-result.json
rm -rf /opt/robzdpi /var/lib/robzdpi
systemctl daemon-reload 2>/dev/null || true
exit 0
