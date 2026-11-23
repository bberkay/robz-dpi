//! Linux'ta yönetici yardımcısı: `robz-dpi --helper <işlem> <iş dosyası>`.
//!
//! Arayüz süreci bu kipi yönetici onayıyla (polkit / pkexec) root olarak çalıştırır. Penceresi yoktur; işi yapar
//! ve çıkar. Çıkış kodu 0 başarıdır. Hata olursa ayrıntı RESULT_FILE'a yazılır.
//!
//! Yardımcı, arayüz sürecinden geleni güvenilir saymaz:
//!   * İstek (strateji, DNS adresi, alan adları) yeniden doğrulanır; systemd birimi buradaki sabitlerle yazılır
//!     ve istekten hiçbir metin içermez.
//!   * Motor olarak kopyalanan program bu sürecin kendi çalışan görüntüsüdür: kullanıcının yönetici onayı verdiği
//!     kod ile birim olarak kurulan kod aynıdır.
//!   * Yalnız sabit yollara yazılır: /opt/robzdpi, /var/lib/robzdpi, birim dosyası ve polkit kuralı.

use super::common::{Applied, Job, JobResult};
use super::linux::{
    engine_id, image_path, processes_named, state_dir, system_binary, ENGINE_BIN, ENGINE_DIR, EXIT_FILE, FOREIGN_ENGINES, POLKIT_RULE,
    UNIT, UNIT_FILE,
};
use super::nfqueue;
use crate::error::{DpiError, DpiResult};
use crate::plan::EngineRequest;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

/// Yardımcının hata bıraktığı dosya (/run yalnız root tarafından yazılır, herkes okuyabilir).
pub(super) const RESULT_FILE: &str = "/run/robzdpi-result.json";

const MAX_JOB_BYTES: u64 = 1024 * 1024;

/// systemd birimi. Type=notify: motor kuyruğu açıp kuralları yükleyince "hazırım" der; `systemctl start` o zaman
/// döner. Birim açılışta kendiliğinden başlamaz: motoru arayüz başlatır.
const UNIT_TEXT: &str = "\
[Unit]
Description=Robz DPI motoru
Documentation=https://github.com/kutsalkiremitci/robz-dpi
After=network.target

[Service]
Type=notify
ExecStart=/opt/robzdpi/robzdpi-engine --engine
# Motor beklenmedik biçimde kapanırsa kuralları geride bırakmasın.
ExecStopPost=-/bin/sh -c 'nft delete table inet robzdpi 2>/dev/null'
TimeoutStartSec=15
TimeoutStopSec=10
# Motorun gereksindiği yetkiler: nftables kuralları ve paket kuyruğu (NET_ADMIN), ham yuva (NET_RAW).
CapabilityBoundingSet=CAP_NET_ADMIN CAP_NET_RAW
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
ReadWritePaths=/var/lib/robzdpi
";

