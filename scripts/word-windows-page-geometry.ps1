# Word GoTo can return a continued table/footnote's earlier body anchor.
# The Page object still identifies the physical EMF being captured.
function Get-WordCapturePageGeometry {
    param($Document, $Page, [int]$PageNumber)
    if ($PageNumber -lt 1) { throw 'Physical page number must be positive' }
    $range=$Document.GoTo(1,1,$PageNumber)
    $source='goto-physical-page'
    if ($range.Information(3) -ne $PageNumber) {
        for ($index=1; $index -le $Page.Rectangles.Count; $index++) {
            try {
                $candidate=$Page.Rectangles.Item($index).Range
                if ($candidate.Information(3) -eq $PageNumber) {
                    $range=$candidate
                    $source='physical-page-rectangle'
                    break
                }
            } catch { continue }
        }
    }
    if ($range.Information(3) -eq $PageNumber) {
        $setup=$range.Sections.Item(1).PageSetup
    } elseif ($Document.Sections.Count -eq 1) {
        # Continued-note-only pages have no body anchor on the page. A sole
        # section gives an unambiguous exact size, including fractional points.
        $setup=$Document.Sections.Item(1).PageSetup
        $source='single-section-continuation'
    } else {
        throw 'Cannot resolve exact section geometry for this physical page'
    }
    if ([Math]::Abs($setup.PageWidth-$Page.Width) -gt 0.51 -or
        [Math]::Abs($setup.PageHeight-$Page.Height) -gt 0.51) {
        throw 'Word page geometry differs from its section setup'
    }
    [pscustomobject]@{PageSetup=$setup;Source=$source}
}
