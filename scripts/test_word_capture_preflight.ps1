$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$encrypted=Join-Path $root 'jade-core/public/fixtures/word-made/agile-encrypted.docx'
$ordinary=Join-Path $root 'jade-core/public/fixtures/hello.docx'
$capture=Join-Path $PSScriptRoot 'capture-word-windows.ps1'
$output=Join-Path ([IO.Path]::GetTempPath()) ('jade-word-preflight-'+[Guid]::NewGuid().ToString('N'))
function Reject-BeforeWord($documents,$message) {
    $actual=''
    try { & $capture -Documents $documents -OutputDirectory $output }
    catch { $actual=$_.Exception.Message }
    if(-not $actual.Contains($message)){throw "Wrong preflight result: $actual"}
    if(Test-Path -LiteralPath $output){throw 'Rejected input created a capture directory'}
}
Reject-BeforeWord @($encrypted) 'requires a supplied password'
Reject-BeforeWord @($ordinary,$ordinary) 'Duplicate fixture name'
'2 unattended-capture preflight regressions passed without Word'
