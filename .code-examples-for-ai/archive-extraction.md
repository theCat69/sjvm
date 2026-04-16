<!-- Demonstrates secure archive extraction: sanitize names, restrict formats, validate destination -->

```rust
// src/core/downloader.rs (excerpt)
let safe_name = Path::new(&request.artifact.filename)
    .file_name()
    .and_then(|n| n.to_str())
    .filter(|n| !n.contains('/') && !n.contains('\\') && !n.contains('\0'))
    .with_context(|| format!("Invalid artifact filename: {:?}", request.artifact.filename))?;

let jdk_dir_name = std::path::Path::new(safe_name)
    .file_stem()
    .and_then(|s| s.to_str())
    .map(|s| s.trim_end_matches(".tar"))
    .map(str::to_owned)
    .context("Cannot derive JDK directory name from filename")?;

let final_dest = request.dest_dir.join(&jdk_dir_name);

let extract_result = if request.artifact.filename.ends_with(".tar.gz") {
    extract_tar_gz(&temp_path, &temp_extract_dir)
} else if request.artifact.filename.ends_with(".zip") {
    extract_zip(&temp_path, &temp_extract_dir)
} else {
    bail!("Unsupported archive format: {}", request.artifact.filename);
};

let top_level = identify_top_level_dir(&temp_extract_dir)?;
validate_dest_within_jdks_dir(&final_dest, &request.dest_dir)?;
```
