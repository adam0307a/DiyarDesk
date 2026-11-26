[CmdletBinding()]
param(
    [string[]]$Abis,
    [int]$MinSdk = 0,
    [switch]$NoCargoNdk
)

$ErrorActionPreference = 'Stop'

# Cross-compile native/diyardesk-gl (Rust + OpenGL ES 3.0) into Android jniLibs on Windows.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File android\build-native-gl.ps1
#   powershell -ExecutionPolicy Bypass -File android\build-native-gl.ps1 -Abis arm64-v8a,x86_64

$ScriptDir = $PSScriptRoot
$RepoRoot = (Resolve-Path (Join-Path $ScriptDir '..')).Path
$CrateDir = Join-Path $RepoRoot 'native\diyardesk-gl'
$JniLibsDir = Join-Path $ScriptDir 'app\src\main\jniLibs'

if ($MinSdk -le 0) {
    if ($env:ANDROID_MIN_SDK -and ($env:ANDROID_MIN_SDK -as [int])) {
        $MinSdk = [int]$env:ANDROID_MIN_SDK
    } else {
        $MinSdk = 29
    }
}

if (-not $Abis -or $Abis.Count -eq 0) {
    if ($env:ANDROID_ABIS) {
        $Abis = @($env:ANDROID_ABIS -split '[\s,]+' | Where-Object { $_ })
    } else {
        $Abis = @('arm64-v8a', 'x86_64', 'armeabi-v7a')
    }
} else {
    $Abis = @($Abis | ForEach-Object { $_ -split '[\s,]+' } | Where-Object { $_ })
}

function Find-AndroidNdk {
    foreach ($candidate in @($env:ANDROID_NDK_HOME, $env:ANDROID_NDK_LATEST_HOME, $env:ANDROID_NDK_ROOT)) {
        if ($candidate -and (Test-Path -LiteralPath $candidate -PathType Container)) {
            return (Resolve-Path -LiteralPath $candidate).Path
        }
    }

    $sdkCandidates = @($env:ANDROID_SDK_ROOT, $env:ANDROID_HOME, (Join-Path $env:LOCALAPPDATA 'Android\Sdk')) |
        Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Container) }
    foreach ($sdk in $sdkCandidates) {
        $ndkRoot = Join-Path $sdk 'ndk'
        if (Test-Path -LiteralPath $ndkRoot -PathType Container) {
            $dirs = Get-ChildItem -LiteralPath $ndkRoot -Directory | Sort-Object -Property {
                $ver = $null
                if ([Version]::TryParse(($_.Name -replace '[^0-9.].*$', ''), [ref]$ver)) { $ver } else { [Version]'0.0' }
            }, Name
            $latest = $dirs | Select-Object -Last 1
            if ($latest) {
                return $latest.FullName
            }
        }
    }
    return $null
}

$NdkDir = Find-AndroidNdk
if (-not $NdkDir) {
    throw 'Android NDK bulunamadi. ANDROID_NDK_HOME ayarlayin veya Android SDK uzerinden NDK kurun.'
}

$env:ANDROID_NDK_HOME = $NdkDir
Write-Host "Using Android NDK: $NdkDir"
Write-Host "Building native Rust + OpenGL ES 3.0 libraries for ABIs: $($Abis -join ' ') (API $MinSdk)"

New-Item -ItemType Directory -Force -Path $JniLibsDir | Out-Null

$cargoNdkCmd = if (-not $NoCargoNdk) { Get-Command 'cargo-ndk' -ErrorAction SilentlyContinue } else { $null }

function Append-Flag([string]$Existing, [string]$Flag) {
    $trimmed = if ($Existing) { $Existing.Trim() } else { '' }
    if ($trimmed -and $trimmed.Contains($Flag)) {
        return $trimmed
    }
    if ($trimmed) {
        return "$trimmed $Flag"
    }
    return $Flag
}

$page16kFlag = '-C link-arg=-Wl,-z,max-page-size=16384'

