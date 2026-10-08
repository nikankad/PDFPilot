use crate::pdf::{self, PdfDocument};
use anyhow::{Context, Result, ensure};
use lopdf::{Document, EncryptionState, EncryptionVersion, Permissions};
use rand::Rng;
use std::{collections::BTreeMap, path::Path, sync::Arc};

#[derive(Clone)]
pub struct SecurityOptions {
    pub open_password: String,
    pub owner_password: String,
    pub print: bool,
    pub edit: bool,
    pub copy: bool,
    pub annotate: bool,
    pub fill: bool,
    pub assemble: bool,
}
impl Default for SecurityOptions {
    fn default() -> Self {
        Self {
            open_password: String::new(),
            owner_password: String::new(),
            print: true,
            edit: true,
            copy: true,
            annotate: true,
            fill: true,
            assemble: true,
        }
    }
}
pub fn protect(doc: &Document, options: &SecurityOptions, output: &Path) -> Result<()> {
    use lopdf::encryption::crypt_filters::{Aes256CryptFilter, CryptFilter};
    ensure!(
        !options.owner_password.is_empty(),
        "Enter an owner password"
    );
    if !options.edit
        || !options.copy
        || !options.print
        || !options.annotate
        || !options.fill
        || !options.assemble
    {
        ensure!(
            options.owner_password != options.open_password,
            "Restricted documents need different open and owner passwords"
        );
    }
    let mut permissions = Permissions::COPYABLE_FOR_ACCESSIBILITY;
    for (allow, flag) in [
        (
            options.print,
            Permissions::PRINTABLE | Permissions::PRINTABLE_IN_HIGH_QUALITY,
        ),
        (options.edit, Permissions::MODIFIABLE),
        (options.copy, Permissions::COPYABLE),
        (options.annotate, Permissions::ANNOTABLE),
        (options.fill, Permissions::FILLABLE),
        (options.assemble, Permissions::ASSEMBLABLE),
    ] {
        if allow {
            permissions |= flag;
        }
    }
    let mut key = [0u8; 32];
    rand::rng().fill(&mut key);
    let mut filters: BTreeMap<Vec<u8>, Arc<dyn CryptFilter>> = BTreeMap::new();
    filters.insert(b"StdCF".to_vec(), Arc::new(Aes256CryptFilter));
    let state = EncryptionState::try_from(EncryptionVersion::V5 {
        encrypt_metadata: true,
        crypt_filters: filters,
        file_encryption_key: &key,
        stream_filter: b"StdCF".to_vec(),
        string_filter: b"StdCF".to_vec(),
        owner_password: &options.owner_password,
        user_password: &options.open_password,
        permissions,
    })?;
    let mut copy = doc.clone();
    copy.version = "2.0".into();
    copy.encrypt(&state)?;
    pdf::atomic_save(&copy, output)
}
pub fn settings(doc: &Document) -> String {
    let Some(state) = &doc.encryption_state else {
        return "Security: no password encryption".into();
    };
    let p = state.permissions();
    let mut lines = vec![
        format!(
            "Security handler: V{}, revision {}",
            state.version(),
            state.revision()
        ),
        format!("Metadata encrypted: {}", state.encrypt_metadata()),
    ];
    for (name, flag) in [
        ("Printing", Permissions::PRINTABLE),
        ("Editing", Permissions::MODIFIABLE),
        ("Copying", Permissions::COPYABLE),
        ("Annotations", Permissions::ANNOTABLE),
        ("Form filling", Permissions::FILLABLE),
        ("Page assembly", Permissions::ASSEMBLABLE),
    ] {
        lines.push(format!(
            "{name}: {}",
            if p.contains(flag) {
                "allowed"
            } else {
                "restricted"
            }
        ));
    }
    lines.join("\n")
}
impl PdfDocument {
    pub fn remove_protection_copy(&self, owner_password: &str, output: &Path) -> Result<()> {
        let state = self
            .doc
            .encryption_state
            .clone()
            .context("This document is not password-protected")?;
        // Reconstruct the encryption dictionary from the preserved state, then
        // authenticate the owner separately from the document-opening password.
        let mut copy = self.doc.clone();
        copy.encrypt(&state)?;
        copy.authenticate_owner_password(owner_password)
            .context("Owner password is required")?;
        copy.decrypt(owner_password)?;
        copy.encryption_state = None;
        pdf::atomic_save(&copy, output)
    }
}
