//! Pencere, tepsi simgesi ve "Windows açılışında başlat". İş mantığı dpi.rs'tedir; burası onu masaüstüne bağlar.

use crate::dpi::Dpi;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::window::Color;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_autostart::ManagerExt as _;

/// tauri.conf.json'daki kimlik; uygulamanın veri klasörünün adıdır.
const APP_IDENTIFIER: &str = "com.robz.dpi";
const TRAY_ID: &str = "main";
const WINDOW: &str = "main";

struct Strings {
    open: &'static str,
    turn_on: &'static str,
    turn_off: &'static str,
    quit: &'static str,
    tip_on: &'static str,
    tip_off: &'static str,
}

fn strings(lang: &str) -> Strings {
    if lang == "ru" {
        Strings {
            open: "Открыть Robz DPI",
            turn_on: "Включить",
            turn_off: "Выключить",
            quit: "Выход",
            tip_on: "Robz DPI: включено",
            tip_off: "Robz DPI: выключено",
        }
    } else if lang == "en" {
        Strings {
            open: "Open Robz DPI",
            turn_on: "Turn on",
            turn_off: "Turn off",
            quit: "Quit",
            tip_on: "Robz DPI: on",
            tip_off: "Robz DPI: off",
        }
    } else {
        Strings {
            open: "Robz DPI'ı aç",
            turn_on: "Aç",
            turn_off: "Kapat",
            quit: "Çıkış",
            tip_on: "Robz DPI: açık",
            tip_off: "Robz DPI: kapalı",
        }
    }
}

// ---- dil ------------------------------------------------------------------------------------------------------------

/// "Windows dili": görüntü dili, tercih edilen diller ya da bölge Türkiye ise Türkçe; Rusça ise Rusça; değilse
/// İngilizce.
/// (Türkiye'de İngilizce Windows kullanmak yaygındır; kullanıcı üst çubuktan tek tıkla değiştirebilir.)
#[cfg(windows)]
pub fn system_language() -> &'static str {
    use windows_sys::Win32::Globalization::{
        GetUserDefaultGeoName, GetUserDefaultLocaleName, GetUserPreferredUILanguages, MUI_LANGUAGE_NAME,
    };
    let mut texts = Vec::new();
    // SAFETY: her çağrıya yeterli boyutta tampon ve boyutu verilir.
    unsafe {
        let mut buf = [0u16; 85];
        let n = GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32);
        if n > 1 {
            texts.push(String::from_utf16_lossy(&buf[..(n - 1) as usize]));
        }
        let mut geo = [0u16; 16];
        let n = GetUserDefaultGeoName(geo.as_mut_ptr(), geo.len() as i32);
        if n > 1 && String::from_utf16_lossy(&geo[..(n - 1) as usize]).eq_ignore_ascii_case("TR") {
            return "tr";
        }
        let mut count = 0u32;
        let mut list = [0u16; 512];
        let mut len = list.len() as u32;
        if GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, list.as_mut_ptr(), &mut len) != 0 {
            texts.extend(String::from_utf16_lossy(&list[..len as usize]).split('\0').map(str::to_string));
        }
    }
    let speaks = |code: &str| texts.iter().any(|t| t.to_ascii_lowercase().starts_with(code));
    if speaks("tr") {
        "tr"
    } else if speaks("ru") {
        "ru"
    } else {
        "en"
    }
}

#[cfg(not(windows))]
pub fn system_language() -> &'static str {
    "en"
}

// ---- pencere --------------------------------------------------------------------------------------------------------

pub fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    } else if let Err(error) = create_window(app) {
        eprintln!("pencere açılamadı: {error}");
    }
}

pub fn create_window(app: &AppHandle) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(app, WINDOW, WebviewUrl::App("index.html".into()))
        .title("Robz DPI")
        .inner_size(520.0, 800.0)
        .min_inner_size(440.0, 600.0)
        .center()
        .theme(Some(tauri::Theme::Dark))
        .background_color(Color(13, 17, 23, 255))
        // Arayüz yalnızca kendi sayfasında kalır; bağlantılar varsayılan tarayıcıda açılır (ui/js/main.js).
        .on_navigation(|url| url.scheme() == "tauri" || url.host_str() == Some("tauri.localhost"))
        .build()?;

    let handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            // Motor ayrı bir Windows servisidir: pencere kapansa da çalışmayı sürdürür. Tepside kalmak yalnızca
            // durumu görmeyi ve tek tıkla açıp kapatmayı sağlar.
            if handle.state::<Arc<Dpi>>().settings().close_to_tray {
                api.prevent_close();
                if let Some(window) = handle.get_webview_window(WINDOW) {
                    let _ = window.hide();
                }
            }
        }
    });
    Ok(())
}

// ---- tepsi ----------------------------------------------------------------------------------------------------------

