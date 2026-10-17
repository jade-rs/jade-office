#[cfg(target_os = "macos")]
pub fn show_pdf_print_dialog(pdf: &[u8]) -> Result<(), String> {
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::NSPrintInfo;
    use objc2_foundation::NSData;
    use objc2_pdf_kit::{PDFDocument, PDFPrintScalingMode};

    let mtm = MainThreadMarker::new()
        .ok_or_else(|| "the native print dialog must run on the main thread".to_string())?;
    let data = NSData::with_bytes(pdf);
    let document = unsafe { PDFDocument::initWithData(PDFDocument::alloc(), &data) }
        .ok_or_else(|| "PDFKit could not open the generated PDF".to_string())?;
    let print_info = NSPrintInfo::sharedPrintInfo();
    let operation = unsafe {
        document.printOperationForPrintInfo_scalingMode_autoRotate(
            Some(&print_info),
            PDFPrintScalingMode::PageScaleNone,
            true,
            mtm,
        )
    }
    .ok_or_else(|| "PDFKit could not create a print operation".to_string())?;

    operation.setShowsPrintPanel(true);
    operation.setShowsProgressPanel(true);
    operation.runOperation();
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn show_pdf_print_dialog(_pdf: &[u8]) -> Result<(), String> {
    Err("native printing is currently available on macOS only".to_string())
}
