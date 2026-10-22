//! The macOS print system: a PDF handed to PDFKit's print operation, with
//! the copies, collation and two-sided setting the app's 印刷 panel chose
//! already set in the system's print panel.

/// What the app's 印刷 panel passes on to the system's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrintJob {
    pub copies: u32,
    pub collate: bool,
    pub duplex: jade_office_model::word::printing::Duplex,
}

impl Default for PrintJob {
    fn default() -> Self {
        Self {
            copies: 1,
            collate: true,
            duplex: jade_office_model::word::printing::Duplex::OneSided,
        }
    }
}

/// The value of the print settings' `PMDuplexing` key: 1 one-sided, 2
/// two-sided bound on the long edge, 3 on the short edge (`PMDuplexMode`).
pub fn duplex_mode(duplex: jade_office_model::word::printing::Duplex) -> i32 {
    use jade_office_model::word::printing::Duplex;
    match duplex {
        Duplex::OneSided => 1,
        Duplex::LongEdge => 2,
        Duplex::ShortEdge => 3,
    }
}

#[cfg(target_os = "macos")]
pub fn show_pdf_print_dialog(pdf: &[u8]) -> Result<(), String> {
    show_pdf_print_dialog_with(pdf, PrintJob::default())
}

#[cfg(target_os = "macos")]
pub fn show_pdf_print_dialog_with(pdf: &[u8], job: PrintJob) -> Result<(), String> {
    use objc2::runtime::ProtocolObject;
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSPrintCopies, NSPrintInfo, NSPrintMustCollate};
    use objc2_foundation::{NSData, NSNumber, NSString};
    use objc2_pdf_kit::{PDFDocument, PDFPrintScalingMode};

    let mtm = MainThreadMarker::new()
        .ok_or_else(|| "the native print dialog must run on the main thread".to_string())?;
    let data = NSData::with_bytes(pdf);
    let document = unsafe { PDFDocument::initWithData(PDFDocument::alloc(), &data) }
        .ok_or_else(|| "PDFKit could not open the generated PDF".to_string())?;
    let print_info = NSPrintInfo::sharedPrintInfo();
    unsafe {
        let attributes = print_info.dictionary();
        attributes.setObject_forKey(
            &NSNumber::numberWithInteger(job.copies.max(1) as isize),
            ProtocolObject::from_ref(NSPrintCopies),
        );
        attributes.setObject_forKey(
            &NSNumber::numberWithBool(job.collate),
            ProtocolObject::from_ref(NSPrintMustCollate),
        );
        let settings = print_info.printSettings();
        let key = NSString::from_str("com_apple_print_PrintSettings_PMDuplexing");
        settings.setObject_forKey(
            &NSNumber::numberWithInt(duplex_mode(job.duplex)),
            ProtocolObject::from_ref(&*key),
        );
    }
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

#[cfg(not(target_os = "macos"))]
pub fn show_pdf_print_dialog_with(_pdf: &[u8], _job: PrintJob) -> Result<(), String> {
    Err("native printing is currently available on macOS only".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use jade_office_model::word::printing::Duplex;

    #[test]
    fn two_sided_printing_maps_to_the_print_system_s_modes() {
        assert_eq!(duplex_mode(Duplex::OneSided), 1);
        assert_eq!(duplex_mode(Duplex::LongEdge), 2);
        assert_eq!(duplex_mode(Duplex::ShortEdge), 3);
    }
}
