param([string]$WordExecutable, [string]$ExpectedArchitecture)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'word-windows-executable-provenance.ps1')

function New-TestPeImage([string]$Path, [uint16]$Machine, [bool]$Is64Bit) {
    $bytes = [byte[]]::new(512)
    [BitConverter]::GetBytes([uint16]0x5a4d).CopyTo($bytes, 0)
    [BitConverter]::GetBytes([uint32]64).CopyTo($bytes, 0x3c)
    [BitConverter]::GetBytes([uint32]0x00004550).CopyTo($bytes, 64)
    [BitConverter]::GetBytes($Machine).CopyTo($bytes, 68)
    $optionalSize = if ($Is64Bit) { [uint16]240 } else { [uint16]224 }
    $magic = if ($Is64Bit) { [uint16]0x20b } else { [uint16]0x10b }
    [BitConverter]::GetBytes($optionalSize).CopyTo($bytes, 84)
    [BitConverter]::GetBytes([uint16]0x0102).CopyTo($bytes, 86)
    [BitConverter]::GetBytes($magic).CopyTo($bytes, 88)
    [BitConverter]::GetBytes([uint32]0x1000).CopyTo($bytes, 120)
    [BitConverter]::GetBytes([uint32]0x200).CopyTo($bytes, 124)
    [BitConverter]::GetBytes([uint32]0x1000).CopyTo($bytes, 144)
    [BitConverter]::GetBytes([uint32]512).CopyTo($bytes, 148)
    [IO.File]::WriteAllBytes($Path, $bytes)
}

function Assert-Rejected([string]$Path) {
    $rejected = $false
    try { $null = Get-WordCaptureExecutableArchitecture -LiteralPath $Path }
    catch { $rejected = $true }
    if (-not $rejected) { throw "Malformed executable was accepted: $Path" }
}

$testRoot = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) ('jade-word-pe-' + [Guid]::NewGuid().ToString('N'))))
[void][IO.Directory]::CreateDirectory($testRoot)
try {
    foreach ($case in @(
        @{name='x86'; machine=[uint16]0x014c; wide=$false},
        @{name='x64'; machine=[uint16]0x8664; wide=$true},
        @{name='arm64'; machine=[uint16]0xaa64; wide=$true},
        @{name='arm64ec'; machine=[uint16]0xa641; wide=$true},
        @{name='pe-machine-0x7777'; machine=[uint16]0x7777; wide=$true}
    )) {
        $path = Join-Path $testRoot ($case.name + '.exe')
        New-TestPeImage $path $case.machine $case.wide
        $actual = Get-WordCaptureExecutableArchitecture -LiteralPath $path
        if ($actual -ne $case.name) { throw "Machine identity changed: $($case.name) -> $actual" }
    }
    $invalid = Join-Path $testRoot 'invalid.exe'
    [IO.File]::WriteAllText($invalid, 'This is not an executable')
    Assert-Rejected $invalid
    [IO.File]::WriteAllBytes($invalid, [byte[]](0x4d,0x5a))
    Assert-Rejected $invalid
    New-TestPeImage $invalid ([uint16]0x014c) $true
    Assert-Rejected $invalid
    New-TestPeImage $invalid ([uint16]0x8664) $false
    Assert-Rejected $invalid
    if ($WordExecutable) {
        if (-not $ExpectedArchitecture) { throw 'Actual Word comparison requires ExpectedArchitecture' }
        $actual = Get-WordCaptureExecutableArchitecture -LiteralPath $WordExecutable
        if ($actual -ne $ExpectedArchitecture) { throw "Actual Word architecture mismatch: $actual" }
        Write-Output "Actual Word executable: $actual; host: $([Runtime.InteropServices.RuntimeInformation]::OSArchitecture)"
    }
    Write-Output 'Executable architecture: five machine identities and four invalid images PASS'
} finally {
    # Delete only this test's own flat temporary files; never recurse.
    foreach ($path in [IO.Directory]::GetFiles($testRoot)) {
        if (-not [IO.Path]::GetFullPath($path).StartsWith($testRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::Ordinal)) {
            throw 'Unexpected test cleanup path'
        }
        [IO.File]::Delete($path)
    }
    [IO.Directory]::Delete($testRoot, $false)
}