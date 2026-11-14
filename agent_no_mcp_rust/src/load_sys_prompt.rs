use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Loads a generated system prompt from an explicit asset directory.
pub fn get_sys_prompt_from_dir(asset_dir: &Path) -> Result<String> {
    let path = asset_dir.join("sys_prompt.md");
    fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))
}

/// Loads the copied generated system prompt packaged with this Rust crate.
pub fn get_sys_prompt() -> Result<String> {
    get_sys_prompt_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .as_path(),
    )
}