static TRAY_SIGNATURE: Mutex<String> = Mutex::new(String::new());

fn build_tray_menu(app: &AppHandle, lang: &str, running: bool) -> tauri::Result<Menu<tauri::Wry>> {
    let t = strings(lang);
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(app, "open", t.open, true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    let (id, label) = if running { ("off", t.turn_off) } else { ("on", t.turn_on) };
    menu.append(&MenuItem::with_id(app, id, label, true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "quit", t.quit, true, None::<&str>)?)?;
    Ok(menu)
}

pub fn create_tray(app: &AppHandle, dpi: &Dpi) -> tauri::Result<()> {
    let (lang, running) = (dpi.language(), dpi.running());
    let t = strings(lang);
    let menu = build_tray_menu(app, lang, running)?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(if running { t.tip_on } else { t.tip_off })
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_window(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| {
            let dpi = app.state::<Arc<Dpi>>().inner().clone();
            match event.id().as_ref() {
                "open" => show_window(app),
                "on" => {
                    // Yönetici onayı ya da bir hata çıkarsa kullanıcı görsün diye pencere de açılır.
                    show_window(app);
                    tauri::async_runtime::spawn(async move { dpi.turn_on().await });
                }
                "off" => {
                    tauri::async_runtime::spawn(async move { dpi.turn_off().await });
                }
                "quit" => app.exit(0),
                _ => {}
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    *TRAY_SIGNATURE.lock().unwrap_or_else(|p| p.into_inner()) = format!("{lang}|{running}");
    Ok(())
}

/// Durum değişince: tepsi menüsü ve ipucu (motor açıldı/kapandı ya da dil değiştiyse).
pub fn state_changed(dpi: &Dpi) {
    let (lang, running) = (dpi.language(), dpi.running());
    let signature = format!("{lang}|{running}");
    let mut last = TRAY_SIGNATURE.lock().unwrap_or_else(|p| p.into_inner());
    if *last == signature {
        return;
    }
    if let Some(tray) = dpi.app.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_tray_menu(&dpi.app, lang, running) {
            let t = strings(lang);
            let _ = tray.set_menu(Some(menu));
            let _ = tray.set_tooltip(Some(if running { t.tip_on } else { t.tip_off }));
            *last = signature;
        }
    }
}

// ---- uygulamayı kaldırma --------------------------------------------------------------------------------------------

/// Kurulum programının bıraktığı kaldırma programı (yalnız kurulu sürümde vardır).
pub fn uninstaller() -> Option<PathBuf> {
    let path = std::env::current_exe().ok()?.parent()?.join("uninstall.exe");
    path.is_file().then_some(path)
}

/// Bu süreç kapandıktan sonra uygulamayı sessizce kaldırır ve ayar klasörünü siler. Ayrı, penceresiz bir komut
/// satırı başlatır ve hemen döner; çağıran ardından uygulamadan çıkmalıdır.
#[cfg(windows)]
pub fn run_uninstaller(uninstaller: &Path, data_dir: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut script = format!("/d /s /c \"ping -n 3 127.0.0.1 >nul & \"{}\" /S", uninstaller.display());
    // Yalnız uygulamanın kendi veri klasörü silinir (geliştirme için başka bir klasör verilmişse dokunulmaz).
    if data_dir.file_name().is_some_and(|name| name == APP_IDENTIFIER) {
        script.push_str(&format!(" & ping -n 4 127.0.0.1 >nul & rmdir /s /q \"{}\"", data_dir.display()));
    }
    script.push('"');
    let cmd = std::env::var_os("SystemRoot").map(|root| Path::new(&root).join("System32").join("cmd.exe"));
    std::process::Command::new(cmd.as_deref().unwrap_or(Path::new("cmd.exe")))
        .raw_arg(script)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(drop)
}

#[cfg(not(windows))]
pub fn run_uninstaller(_uninstaller: &Path, _data_dir: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("yalnız Windows"))
}

// ---- açılışta başlatma ve zamanlayıcı -------------------------------------------------------------------------------

/// Yalnız kurulu sürümde: geliştirme kipinde geçici exe'yi Windows açılışına eklemeyelim.
pub fn apply_autostart(app: &AppHandle, enabled: bool) {
    if cfg!(debug_assertions) {
        return;
    }
    let autostart = app.autolaunch();
    let _ = if enabled { autostart.enable() } else { autostart.disable() };
}

/// Servisin durumu 2 sn'de bir okunur (pencere kapalı ya da tepsideyken de). Okuma ucuzdur: Servis Denetim
/// Yöneticisi'ne tek sorgu ve süreç listesine bir bakış.
pub fn start_timers(dpi: Arc<Dpi>) {
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(2));
        loop {
            tick.tick().await;
            dpi.poll();
        }
    });
}
