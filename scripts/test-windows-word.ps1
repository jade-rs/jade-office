# Run from the repository root. Requires mbx, Rust stable, MSVC/Windows SDK, and
# Windows Arial/Times New Roman/MS Mincho/Yu Mincho/Century fonts.
$ErrorActionPreference = 'Stop'
$saved = @{}
foreach ($name in @('PATH','RUSTUP_TOOLCHAIN','SKIA_BINARIES_URL','CARGO_TARGET_DIR')) {
    $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
try {
    $env:RUSTUP_TOOLCHAIN = 'stable'
    $env:PATH = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;" + $env:PATH
    # rust-skia 0.87 publishes this Windows feature superset, but does not
    # publish the svg/webp combination without Vulkan. Rendering remains CPU.
    if (-not $env:SKIA_BINARIES_URL) {
        $env:SKIA_BINARIES_URL = 'https://github.com/rust-skia/skia-binaries/releases/download/0.87.0/skia-binaries-e551f334ad5cbdf43abf-x86_64-pc-windows-msvc-pdf-svg-textlayout-vulkan-webpd-webpe.tar.gz'
    }
    # mbx shares compiled actions across the native crate, workspace and worktrees.
    # Let it manage target placement rather than pinning a private target directory.
    Remove-Item -LiteralPath 'Env:\CARGO_TARGET_DIR' -ErrorAction SilentlyContinue
    $mbx = (Get-Command mbx -CommandType Application -ErrorAction Stop).Source
    & $mbx test --locked --manifest-path jade-core/crates/jade-docx-native-render/Cargo.toml --lib --test word_mac_coordinates --test word_windows_coordinates
    if ($LASTEXITCODE -ne 0) { throw 'Native coordinate tests failed' }
    & $mbx test --locked --manifest-path jade-core/Cargo.toml -p jade-docx-layout --lib --features native-shaping
    if ($LASTEXITCODE -ne 0) { throw 'Layout tests failed' }
} finally {
    foreach ($name in $saved.Keys) {
        if ($null -eq $saved[$name]) {
            Remove-Item -LiteralPath ("Env:\$name") -ErrorAction SilentlyContinue
        } else {
            [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process')
        }
    }
}
