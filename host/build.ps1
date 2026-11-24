# 从源码构建 Deskland：使用 Windows 自带的 .NET Framework C# 编译器。
$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$lib = Join-Path $root 'lib'
$bin = Join-Path $root 'bin'
New-Item -ItemType Directory -Force $lib, $bin | Out-Null
$version = '1.0.4191.47'
$assemblies = @(
    @{ Entry = 'lib/net462/Microsoft.Web.WebView2.Core.dll'; Name = 'Microsoft.Web.WebView2.Core.dll' },
    @{ Entry = 'lib/net462/Microsoft.Web.WebView2.WinForms.dll'; Name = 'Microsoft.Web.WebView2.WinForms.dll' },
    @{ Entry = 'runtimes/win-x64/native/WebView2Loader.dll'; Name = 'WebView2Loader.dll' }
)
$missing = @($assemblies | Where-Object { -not (Test-Path -LiteralPath (Join-Path $lib $_.Name)) })
if ($missing.Count -gt 0) {
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = Join-Path ([IO.Path]::GetTempPath()) ('deskland-webview2-' + [Guid]::NewGuid().ToString('N') + '.zip')
    $package = $null
    try {
        Write-Host "下载 WebView2 SDK $version..."
        Invoke-WebRequest "https://api.nuget.org/v3-flatcontainer/microsoft.web.webview2/$version/microsoft.web.webview2.$version.nupkg" -OutFile $archive -UseBasicParsing
        $package = [IO.Compression.ZipFile]::OpenRead($archive)
        foreach ($assembly in $assemblies) {
            $entry = $package.GetEntry($assembly.Entry)
            if ($null -eq $entry) { throw "依赖包缺少 $($assembly.Entry)" }
            $sourceStream = $entry.Open()
            $targetStream = $null
            try {
                $targetStream = [IO.File]::Create((Join-Path $lib $assembly.Name))
                $sourceStream.CopyTo($targetStream)
            } finally {
                if ($null -ne $targetStream) { $targetStream.Dispose() }
                $sourceStream.Dispose()
            }
        }
    } finally {
        if ($null -ne $package) { $package.Dispose() }
        if (Test-Path -LiteralPath $archive) { Remove-Item -LiteralPath $archive -Force }
    }
}
$csc = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
if (-not (Test-Path -LiteralPath $csc)) { throw '找不到 .NET Framework C# 编译器，请在 64 位 Windows 上构建。' }
$appVersion = (Get-Content -LiteralPath (Join-Path $root '..\tools\version.json') -Raw | ConvertFrom-Json).version
if ($appVersion -notmatch '^\d+\.\d+\.\d+$') { throw 'Invalid application version' }
$versionSource = Join-Path $bin 'Deskland-version.cs'
$versionText = @"
using System.Reflection;
[assembly: AssemblyTitle("Deskland Preview")]
[assembly: AssemblyProduct("Deskland Preview")]
[assembly: AssemblyVersion("$appVersion.0")]
[assembly: AssemblyFileVersion("$appVersion.0")]
[assembly: AssemblyCopyright("Copyright 2026 Deskland authors")]
"@
[IO.File]::WriteAllText($versionSource, $versionText, (New-Object Text.UTF8Encoding($false)))
$rtDir = [Runtime.InteropServices.RuntimeEnvironment]::GetRuntimeDirectory()
$wpfDir = Join-Path $rtDir 'WPF'
$wpfRefs = @()
if ((Test-Path -LiteralPath (Join-Path $wpfDir 'PresentationCore.dll')) -and (Test-Path -LiteralPath (Join-Path $wpfDir 'WindowsBase.dll'))) {
    $wpfRefs = @(
        '/define:HAS_WIC_WEBP',
        '/r:System.Xaml.dll',
        "/r:$(Join-Path $wpfDir 'PresentationCore.dll')",
        "/r:$(Join-Path $wpfDir 'WindowsBase.dll')"
    )
}
$winRtRefs = @()
$winRtDll = Join-Path $rtDir 'System.Runtime.WindowsRuntime.dll'
$winMeta = Join-Path $env:WINDIR 'System32\WinMetadata'
if ((Test-Path -LiteralPath $winRtDll) -and (Test-Path -LiteralPath (Join-Path $winMeta 'Windows.Media.winmd'))) {
    $winRtRefs = @(
        '/define:HAS_WINRT_SMTC',
        "/r:$(Join-Path $rtDir 'System.Runtime.dll')",
        "/r:$(Join-Path $rtDir 'System.Threading.Tasks.dll')",
        "/r:$(Join-Path $rtDir 'System.Runtime.InteropServices.WindowsRuntime.dll')",
        "/r:$winRtDll",
        "/r:$winMeta\Windows.Foundation.winmd",
        "/r:$winMeta\Windows.Media.winmd",
        "/r:$winMeta\Windows.Storage.winmd"
    )
}
& $csc /nologo /target:winexe /platform:x64 /optimize+ /codepage:65001 /win32manifest:"$root\app.manifest" `
    /win32icon:"$root\DesklandPreview.ico" `
    /out:"$bin\Deskland.exe" `
    /r:System.dll /r:System.Core.dll /r:System.Drawing.dll /r:System.Windows.Forms.dll /r:System.Web.Extensions.dll `
    /r:System.IO.Compression.dll /r:System.IO.Compression.FileSystem.dll `
    @wpfRefs `
    @winRtRefs `
    /r:"$lib\Microsoft.Web.WebView2.Core.dll" /r:"$lib\Microsoft.Web.WebView2.WinForms.dll" `
    "$root\DesktopHost.cs" "$root\Themes.cs" "$root\LanSync.cs" "$root\IpcControl.cs" "$root\SystemBridge.cs" "$root\MediaBridge.cs" "$root\NativeGlHost.cs" "$root\NativeSettingsForm.cs" "$versionSource"
