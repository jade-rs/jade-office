//! デジタル署名 and 署名欄 on the browser build's Word document — what both
//! shells run: a signed document lists who signed and that it holds; once
//! edited its signatures stop holding and the save takes them out; a
//! signature line drawn from its own little page is inserted, saved and
//! read back with whom it asks to sign.

mod common;
use common::{contract, open_bytes, typed};
use jade_web::word::signatures::{
    is_signed, signature_line_docx, SignatureLine, SignatureState, SignatureStatus, Signer,
    WordSignatures,
};

struct TestSigner {
    key: rsa::RsaPrivateKey,
    certificate: Vec<u8>,
}

impl TestSigner {
    fn new(name: &str) -> Self {
        use der::Encode;
        use rsa::pkcs8::EncodePublicKey;
        use std::str::FromStr;
        use x509_cert::builder::{Builder, CertificateBuilder, Profile};
        let key = rsa::RsaPrivateKey::new(&mut rand::thread_rng(), 1024).unwrap();
        let spki_der = key.to_public_key().to_public_key_der().unwrap();
        let spki =
            x509_cert::spki::SubjectPublicKeyInfoOwned::try_from(spki_der.as_bytes()).unwrap();
        let signing_key = rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(key.clone());
        let certificate = CertificateBuilder::new(
            Profile::Root,
            x509_cert::serial_number::SerialNumber::from(7u32),
            x509_cert::time::Validity::from_now(std::time::Duration::from_secs(600)).unwrap(),
            x509_cert::name::Name::from_str(&format!("CN={name}")).unwrap(),
            spki,
            &signing_key,
        )
        .unwrap()
        .build::<rsa::pkcs1v15::Signature>()
        .unwrap();
        Self {
            key,
            certificate: certificate.to_der().unwrap(),
        }
    }
}

impl Signer for TestSigner {
    fn certificate(&self) -> Vec<u8> {
        self.certificate.clone()
    }
    fn sign_sha256(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        use sha2::Digest;
        self.key
            .sign(
                rsa::Pkcs1v15Sign::new::<sha2::Sha256>(),
                &sha2::Sha256::digest(data),
            )
            .map_err(|error| error.to_string())
    }
}

#[test]
fn a_signed_document_holds_until_it_is_edited_and_saved() {
    let document = contract();
    let unsigned = document.export_docx().unwrap();
    let signer = TestSigner::new("Ann Lee");
    // 署名: the next save signs.
    let mut state = SignatureState::opened(&unsigned);
    assert!(!state.is_signed());
    state.pending = Some(jade_web::word::signatures::PendingSign {
        signed_at: "2026-04-01T09:00:00Z".into(),
        comment: "承認".into(),
    });
    let signed = state.finish_save(unsigned, Some(&signer)).unwrap();
    assert!(state.is_signed());
    assert_eq!(state.signatures[0].signer.as_deref(), Some("Ann Lee"));
    assert_eq!(state.status(&state.signatures[0]), SignatureStatus::Valid);

    // Opened again: signed, and valid.
    let mut reopened_state = SignatureState::opened(&signed);
    assert_eq!(reopened_state.signatures.len(), 1);
    assert_eq!(
        reopened_state.signatures[0].comment.as_deref(),
        Some("承認")
    );
    assert_eq!(reopened_state.signatures[0].status, SignatureStatus::Valid);

    // Edited: the signature stops holding, and the save takes it out.
    let mut reopened = open_bytes(&signed);
    typed(&mut reopened, "changed ");
    reopened_state.edited = true;
    assert!(matches!(
        reopened_state.status(&reopened_state.signatures[0]),
        SignatureStatus::Invalid(_)
    ));
    let saved = reopened_state
        .finish_save(reopened.export_docx().unwrap(), None)
        .unwrap();
    assert!(!is_signed(&saved));
    assert!(!reopened_state.is_signed());
}

#[test]
fn a_signature_line_is_inserted_from_its_own_page_and_read_back() {
    let line = SignatureLine {
        id: None,
        signer: Some("山田 太郎".into()),
        title: Some("部長".into()),
        email: Some("taro@example.com".into()),
        instructions: None,
    };
    // Its picture: the little page, rendered as the shell renders pages.
    let page = open_bytes(&signature_line_docx(&line));
    assert_eq!(page.page_count(), 1);
    let picture = page.page(0).unwrap().encode_png().unwrap();
    let mut document = contract();
    let paragraph = typed(&mut document, "署名: ");
    document.place_caret(paragraph, "署名: ".len());
    document.insert_signature_line(line, picture).unwrap();
    let lines = document.signature_lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].signer.as_deref(), Some("山田 太郎"));
    let reopened = open_bytes(&document.export_docx().unwrap());
    let lines = reopened.signature_lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].title.as_deref(), Some("部長"));
    document.undo().unwrap();
    assert!(document.signature_lines().is_empty());
}
