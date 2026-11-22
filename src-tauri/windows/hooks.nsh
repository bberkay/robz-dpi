; Robz DPI'ın kurulum programına eklenen adımlar (tauri.conf.json -> bundle.windows.nsis.installerHooks).
;
; Uygulama Windows'un "Uygulamalar" listesinden kaldırılırken motor da kaldırılır: Windows servisi, WinDivert
; sürücüsü ve "Program Files\RobzDPI" klasörü. Bunlar yönetici yetkisiyle kurulduğu için Windows onay ister.
; - Güncellemede (/UPDATE) dokunulmaz: motor çalışmayı sürdürür.
; - Motor kurulu değilse (klasör yoksa) onay istenmez. Uygulamanın içindeki "Robz DPI'ı kaldır" motoru önceden
;   sildiği için o yolda da burada yapılacak iş kalmaz.
; - Kullanıcı onayı reddederse kaldırma sürer; motor yerinde kalır.

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    ${If} ${FileExists} "$PROGRAMFILES64\RobzDPI\*.*"
      ExecShellWait "runas" "$INSTDIR\RobzDPI.exe" "--helper remove" SW_HIDE
    ${EndIf}
  ${EndIf}
!macroend
