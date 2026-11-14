param(
    [Parameter(Mandatory=$true)][string[]]$Documents,
    [Parameter(Mandatory=$true)][string]$PythonExecutable,
    [string]$OutputDirectory='tmp/word-windows-versioned',
    [string]$NativeExecutable='jade-core/crates/jade-docx-native-render/target/debug/examples/word_coordinates.exe',
    [switch]$CaptureMetafiles,
    [switch]$UnclippedTableGlyphs,
    [hashtable]$DocumentPasswordsBySha256=@{}
)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path -LiteralPath $NativeExecutable)) { throw 'Build the word_coordinates example before capturing' }
$manifest = & (Join-Path $PSScriptRoot 'capture-word-windows.ps1') -Documents $Documents -OutputDirectory $OutputDirectory -CaptureMetafiles:$CaptureMetafiles -UnclippedTableGlyphs:$UnclippedTableGlyphs -DocumentPasswordsBySha256 $DocumentPasswordsBySha256
$capture = Get-Content -LiteralPath $manifest -Raw | ConvertFrom-Json
$directory = Split-Path -Parent $manifest
& $PythonExecutable (Join-Path $PSScriptRoot 'extract-word-windows-reference.py') $manifest
if ($LASTEXITCODE -ne 0) { throw 'Word reference extraction failed' }
foreach ($item in $capture.documents) {
    $nativeOutput = Join-Path $directory ($item.name+'.json')
    if ($UnclippedTableGlyphs) {
        $nativePdf = Join-Path $directory ($item.name+'.native.pdf')
        & $NativeExecutable $item.source $nativeOutput windows $nativePdf --unclipped-tables
    } else {
        & $NativeExecutable $item.source $nativeOutput windows
    }
    if ($LASTEXITCODE -ne 0) { throw "Native capture failed: $($item.name)" }
}
if ($UnclippedTableGlyphs) {
    & $PythonExecutable (Join-Path $PSScriptRoot 'extract-word-windows-reference.py') $manifest --visible
    if ($LASTEXITCODE -ne 0) { throw 'Visible Word reference extraction failed' }
    foreach ($item in $capture.documents) {
        $visibleOutput = Join-Path $directory ($item.name+'.visible.json')
        & $NativeExecutable $item.source $visibleOutput windows
        if ($LASTEXITCODE -ne 0) { throw "Visible native capture failed: $($item.name)" }
    }
    & $PythonExecutable (Join-Path $PSScriptRoot 'compare-word-coordinates.py') $directory --visible
    if ($LASTEXITCODE -ne 0) { throw 'Visible coordinate comparison failed' }
    & $PythonExecutable (Join-Path $PSScriptRoot 'compare-word-coordinates.py') $directory --native-pdf
    if ($LASTEXITCODE -ne 0) { throw 'Painted PDF coordinate comparison failed' }
}
& $PythonExecutable (Join-Path $PSScriptRoot 'compare-word-coordinates.py') $directory
if ($LASTEXITCODE -ne 0) { throw 'Coordinate comparison failed' }
if ($CaptureMetafiles) {
    & $PythonExecutable (Join-Path $PSScriptRoot '../jade-core/scripts/word-oracle/extract_windows_emf.py') $manifest
    if ($LASTEXITCODE -ne 0) { throw 'Word drawing-reference extraction failed' }
    & $PythonExecutable (Join-Path $PSScriptRoot 'compare-word-emf-coordinates.py') $directory
    if ($LASTEXITCODE -ne 0) { throw 'Word drawing-coordinate comparison failed' }
}
Write-Output "Word $($capture.word.product_version): $directory"
