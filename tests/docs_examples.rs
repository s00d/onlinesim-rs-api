//! Compiles the Rust examples of README.md and docs/content/*.md against the crate, so the docs cannot drift
//! from the API. A ```rust block counts as an example when it uses `client`, starts with `use` or has a
//! `fn main`; the other rust blocks are signatures. Examples with `fn main` or `#[tokio::test]` become a
//! module; the others become the body of
//! `async fn(client: &Client, tzid: i64) -> Result<()>`.
//!
//! Slow (builds a separate crate), so ignored by default: `cargo test --test docs_examples -- --ignored`.

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

fn examples(root: &Path) -> Vec<(String, String)> {
    let mut sources = vec![root.join("README.md")];
    let mut pages: Vec<_> = std::fs::read_dir(root.join("docs/content"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    pages.sort();
    sources.extend(pages);

    let mut out = Vec::new();
    for source in sources {
        let md = std::fs::read_to_string(&source).unwrap();
        let name = source.file_name().unwrap().to_string_lossy().to_string();
        let mut lines = md.lines();
        while let Some(line) = lines.next() {
            if line.trim() != "```rust" {
                continue;
            }
            let code: Vec<&str> = lines.by_ref().take_while(|l| l.trim() != "```").collect();
            let code = code.join("\n");
            let is_example = code.contains("client")
                || code.trim_start().starts_with("use ")
                || code.contains("fn main");
            if is_example {
                out.push((name.clone(), code));
            }
        }
    }
    out
}

#[test]
#[ignore = "builds a separate crate; run with --ignored"]
fn docs_examples_compile() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let examples = examples(root);
    assert!(
        examples.len() > 25,
        "found only {} examples",
        examples.len()
    );

    let mut lib = String::from(
        "#![allow(unused, clippy::all)]\nuse onlinesim_rs_api::*;\nuse onlinesim_rs_api::types::*;\nuse std::collections::HashMap;\n",
    );
    for (i, (source, code)) in examples.iter().enumerate() {
        if code.contains("fn main") || code.contains("#[tokio::test]") {
            writeln!(lib, "// {source}\nmod ex_{i} {{\nuse super::*;\n{code}\n}}").unwrap();
        } else {
            writeln!(
                lib,
                "// {source}\nmod ex_{i} {{\nuse super::*;\npub async fn run(client: &Client, tzid: i64) -> Result<()> {{\n{code}\nOk(())\n}}\n}}"
            )
            .unwrap();
        }
    }

    let dir = root.join("target/docs-examples");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("lib.rs"), lib).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "docs-examples"
version = "0.0.0"
edition = "2024"
publish = false

[lib]
path = "lib.rs"

[dependencies]
onlinesim-rs-api = {{ path = {root:?}, features = ["async", "blocking", "mock-transport"] }}
serde_json = "1"
tokio = {{ version = "1", features = ["macros", "rt-multi-thread"] }}

[workspace]
"#
        ),
    )
    .unwrap();
    if root.join("Cargo.lock").exists() {
        std::fs::copy(root.join("Cargo.lock"), dir.join("Cargo.lock")).unwrap();
    }

    let output = Command::new(env!("CARGO"))
        .args(["check", "--quiet", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", dir.join("target"))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "docs examples do not compile:\n{stderr}"
    );
}
