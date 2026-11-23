# Windows taşınabilir sürümünü (zip) derlenmiş uygulamadan kurar:  pwsh scripts/portable.ps1 -Version 0.4.0
#
# Önce `npm run build` (ya da `npx tauri build`) çalışmış olmalı. Zip'in içinde tek bir "Robz DPI" klasörü vardır:
#   RobzDPI.exe          uygulama (kurulum dosyasındakiyle aynı program)
#   engine\              WinDivert dosyaları ve lisansları (kurulumdaki "engine" klasörüyle aynı)
#   portable.txt         taşınabilir sürümün işareti: ayarlar programın yanındaki "data" klasöründe tutulur
#   LICENSE, THIRD-PARTY-NOTICES.md, DISCLAIMER.md
# Çıktı: out\RobzDPI-Portable-<sürüm>-x64.zip ve sabit adlı out\RobzDPI-Portable.zip
param(
  [Parameter(Mandatory = $true)][string]$Version,
  [string]$Out = 'out'
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

$exe = Join-Path $root 'src-tauri/target/release/RobzDPI.exe'
$engine = Join-Path $root 'src-tauri/engine'
foreach ($path in @($exe, (Join-Path $engine 'WinDivert.dll'), (Join-Path $engine 'WinDivert64.sys'), (Join-Path $engine 'licenses'))) {
  if (-not (Test-Path $path)) { throw "$path yok: önce 'npm run build' çalıştırın" }
}

$work = Join-Path ([IO.Path]::GetTempPath()) ("robzdpi-portable-" + [guid]::NewGuid().ToString('N'))
$dir = Join-Path $work 'Robz DPI'
New-Item -ItemType Directory -Force (Join-Path $dir 'engine') | Out-Null
try {
  Copy-Item $exe $dir
  Copy-Item (Join-Path $engine 'WinDivert.dll'), (Join-Path $engine 'WinDivert64.sys') (Join-Path $dir 'engine')
  Copy-Item (Join-Path $engine 'licenses') (Join-Path $dir 'engine/licenses') -Recurse
  Copy-Item (Join-Path $root 'src-tauri/windows/portable.txt') $dir
  foreach ($doc in 'LICENSE', 'THIRD-PARTY-NOTICES.md', 'DISCLAIMER.md') { Copy-Item (Join-Path $root $doc) $dir }

  New-Item -ItemType Directory -Force $Out | Out-Null
  $zip = Join-Path $Out "RobzDPI-Portable-$Version-x64.zip"
  if (Test-Path $zip) { Remove-Item $zip }
  Compress-Archive -Path $dir -DestinationPath $zip -CompressionLevel Optimal
  Copy-Item $zip (Join-Path $Out 'RobzDPI-Portable.zip') -Force
  Get-ChildItem $Out -Filter 'RobzDPI-Portable*.zip' | ForEach-Object { '{0}  {1:N1} MB' -f $_.Name, ($_.Length / 1MB) }
} finally {
  Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
}
