use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=../dist");
    println!("cargo:rerun-if-changed=../bin");

    if let Ok(profile) = std::env::var("PROFILE") {
        let target_dir = Path::new("target").join(&profile);
        let _ = fs::create_dir_all(&target_dir);
        if target_dir.exists() {
            if let Ok(entries) = fs::read_dir("../bin") {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("dll") {
                        if let Some(file_name) = path.file_name() {
                            let dest = target_dir.join(file_name);
                            let _ = fs::copy(&path, &dest);
                        }
                    }
                }
            }
        }
    }

    tauri_build::build()
}
