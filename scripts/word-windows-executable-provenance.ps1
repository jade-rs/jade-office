# Read Word's executable architecture, independently of the host OS or PowerShell.
Add-Type -AssemblyName System.Reflection.Metadata

function Get-WordCaptureExecutableArchitecture {
    [CmdletBinding()]
    [OutputType([string])]
    param([Parameter(Mandatory=$true)][string]$LiteralPath)

    $stream = [IO.File]::OpenRead([IO.Path]::GetFullPath($LiteralPath))
    try {
        $reader = [System.Reflection.PortableExecutable.PEReader]::new(
            $stream, [System.Reflection.PortableExecutable.PEStreamOptions]::LeaveOpen)
        try {
            $headers = $reader.PEHeaders
            if ($null -eq $headers.PEHeader) { throw 'Word executable must be a PE image' }
            $machine = [uint16]$headers.CoffHeader.Machine
            $expectedMagic = $null
            $architecture = switch ($machine) {
                0x014c { $expectedMagic = 'PE32'; 'x86' }
                0x8664 { $expectedMagic = 'PE32Plus'; 'x64' }
                0xaa64 { $expectedMagic = 'PE32Plus'; 'arm64' }
                0xa641 { $expectedMagic = 'PE32Plus'; 'arm64ec' }
                default { 'pe-machine-0x{0:x4}' -f $machine }
            }
            if ($expectedMagic -and $headers.PEHeader.Magic.ToString() -ne $expectedMagic) {
                throw 'Word executable machine and optional-header architecture disagree'
            }
            return $architecture
        } finally { $reader.Dispose() }
    } finally { $stream.Dispose() }
}