$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'word-windows-page-geometry.ps1')
function Collection($values) {
    $c=[pscustomobject]@{Count=$values.Count;Values=$values}
    $c|Add-Member ScriptMethod Item {param($i) $this.Values[$i-1]}
    return $c
}
function Range($page, $width=420.0, $height=480.0) {
    $r=[pscustomobject]@{PageNumber=$page;Sections=(Collection @([pscustomobject]@{PageSetup=[pscustomobject]@{PageWidth=$width;PageHeight=$height}}))}
    $r|Add-Member ScriptMethod Information {param($kind) if($kind -ne 3){throw 'Unexpected information request'};return $this.PageNumber}
    return $r
}
function Document($range, $sections=1) {
    $d=[pscustomobject]@{Anchor=$range;Sections=(Collection @($range.Sections.Item(1))) }
    $d.Sections.Count=$sections
    $d|Add-Member ScriptMethod GoTo {param($what,$which,$count) if($what -ne 1 -or $which -ne 1){throw 'Unexpected page navigation'};return $this.Anchor}
    return $d
}
function Page($ranges, $width=420.0, $height=480.0) {
    [pscustomobject]@{Width=$width;Height=$height;Rectangles=(Collection @($ranges|ForEach-Object {[pscustomobject]@{Range=$_}}))}
}
function Equal($actual,$expected,$label) {if($actual -ne $expected){throw "$label : expected $expected, got $actual"}}
function Reject($block,$message) {
    try {& $block;throw 'Expected rejection was missing'} catch {
        if(-not $_.Exception.Message.Contains($message)){throw}
    }
}
$normal=Range 2
$r=Get-WordCapturePageGeometry (Document $normal) (Page @()) 2
Equal $r.Source 'goto-physical-page' 'ordinary page'
$anchor=Range 1
$r=Get-WordCapturePageGeometry (Document $anchor 2) (Page @($null,(Range 2))) 2
Equal $r.Source 'physical-page-rectangle' 'merged cell earlier-page anchor'
Equal $r.PageSetup.PageWidth 420 'actual section width'
$r=Get-WordCapturePageGeometry (Document $anchor) (Page @((Range 1))) 2
Equal $r.Source 'single-section-continuation' 'continued-footnote-only page'
Reject {Get-WordCapturePageGeometry (Document $anchor 2) (Page @((Range 1))) 2} 'Cannot resolve exact section'
Reject {Get-WordCapturePageGeometry (Document (Range 2 421)) (Page @()) 2} 'geometry differs'
Reject {Get-WordCapturePageGeometry (Document $anchor) (Page @()) 0} 'must be positive'
$r=Get-WordCapturePageGeometry (Document (Range 1 595.3 841.9)) (Page @() 595 842) 1
Equal $r.PageSetup.PageWidth 595.3 'exact A4 section width must survive rounded window size'
'7 physical-page geometry regressions passed'
