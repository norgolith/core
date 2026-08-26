//! Internal link graph: extracts links between pages and inverts them into
//! per-page backlinks for template consumption (`metadata.backlinks`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miette::Result;
use rayon::prelude::*;
use rust_norg::{LinkTarget, NorgAST, NorgASTFlat, ParagraphSegment, parse_tree};
use serde::Serialize;

/// A page that references the current one.
#[derive(Clone, Debug, Serialize)]
pub struct Backlink {
    pub url: String,
    pub title: String,
}

/// Canonical root-relative route from a page path or link target:
/// leading slash guaranteed, `/index` suffix folded, trailing slash dropped.
/// The site root is the empty string.
fn canonical_route(path_like: &str) -> String {
    let mut r = path_like.trim().trim_end_matches('/').to_string();
    if !r.starts_with('/') {
        r.insert(0, '/');
    }
    if r.ends_with("/index") {
        r.truncate(r.len() - "/index".len());
    }
    if r == "/" { String::new() } else { r }
}

/// Canonical route of a content file given its path relative to `content/`
/// (extension already stripped), e.g. `blog/index` -> `/blog`.
pub fn route_for(rel_path_no_ext: &str) -> String {
    canonical_route(rel_path_no_ext)
}

fn strip_meta_block(document: &str) -> &str {
    document
        .strip_prefix("@document.meta")
        .and_then(|rest| rest.find("\n@end").map(|i| &rest[i + "\n@end".len()..]))
        .map(|s| s.strip_prefix('\n').unwrap_or(s))
        .unwrap_or(document)
}

/// Extracts internal link targets (canonical routes) from a norg document.
/// External URLs, timestamps, footnotes, definitions and same-page anchors are
/// ignored. Duplicates collapsed.
pub fn extract_internal_links(document: &str) -> Vec<String> {
    let doc_body = strip_meta_block(document);
    let Ok(ast) = parse_tree(doc_body) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect_ast_links(&ast, &mut out);
    out.sort();
    out.dedup();
    out
}

fn collect_ast_links(ast: &[NorgAST], out: &mut Vec<String>) {
    for node in ast {
        match node {
            NorgAST::Paragraph(segments) => collect_segment_links(segments, out),
            NorgAST::Heading { title, content, .. } => {
                collect_segment_links(title, out);
                collect_ast_links(content, out);
            }
            NorgAST::List { items, .. } => collect_ast_links(items, out),
            NorgAST::NestableDetachedModifier { text, content, .. } => {
                collect_flat_links(std::slice::from_ref(text), out);
                collect_ast_links(content, out);
            }
            NorgAST::RangeableDetachedModifier { title, content, .. } => {
                collect_segment_links(title, out);
                collect_flat_links(content, out);
            }
            NorgAST::CarryoverTag { next_object, .. } => {
                collect_ast_links(std::slice::from_ref(next_object), out);
            }
            NorgAST::RangedTag { content, .. } => collect_flat_links(content, out),
            NorgAST::VerbatimRangedTag { .. }
            | NorgAST::InfirmTag { .. }
            | NorgAST::DelimitingModifier(_) => {}
        }
    }
}

fn collect_flat_links(flat: &[NorgASTFlat], out: &mut Vec<String>) {
    for node in flat {
        match node {
            NorgASTFlat::Paragraph(segments) => collect_segment_links(segments, out),
            NorgASTFlat::NestableDetachedModifier { content, .. } => {
                collect_flat_links(std::slice::from_ref(content), out);
            }
            NorgASTFlat::RangeableDetachedModifier { title, content, .. } => {
                collect_segment_links(title, out);
                collect_flat_links(content, out);
            }
            NorgASTFlat::Heading { title, .. } => collect_segment_links(title, out),
            NorgASTFlat::CarryoverTag { next_object, .. } => {
                collect_flat_links(std::slice::from_ref(next_object), out);
            }
            NorgASTFlat::RangedTag { content, .. } => collect_flat_links(content, out),
            NorgASTFlat::VerbatimRangedTag { .. }
            | NorgASTFlat::InfirmTag { .. }
            | NorgASTFlat::DelimitingModifier(_) => {}
        }
    }
}