if ($LASTEXITCODE -ne 0) { throw '编译失败' }
foreach ($assembly in $assemblies) { Copy-Item -LiteralPath (Join-Path $lib $assembly.Name) -Destination $bin -Force }
Copy-Item -LiteralPath (Join-Path $root 'DesklandPreview.ico') -Destination (Join-Path $bin 'DesklandPreview.ico') -Force
$rustDir = Join-Path $root '..\native\diyardesk-gl'
$cargoExe = $null
$cargoCmd = Get-Command cargo -ErrorAction SilentlyContinue
if ($cargoCmd) {
    $cargoExe = $cargoCmd.Source
} else {
    $userCargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
    if (Test-Path -LiteralPath $userCargo) { $cargoExe = $userCargo }
}
if ((Test-Path -LiteralPath (Join-Path $rustDir 'Cargo.toml')) -and $cargoExe) {
    Write-Host "Derleniyor: native\diyardesk-gl (cargo build --release)..."
    Push-Location $rustDir
    try {
        & $cargoExe build --release
        if ($LASTEXITCODE -ne 0) { throw 'Rust OpenGL motoru (diyardesk_gl) derlenemedi' }
    } finally {
        Pop-Location
    }
}
$rustDll = Join-Path $rustDir 'target\release\diyardesk_gl.dll'
if (Test-Path -LiteralPath $rustDll) {
    $destDll = Join-Path $bin 'diyardesk_gl.dll'
    $locked = $false
    try {
        $fs = [System.IO.File]::Open($destDll, [System.IO.FileMode]::Open, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::None)
        $fs.Close()
    } catch {
        $locked = $true
    }
    if ($locked) {
        Write-Warning "diyardesk_gl.dll kullanımda (Deskland.exe çalışıyor?). Önce uygulamayı kapatın."
        $procs = Get-Process -Name "Deskland","DiyarDesk" -ErrorAction SilentlyContinue
        if ($procs) {
            Write-Host "Çalışan süreçler: $($procs.Name -join ', ')"
        }
    }
    Copy-Item -LiteralPath $rustDll -Destination $destDll -Force
}
Write-Host "编译完成：$bin\Deskland.exe"
