use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let repo_root = Path::new(&manifest_dir).join("..");
    let canonical_root = repo_root.canonicalize().unwrap();
    println!("cargo:rustc-env=NORGOLITH_ROOT={}", canonical_root.display());

    let docs_dir = repo_root.join("docs/content");
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("docs.rs");

    let mut entries = vec![];

    // Doc resources
    if docs_dir.exists() {
        let docs_sub = docs_dir.join("docs");
        if docs_sub.exists() {
            walk_dir(&docs_sub, &docs_sub, "docs", &mut entries);
        }
        let index = docs_dir.join("index.norg");
        if index.exists() {
            let abs = index.to_string_lossy().to_string();
            entries.push(("index".into(), "norgolith://index".into(), abs));
            println!("cargo:rerun-if-changed={}", index.display());
        }
    }

    // Source resources for plugin development
    let source_dirs = [
        ("sdk", repo_root.join("sdk/src")),
        ("core/plugin", repo_root.join("core/src/plugin")),
    ];
    for (prefix, dir) in &source_dirs {
        if dir.exists() {
            walk_source(dir, dir, prefix, &mut entries);
        }
    }

    for (_, _, abs_path) in &entries {
        println!("cargo:rerun-if-changed={}", abs_path);
    }

    let mut code = String::from(
        "#[derive(Clone, Copy)]\n\
         pub struct DocEntry {\n\
         \tpub name: &'static str,\n\
         \tpub uri: &'static str,\n\
         \tpub content: &'static str,\n\
         }\n\n\
         pub const DOC_ENTRIES: &[DocEntry] = &[\n",
    );

    for (name, uri, abs_path) in &entries {
        let escaped = abs_path.replace('\\', "\\\\");
        code.push_str(&format!(
            "\tDocEntry {{ name: {name:?}, uri: {uri:?}, content: include_str!({escaped:?}) }},\n",
        ));
    }

    code.push_str("];\n");

    fs::write(&dest, &code).unwrap();
}

fn walk_dir(base: &Path, dir: &Path, prefix: &str, entries: &mut Vec<(String, String, String)>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            walk_dir(base, &path, prefix, entries);
        } else if path.extension().and_then(|s| s.to_str()) == Some("norg") {
            let rel = path
                .strip_prefix(base)
                .unwrap()
                .with_extension("");
            let rel_str = rel.to_string_lossy().to_string();
            let name = format!("{}-{}", prefix, rel_str.replace('/', "-"));
            let uri = format!("norgolith://{}/{}", prefix, rel_str);
            let abs_path = path.to_string_lossy().to_string();
            entries.push((name, uri, abs_path));
        }
    }
}

fn walk_source(base: &Path, dir: &Path, prefix: &str, entries: &mut Vec<(String, String, String)>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            walk_source(base, &path, prefix, entries);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            let rel = path.strip_prefix(base).unwrap();
            let rel_str = rel.to_string_lossy().to_string();
            let name = format!("{}-{}", prefix, rel_str.replace('/', "-").replace(".rs", ""));
            let uri = format!("norgolith://src/{}/{}", prefix, rel_str);
            let abs_path = path.to_string_lossy().to_string();
            entries.push((name, uri, abs_path));
        }
    }
}