/// polkit kuralı: oturum açmış yerel kullanıcı birimi yalnız başlatıp durdurabilir. Motoru kurmak, ayarını
/// değiştirmek ve kaldırmak yönetici onayı ister (pkexec).
const POLKIT_TEXT: &str = "\
// Robz DPI: oturum açmış yerel kullanıcı motoru başlatıp durdurabilir.
polkit.addRule(function(action, subject) {
    if (action.id == \"org.freedesktop.systemd1.manage-units\" &&
        action.lookup(\"unit\") == \"robzdpi.service\" &&
        (action.lookup(\"verb\") == \"start\" || action.lookup(\"verb\") == \"stop\") &&
        subject.local && subject.active) {
        return polkit.Result.YES;
    }
});
";

pub fn main(args: &[String]) -> i32 {
    let action = args.first().map(String::as_str).unwrap_or_default();
    let job = args.get(1).and_then(|path| read_job(Path::new(path))).unwrap_or_default();
    let _ = fs::remove_file(RESULT_FILE);
    // SAFETY: geteuid bağımsız değişken almaz ve başarısız olmaz.
    let result = if unsafe { libc::geteuid() } != 0 {
        Err(DpiError::new("access-denied"))
    } else {
        match action {
            "apply" => job.request.as_ref().ok_or_else(|| DpiError::new("bad-request")).and_then(apply),
            "stop" => stop_engine(),
            "remove" => remove(),
            "conflicts" => stop_conflicts(),
            _ => Err(DpiError::new("bad-request")),
        }
    };
    match result {
        Ok(()) => 0,
        Err(error) => {
            let result = JobResult { nonce: job.nonce, error };
            let _ = fs::write(RESULT_FILE, serde_json::to_vec(&result).unwrap_or_default());
            let _ = fs::set_permissions(RESULT_FILE, fs::Permissions::from_mode(0o644));
            1
        }
    }
}

fn read_job(path: &Path) -> Option<Job> {
    if fs::metadata(path).ok()?.len() > MAX_JOB_BYTES {
        return None;
    }
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

fn io_error(code: &str) -> impl Fn(std::io::Error) -> DpiError + '_ {
    move |error| DpiError::with(code, error)
}

fn systemctl(arguments: &[&str]) -> DpiResult<bool> {
    let program = system_binary("systemctl").ok_or_else(|| DpiError::new("engine-missing"))?;
    let status = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(io_error("helper-failed"))?;
    Ok(status.success())
}

/// Herkesin okuyabildiği, yalnız root'un yazabildiği dosya.
fn write_file(path: &Path, bytes: &[u8], mode: u32) -> DpiResult<()> {
    fs::write(path, bytes).map_err(io_error("write-failed"))?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(io_error("write-failed"))
}

fn make_dir(path: &Path) -> DpiResult<()> {
    fs::create_dir_all(path).map_err(io_error("write-failed"))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).map_err(io_error("write-failed"))
}

/// Motoru kurar ya da ayarını değiştirir, sonra başlatır.
fn apply(request: &EngineRequest) -> DpiResult<()> {
    request.validate()?;
    let state = state_dir();
    let engine_dir = Path::new(ENGINE_DIR);

    // Çalışıyorsa durdur: program ve ayar değişecek.
    stop_engine()?;
    make_dir(engine_dir)?;
    make_dir(&state)?;
    install_engine(engine_dir)?;

    write_file(Path::new(UNIT_FILE), UNIT_TEXT.as_bytes(), 0o644)?;
    // polkit kuralı yazılamazsa (klasör yoksa) kurulum sürer: birimi başlatırken sistem parola sorar.
    if Path::new(POLKIT_RULE).parent().is_some_and(Path::is_dir) {
        let _ = write_file(Path::new(POLKIT_RULE), POLKIT_TEXT.as_bytes(), 0o644);
    }
    write_file(&state.join("engine.json"), &serde_json::to_vec_pretty(request).unwrap_or_default(), 0o644)?;

    // Birim artık bu ayarla kurulu: başlatma başarısız olsa da kayıt doğrudur.
    let applied = Applied { fingerprint: request.fingerprint(&engine_id()) };
    write_file(&state.join("applied.json"), &serde_json::to_vec(&applied).unwrap_or_default(), 0o644)?;

    systemctl(&["daemon-reload"])?;
    if systemctl(&["start", UNIT])? {
        return Ok(());
    }
    let code = fs::read_to_string(state.join(EXIT_FILE)).ok().and_then(|text| text.trim().parse::<u32>().ok());
    Err(match code {
        Some(code) => DpiError::with("engine-start-failed", code),
        None => DpiError::new("engine-start-failed"),
    })
}

/// Motoru /opt/robzdpi'ye kopyalar: bu sürecin kendi çalışan görüntüsü. Önce geçici bir ada yazılır, sonra yeniden
/// adlandırılır; yarım kalmış bir kopya hiçbir zaman motorun yerinde durmaz.
fn install_engine(dir: &Path) -> DpiResult<()> {
    let source = image_path().ok_or_else(|| DpiError::new("engine-missing"))?;
    let bytes = fs::read(&source).map_err(io_error("engine-missing"))?;
    let target = dir.join(ENGINE_BIN);
    if fs::read(&target).is_ok_and(|current| current == bytes) {
        return Ok(());
    }
    let temporary = dir.join(format!("{ENGINE_BIN}.new"));
    write_file(&temporary, &bytes, 0o755)?;
    fs::rename(&temporary, &target).map_err(io_error("write-failed"))
}

/// Birimi durdurur. Kurulu değilse bir şey yapmaz.
fn stop_engine() -> DpiResult<()> {
    if !Path::new(UNIT_FILE).is_file() {
        return Ok(());
    }
    if systemctl(&["stop", UNIT])? {
        Ok(())
    } else {
        Err(DpiError::new("engine-stop-failed"))
    }
}

/// Motoru bütünüyle kaldırır: birim, polkit kuralı, kurallar ve iki klasör.
fn remove() -> DpiResult<()> {
    stop_engine()?;
    nfqueue::remove_rules();
    let _ = fs::remove_file(UNIT_FILE);
    let _ = fs::remove_file(POLKIT_RULE);
    let _ = systemctl(&["daemon-reload"]);
    let _ = systemctl(&["reset-failed", UNIT]);
    for dir in [Path::new(ENGINE_DIR), state_dir().as_path()] {
        if dir.exists() {
            fs::remove_dir_all(dir).map_err(io_error("write-failed"))?;
        }
    }
    Ok(())
}

/// Bilgisayarda çalışan başka bir DPI aracını durdurur (zapret'in nfqws/tpws süreçleri gibi). Arayüz bunu yalnız
/// kullanıcı onay verdikten sonra ister. Birim olarak kuruluysa birimi de durdurulur; dosyalarına dokunulmaz.
fn stop_conflicts() -> DpiResult<()> {
    if Path::new("/etc/systemd/system/zapret.service").is_file() || Path::new("/lib/systemd/system/zapret.service").is_file() {
        let _ = systemctl(&["stop", "zapret.service"]);
    }
    // SAFETY: getpid bağımsız değişken almaz.
    let own = unsafe { libc::getpid() } as u32;
    for (_, pid) in processes_named(&FOREIGN_ENGINES) {
        if pid > 1 && pid != own {
            // SAFETY: yalnız yukarıda adı doğrulanmış süreçlere SIGTERM gönderilir.
            unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(500));
    if processes_named(&FOREIGN_ENGINES).iter().any(|(_, pid)| *pid != own) {
        return Err(DpiError::new("conflict-stop-failed"));
    }
    Ok(())
}
