//! Word's templates as files: the folder テンプレートとして保存 offers and
//! 新規テンプレートから opens.

use std::path::PathBuf;

/// The user's templates folder, made when it is not there yet: what Word
/// for Mac calls マイ テンプレート.
pub fn templates_folder() -> PathBuf {
    let folder = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join("Library/Application Support/jade-office/Templates"))
        .unwrap_or_else(|| PathBuf::from("."));
    if !cfg!(test) {
        std::fs::create_dir_all(&folder).ok();
    }
    folder
}