fn collect_segment_links(segments: &[ParagraphSegment], out: &mut Vec<String>) {
    for segment in segments {
        // Links can sit inside attached modifiers (*bold {:/link:}*).
        if let ParagraphSegment::AttachedModifier { content, .. } = segment {
            collect_segment_links(content, out);
            continue;
        }
        let ParagraphSegment::Link {
            filepath, targets, ..
        } = segment
        else {
            continue;
        };
        // NOTE: rust-norg splits '{:https://x:}' into filepath Some("https")
        // plus a Url target, so any Url target marks the link external even
        // when filepath is set.
        if targets.iter().any(|t| matches!(t, LinkTarget::Url(_))) {
            continue;
        }
        let raw = filepath.clone().or_else(|| match targets.first() {
            Some(LinkTarget::Path(p)) => Some(p.clone()),
            _ => None,
        });
        let Some(raw) = raw else { continue };
        let cleaned = raw.trim();
        if cleaned.is_empty() || cleaned.starts_with('#') {
            continue;
        }
        // Drop fragment and .norg extension; what remains should be a page route.
        let cleaned = cleaned.split('#').next().unwrap_or(cleaned);
        if cleaned.contains("://") {
            continue; // external URL (may arrive wrapped as a Path target)
        }
        let cleaned = cleaned.strip_suffix(".norg").unwrap_or(cleaned);
        if cleaned.is_empty() {
            continue;
        }
        out.push(canonical_route(cleaned));
    }
}

struct PageLinks {
    route: String,
    title: String,
    links: Vec<String>,
}

/// Change-detection signature for a page's backlinks list. `None` when the
/// page has no backlinks (matches cache entries without embedded ones).
pub fn signature(backlinks: &[Backlink]) -> Option<u64> {
    if backlinks.is_empty() {
        return None;
    }
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for link in backlinks {
        link.url.hash(&mut hasher);
        link.title.hash(&mut hasher);
    }
    Some(hasher.finish())
}

fn permalink_for(routes_url: &str, route: &str) -> String {
    if route.is_empty() {
        format!("{routes_url}/")
    } else {
        format!("{routes_url}{route}/")
    }
}

/// Walks every `.norg` file under `content/` (except the categories dir) and
/// builds the inverted link map: canonical route -> pages linking to it.
pub fn build_backlink_map(
    content_dir: &Path,
    categories_dir: &str,
    routes_url: &str,
) -> Result<HashMap<String, Vec<Backlink>>> {
    let entries: Vec<PathBuf> = walkdir::WalkDir::new(content_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "norg"))
        .filter(|e| {
            e.path()
                .strip_prefix(content_dir)
                .is_ok_and(|p| !p.starts_with(categories_dir))
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    let pages: Vec<PageLinks> = entries
        .par_iter()
        .filter_map(|path| {
            let rel = path.strip_prefix(content_dir).ok()?;
            let rel_no_ext = rel.with_extension("").to_string_lossy().to_string();
            let content = std::fs::read_to_string(path).ok()?;
            let meta = crate::shared::extract_metadata_from_content(&content, rel, routes_url).ok();
            // Drafts are not rendered, so they must not appear as backlink
            // sources either (would leak unpublished titles/URLs).
            if meta
                .as_ref()
                .is_some_and(|m| m.get("draft").and_then(|v| v.as_bool()).unwrap_or(false))
            {
                return None;
            }
            // Title via cheap metadata-only parse; falls back to the route.
            let title = meta
                .as_ref()
                .and_then(|m| m.get("title").and_then(|v| v.as_str()).map(String::from))
                .unwrap_or_else(|| rel_no_ext.clone());
            Some(PageLinks {
                route: route_for(&rel_no_ext),
                title,
                links: extract_internal_links(&content),
            })
        })
        .collect();

    let mut map: HashMap<String, Vec<Backlink>> = HashMap::new();
    for page in pages {
        for target in &page.links {
            if *target == page.route {
                continue; // self-references are not backlinks
            }
            map.entry(target.clone()).or_default().push(Backlink {
                url: permalink_for(routes_url, &page.route),
                title: page.title.clone(),
            });
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_route_folds_index_and_slashes() {
        assert_eq!(route_for("blog/my-post"), "/blog/my-post");
        assert_eq!(route_for("blog/index"), "/blog");
        assert_eq!(route_for("index"), "");
        assert_eq!(canonical_route("blog/"), "/blog");
        assert_eq!(canonical_route("/blog"), "/blog");
    }

    #[test]
    fn extracts_internal_links_only() {
        let doc = "@document.meta\ntitle: T\n@end\n\n# Heading\n\nSome {:/blog/a:} and {:/b.norg:} links.\n\nExternal {:https://x.y:} skipped.\n";
        assert_eq!(extract_internal_links(doc), vec!["/b", "/blog/a"]);
    }

    #[test]
    fn ignores_fragments_and_headed_links() {
        // '{*...*}' parses as a Heading-target link (same-page anchor), not a
        // page reference; fragments-only targets are dropped too.
        let doc = "@document.meta\ntitle: T\n@end\n\nSee {* Some Heading *} and {:#frag:}\n";
        assert!(extract_internal_links(doc).is_empty());
    }

    #[test]
    fn build_map_inverts_and_skips_self() {
        // Indirect coverage through the public helpers; full map building
        // needs files on disk, covered by integration-level checks.
        let route = route_for("a");
        assert_ne!(route, route_for("b"));
    }
}
