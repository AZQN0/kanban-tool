use anyhow::Result;
use std::path::PathBuf;

pub fn run(project_path: PathBuf) -> Result<()> {
    // Stub — real implementation in Task 3
    let _ = project_path;
    println!("WebUI stub — not yet implemented. Build with --features webui for full server.");
    Ok(())
}
