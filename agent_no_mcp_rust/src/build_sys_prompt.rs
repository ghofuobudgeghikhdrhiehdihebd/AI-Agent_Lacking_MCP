use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::path::Path;

/// Rebuilds `sys_prompt.md` from the template and JSON tool definitions.
pub fn build_sys_prompt_in_dir(asset_dir: &Path) -> Result<()> {
    let template_path = asset_dir.join("sys_prompt_template.md");
    let tools_path = asset_dir.join("tools.json");
    let output_path = asset_dir.join("sys_prompt.md");

    let template = fs::read_to_string(&template_path)
        .with_context(|| format!("failed to read {}", template_path.display()))?;
    let tools: serde_json::Map<String, Value> = serde_json::from_str(
        &fs::read_to_string(&tools_path)
            .with_context(|| format!("failed to read {}", tools_path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", tools_path.display()))?;

    let mut lines: Vec<String> = template.lines().map(ToOwned::to_owned).collect();
    let tools_marker = lines
        .iter()
        .position(|line| line == "<tools>")
        .context("sys_prompt_template.md is missing the <tools> marker")?;

    let descriptions = tools
        .iter()
        .map(|(name, definition)| {
            let description = definition
                .get("description")
                .and_then(Value::as_str)
                .with_context(|| format!("tool {name:?} is missing a string description"))?;
            Ok(format!("{name}: {description}"))
        })
        .collect::<Result<Vec<_>>>()?
        .join("\n");

    let definitions = tools
        .iter()
        .map(|(name, definition)| {
            let yaml = serde_yaml::to_string(definition)
                .with_context(|| format!("failed to serialize tool {name:?} as YAML"))?;
            let indented_yaml = yaml
                .trim_end()
                .lines()
                .map(|line| format!("  {line}"))
                .collect::<Vec<_>>()
                .join("\n");
            Ok(format!("{name}:\n{indented_yaml}"))
        })
        .collect::<Result<Vec<_>>>()?
        .join("\n");

    lines[tools_marker + 1] = descriptions;
    let final_line = lines
        .last_mut()
        .context("sys_prompt_template.md must not be empty")?;
    *final_line = definitions;

    fs::write(&output_path, lines.join("\n"))
        .with_context(|| format!("failed to write {}", output_path.display()))
}

/// Rebuilds the prompt in this crate's copied asset directory.
pub fn build_sys_prompt() -> Result<()> {
    build_sys_prompt_in_dir(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .as_path(),
    )
}

#[cfg(test)]
mod tests {
    use super::build_sys_prompt_in_dir;
    use std::fs;

    #[test]
    fn builds_prompt_from_template_and_tools() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("sys_prompt_template.md"),
            "start\n<tools>\nplaceholder\nend\n",
        )
        .unwrap();
        fs::write(
            directory.path().join("tools.json"),
            r#"{"read":{"description":"Reads files.","params":{"path":{"mandatory":true}}}}"#,
        )
        .unwrap();

        build_sys_prompt_in_dir(directory.path()).unwrap();

        let prompt = fs::read_to_string(directory.path().join("sys_prompt.md")).unwrap();
        assert!(prompt.contains("read: Reads files."));
        assert!(prompt.contains("read:\n  description: Reads files."));
    }
}