if ($cargoNdkCmd) {
    $targetArgs = @()
    foreach ($abi in $Abis) {
        $targetArgs += @('-t', $abi)
    }

    $prevAarch64 = $env:CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS
    $prevX8664 = $env:CARGO_TARGET_X86_64_LINUX_ANDROID_RUSTFLAGS

    Push-Location $CrateDir
    try {
        $env:CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS = Append-Flag $prevAarch64 $page16kFlag
        $env:CARGO_TARGET_X86_64_LINUX_ANDROID_RUSTFLAGS = Append-Flag $prevX8664 $page16kFlag
        & cargo ndk @targetArgs --platform $MinSdk -o $JniLibsDir build --release
        if ($LASTEXITCODE -ne 0) {
            throw "cargo ndk build basarisiz oldu (exit code: $LASTEXITCODE)"
        }
    } finally {
        Pop-Location
        if ($null -eq $prevAarch64) { Remove-Item Env:CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS -ErrorAction SilentlyContinue } else { $env:CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS = $prevAarch64 }
        if ($null -eq $prevX8664) { Remove-Item Env:CARGO_TARGET_X86_64_LINUX_ANDROID_RUSTFLAGS -ErrorAction SilentlyContinue } else { $env:CARGO_TARGET_X86_64_LINUX_ANDROID_RUSTFLAGS = $prevX8664 }
    }
} else {
    $hostTags = @('windows-x86_64', 'windows')
    $toolchainBin = $null
    foreach ($tag in $hostTags) {
        $candidate = Join-Path $NdkDir "toolchains\llvm\prebuilt\$tag\bin"
        if (Test-Path -LiteralPath $candidate -PathType Container) {
            $toolchainBin = $candidate
            break
        }
    }
    if (-not $toolchainBin) {
        throw "NDK LLVM toolchain dizini bulunamadi: $NdkDir\toolchains\llvm\prebuilt\..."
    }

    $arPath = Join-Path $toolchainBin 'llvm-ar.exe'
    if (-not (Test-Path -LiteralPath $arPath -PathType Leaf)) {
        throw "NDK llvm-ar bulunamadi: $arPath"
    }

    foreach ($abi in $Abis) {
        switch ($abi) {
            'arm64-v8a' {
                $rustTarget = 'aarch64-linux-android'
                $clangPrefix = "aarch64-linux-android$MinSdk"
                $targetLower = 'aarch64_linux_android'
                $targetUpper = 'AARCH64_LINUX_ANDROID'
                $extraRustflags = $page16kFlag
            }
            'armeabi-v7a' {
                $rustTarget = 'armv7-linux-androideabi'
                $clangPrefix = "armv7a-linux-androideabi$MinSdk"
                $targetLower = 'armv7_linux_androideabi'
                $targetUpper = 'ARMV7_LINUX_ANDROIDEABI'
                $extraRustflags = ''
            }
            'x86_64' {
                $rustTarget = 'x86_64-linux-android'
                $clangPrefix = "x86_64-linux-android$MinSdk"
                $targetLower = 'x86_64_linux_android'
                $targetUpper = 'X86_64_LINUX_ANDROID'
                $extraRustflags = $page16kFlag
            }
            default {
                throw "Desteklenmeyen ABI: '$abi'"
            }
        }

        $linkerPath = Join-Path $toolchainBin "$clangPrefix-clang.cmd"
        if (-not (Test-Path -LiteralPath $linkerPath -PathType Leaf)) {
            throw "NDK linker bulunamadi: $linkerPath"
        }

        Write-Host "Compiling $rustTarget ($abi) via $linkerPath..."

        $linkerVar = "CARGO_TARGET_${targetUpper}_LINKER"
        $ccVar = "CC_${targetLower}"
        $arVar = "AR_${targetLower}"
        $rfVar = "CARGO_TARGET_${targetUpper}_RUSTFLAGS"

        $savedEnv = @{
            $linkerVar = [Environment]::GetEnvironmentVariable($linkerVar, 'Process')
            $ccVar = [Environment]::GetEnvironmentVariable($ccVar, 'Process')
            $arVar = [Environment]::GetEnvironmentVariable($arVar, 'Process')
            $rfVar = [Environment]::GetEnvironmentVariable($rfVar, 'Process')
        }

        Push-Location $CrateDir
        try {
            [Environment]::SetEnvironmentVariable($linkerVar, $linkerPath, 'Process')
            [Environment]::SetEnvironmentVariable($ccVar, $linkerPath, 'Process')
            [Environment]::SetEnvironmentVariable($arVar, $arPath, 'Process')
            if ($extraRustflags) {
                [Environment]::SetEnvironmentVariable($rfVar, (Append-Flag $savedEnv[$rfVar] $extraRustflags), 'Process')
            }

            & cargo build --release --target $rustTarget
            if ($LASTEXITCODE -ne 0) {
                throw "cargo build --release --target $rustTarget basarisiz oldu (exit code: $LASTEXITCODE)"
            }
        } finally {
            Pop-Location
            foreach ($k in $savedEnv.Keys) {
                [Environment]::SetEnvironmentVariable($k, $savedEnv[$k], 'Process')
            }
        }

        $builtSo = Join-Path $CrateDir "target\$rustTarget\release\libdiyardesk_gl.so"
        if (-not (Test-Path -LiteralPath $builtSo -PathType Leaf)) {
            throw "Derlenen .so bulunamadi: $builtSo"
        }
        $abiOutDir = Join-Path $JniLibsDir $abi
        New-Item -ItemType Directory -Force -Path $abiOutDir | Out-Null
        Copy-Item -LiteralPath $builtSo -Destination (Join-Path $abiOutDir 'libdiyardesk_gl.so') -Force
    }
}

Write-Host 'Native Android OpenGL ES 3.0 libraries ready:'
Get-ChildItem -Path $JniLibsDir -Filter 'libdiyardesk_gl.so' -Recurse | ForEach-Object {
    $mb = [math]::Round($_.Length / 1MB, 2)
    Write-Host ("  {0} ({1} bytes / {2} MB)" -f $_.FullName, $_.Length, $mb)
}
