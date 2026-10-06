[CmdletBinding()]
param(
    [string]$LlvmMingwHome = $env:LLVM_MINGW_HOME,
    [switch]$SkipReleaseBuild
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = Split-Path -Parent $PSScriptRoot
$product = Join-Path $root 'product'

if (-not $LlvmMingwHome) {
    $LlvmMingwHome = Get-ChildItem -Path (Join-Path $HOME 'Tools') -Directory -Filter 'llvm-mingw-*-ucrt-x86_64' -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending |
        Select-Object -First 1 -ExpandProperty FullName
}
if (-not $LlvmMingwHome -or -not (Test-Path -LiteralPath (Join-Path $LlvmMingwHome 'bin\x86_64-w64-mingw32-gcc.exe'))) {
    throw 'LLVM-MinGW was not found. Set LLVM_MINGW_HOME to an extracted free/open-source llvm-mingw UCRT x86_64 release.'
}

$toolchain = '1.85.1-x86_64-pc-windows-gnu'
$cargo = Join-Path $HOME '.cargo\bin\cargo.exe'
$rustup = Join-Path $HOME '.cargo\bin\rustup.exe'
if (-not (Test-Path -LiteralPath $cargo) -or -not (Test-Path -LiteralPath $rustup)) {
    throw 'Rustup/Cargo was not found under ~/.cargo/bin.'
}

& $rustup run $toolchain rustc --version | Out-Host
if ($LASTEXITCODE -ne 0) {
    throw "Pinned GNU Rust toolchain is missing: $toolchain"
}

$compat = Join-Path $product 'target\llvm-mingw-rust-compat'
New-Item -ItemType Directory -Force -Path $compat | Out-Null
$builtins = Get-ChildItem -Path (Join-Path $LlvmMingwHome 'lib\clang\*\lib\windows\libclang_rt.builtins-x86_64.a') -File |
    Sort-Object FullName -Descending |
    Select-Object -First 1 -ExpandProperty FullName
if (-not $builtins) { throw 'LLVM-MinGW compiler-rt builtins archive was not found.' }
Copy-Item -Force -LiteralPath $builtins -Destination (Join-Path $compat 'libgcc.a')
Copy-Item -Force -LiteralPath (Join-Path $LlvmMingwHome 'x86_64-w64-mingw32\lib\libunwind.a') -Destination (Join-Path $compat 'libgcc_eh.a')

$oldPath = $env:PATH
$oldRustFlags = $env:RUSTFLAGS
try {
    $env:PATH = "$(Join-Path $LlvmMingwHome 'bin');$env:PATH"
    $env:RUSTFLAGS = "-L native=$compat"
    Push-Location $product
    try {
        & $cargo "+$toolchain" fmt --all -- --check
        if ($LASTEXITCODE -ne 0) { throw 'cargo fmt failed' }
        & $cargo "+$toolchain" check --workspace --all-targets --locked
        if ($LASTEXITCODE -ne 0) { throw 'cargo check failed' }
        & $cargo "+$toolchain" test --workspace --exclude polik-recovery-desktop --locked
        if ($LASTEXITCODE -ne 0) { throw 'non-desktop workspace tests failed' }
        # The GNU debug cdylib exceeds PE's 65,535-export ceiling. Release
        # dead-code elimination keeps the linked desktop test artifact below
        # that platform limit, so exercise the desktop tests in release mode.
        & $cargo "+$toolchain" test --release --locked -p polik-recovery-desktop
        if ($LASTEXITCODE -ne 0) { throw 'release desktop tests failed' }
        & $cargo "+$toolchain" clippy --workspace --all-targets --locked -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw 'cargo clippy failed' }
        if (-not $SkipReleaseBuild) {
            & $cargo "+$toolchain" build --release --locked -p polik-recovery-cli
            if ($LASTEXITCODE -ne 0) { throw 'release CLI build failed' }
        }
    }
    finally {
        Pop-Location
    }
}
finally {
    $env:PATH = $oldPath
    $env:RUSTFLAGS = $oldRustFlags
}

Write-Host 'Windows product checks passed (including release-profile desktop linked tests).'
