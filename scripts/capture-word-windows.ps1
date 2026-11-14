param(
    [Parameter(Mandatory=$true)][string[]]$Documents,
    [Parameter(Mandatory=$true)][string]$OutputDirectory,
    [switch]$CaptureMetafiles,
    [switch]$UnclippedTableGlyphs,
    [switch]$HideComments,
    [switch]$OpenAndRepair,
    [hashtable]$DocumentPasswordsBySha256=@{}
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'word-windows-page-geometry.ps1')
$root = [IO.Path]::GetFullPath($OutputDirectory)
$sourceNames = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
foreach ($source in $Documents) {
    $path = (Resolve-Path -LiteralPath $source).Path
    if (-not $sourceNames.Add([IO.Path]::GetFileNameWithoutExtension($path))) {
        throw "Duplicate fixture name: $path"
    }
    $sourceHash=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    if (-not $DocumentPasswordsBySha256.ContainsKey($sourceHash)) {
        $stream=[IO.File]::OpenRead($path)
        try { $header=New-Object byte[] 8; $read=$stream.Read($header,0,8) }
        finally { $stream.Dispose() }
        if ($read -eq 8 -and [BitConverter]::ToString($header) -eq 'D0-CF-11-E0-A1-B1-1A-E1') {
        throw 'Compound/encrypted DOCX requires a supplied password for unattended capture'
        }
    }
}
# A separate COM instance; never attach to or close the user's Word instance.
$word = New-Object -ComObject Word.Application
$word.Visible = $false
$word.DisplayAlerts = 0
$word.AutomationSecurity = 3
$metadata = $null
$manifest = $null
try {
    $binary = Join-Path $word.Path 'WINWORD.EXE'
    $binaryVersion = [Diagnostics.FileVersionInfo]::GetVersionInfo($binary)
    $productVersion = $binaryVersion.ProductVersion
    if ($productVersion -notmatch '^\d+\.\d+\.\d+\.\d+$') { throw "Unknown Word product version: $productVersion" }
    $captureId = [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffZ') + '-' + [Guid]::NewGuid().ToString('N').Substring(0,8)
    $output = Join-Path (Join-Path $root "word-$productVersion") $captureId
    [void][IO.Directory]::CreateDirectory($output)
    $binaryHash = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
    $metadata = [ordered]@{
        schema=2; platform='windows'; capture_id=$captureId; captured_at_utc=[DateTime]::UtcNow.ToString('o'); status='capturing'
        word=[ordered]@{ product_version=$productVersion; file_version=$binaryVersion.FileVersion; application_version=$word.Version; application_build=$word.Build; executable_sha256=$binaryHash }
        os=[Runtime.InteropServices.RuntimeInformation]::OSDescription
        os_architecture=[Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
        documents=@()
    }
    if ($UnclippedTableGlyphs) { $metadata['table_glyph_projection'] = 'unclipped' }
    $manifest = Join-Path $output 'capture.json'
    foreach ($source in $Documents) {
        $path = (Resolve-Path -LiteralPath $source).Path
        $sourceHash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($OpenAndRepair) {
            $missing=[Type]::Missing
            $password=$missing
            if ($DocumentPasswordsBySha256.ContainsKey($sourceHash)) {
                $password=[string]$DocumentPasswordsBySha256[$sourceHash]
            }
            # Documents.Open: OpenAndRepair is argument 14. This repairs only
            # the read-only in-memory document; never save over the source.
            $document=$word.Documents.Open($path,$false,$true,$false,$password,$missing,
                $false,$missing,$missing,$missing,$missing,$false,$false,$true,$missing,$true)
        } elseif ($DocumentPasswordsBySha256.ContainsKey($sourceHash)) {
            $password=[string]$DocumentPasswordsBySha256[$sourceHash]
            $document=$word.Documents.Open($path,$false,$true,$false,$password)
        } else {
            # Preserve ordinary DOCX opening; the preflight already excludes
            # compound/encrypted inputs without an explicitly supplied password.
            $document=$word.Documents.Open($path,$false,$true,$false)
        }
        try {
            if ($HideComments) { $document.ActiveWindow.View.ShowComments=$false }
            $document.Repaginate()
            $target = Join-Path $output ([IO.Path]::GetFileNameWithoutExtension($path)+'.pdf')
            $document.ExportAsFixedFormat($target, 17)
            $pages=$document.ComputeStatistics(2)
            $metafiles=@()
            if ($CaptureMetafiles) {
                $document.ActiveWindow.View.Type=3
                $document.Repaginate()
                for ($pageNumber=1; $pageNumber -le $pages; $pageNumber++) {
                    $page=$document.ActiveWindow.Panes.Item(1).Pages.Item($pageNumber)
                    $geometry=Get-WordCapturePageGeometry $document $page $pageNumber
                    $pageSetup=$geometry.PageSetup
                    $metafileName=([IO.Path]::GetFileNameWithoutExtension($path))+('.page-{0:D3}.emf' -f $pageNumber)
                    $metafilePath=Join-Path $output $metafileName
                    [IO.File]::WriteAllBytes($metafilePath, [byte[]]$page.EnhMetaFileBits)
                    $metafiles += [ordered]@{ page=$pageNumber; file=$metafileName; sha256=(Get-FileHash -LiteralPath $metafilePath -Algorithm SHA256).Hash.ToLowerInvariant(); width_pt=$pageSetup.PageWidth; height_pt=$pageSetup.PageHeight; geometry_source=$geometry.Source }
                }
            }
            if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $sourceHash) { throw "DOCX changed during capture: $path" }
            $item=[ordered]@{ name=[IO.Path]::GetFileNameWithoutExtension($path); source=[IO.Path]::GetRelativePath((Get-Location).Path,$path); sha256=$sourceHash; pdf=[IO.Path]::GetFileName($target); pdf_sha256=(Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash.ToLowerInvariant(); pages=$pages; compatibility_mode=[int]$document.CompatibilityMode }
            $item['password_provided']=$DocumentPasswordsBySha256.ContainsKey($sourceHash)
            $item['open_mode']=if($OpenAndRepair){'open-and-repair-read-only'}else{'normal-read-only'}
            $item['word_view']=[ordered]@{
                hide_comments_requested=[bool]$HideComments
                show_comments=[bool]$document.ActiveWindow.View.ShowComments
                show_revisions_and_comments=[bool]$document.ActiveWindow.View.ShowRevisionsAndComments
                revisions_mode=[int]$document.ActiveWindow.View.RevisionsMode
            }
            if ($CaptureMetafiles) { $item.metafiles=$metafiles }
            $metadata.documents += $item
        } finally { $document.Close(0); [void][Runtime.InteropServices.Marshal]::ReleaseComObject($document) }
    }
    if ((Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant() -ne $binaryHash) { throw 'Word binary changed during capture' }
    $metadata.status = 'complete'
    $metadata | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $manifest -Encoding utf8
    Write-Output $manifest
} catch {
    if ($metadata -and $manifest) {
        $metadata.status = 'failed'
        $metadata | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $manifest -Encoding utf8
    }
    throw
} finally { $word.Quit(); [void][Runtime.InteropServices.Marshal]::ReleaseComObject($word) }
