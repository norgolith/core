use miette::Result;
use tera::Context;
use tracing::{debug, warn};

/// Markers used by the converter to wrap @embed html content.
const MARKER_OPEN: &str = "<!--lith:embed-->";
const MARKER_CLOSE: &str = "<!--/lith:embed-->";

/// Renders any shortcode component calls within @embed html islands.
///
/// Each island is a block of HTML containing Tera v2 component syntax
/// (e.g. `{{ <tweet id="x" /> }}`), wrapped by the converter with
/// `<!--lith:embed-->...<!--/lith:embed-->` markers.
///
/// The function extracts each island, passes it through Tera's `render_str`
/// with autoescape disabled, and splices the result back into the HTML.
/// Output is never re-scanned - single pass guarantees no recursion.
pub fn process(html: &str, tera: &tera::Tera, context: &Context) -> Result<String> {
    if !html.contains(MARKER_OPEN) {
        return Ok(html.to_string());
    }

    let mut result = String::with_capacity(html.len());
    let mut cursor = 0;

    while let Some(open_pos) = html[cursor..].find(MARKER_OPEN) {
        let abs_open = cursor + open_pos;
        // Write everything before this marker
        result.push_str(&html[cursor..abs_open]);

        let content_start = abs_open + MARKER_OPEN.len();

        match html[content_start..].find(MARKER_CLOSE) {
            Some(close_offset) => {
                let content_end = content_start + close_offset;
                let island = &html[content_start..content_end];

                // Only render through Tera if the island contains component syntax
                if island.contains("{{ <") || island.contains("{% <") {
                    match tera.render_str(island, context, false) {
                        Ok(rendered) => result.push_str(&rendered),
                        Err(e) => {
                            warn!("Shortcode render error: {}", e);
                            // Fall back to raw island content
                            result.push_str(island);
                        }
                    }
                } else {
                    // No component calls - pass through as-is
                    result.push_str(island);
                }

                cursor = content_end + MARKER_CLOSE.len();
            }
            None => {
                // Unclosed marker - pass through rest as-is
                debug!("Unclosed lith:embed marker at offset {}", abs_open);
                result.push_str(&html[abs_open..]);
                cursor = html.len();
                break;
            }
        }
    }

    // Write any remaining content after last marker
    if cursor < html.len() {
        result.push_str(&html[cursor..]);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_markers_passthrough() {
        let html = "<p>Hello world</p>";
        let tera = tera::Tera::default();
        let ctx = Context::new();
        assert_eq!(process(html, &tera, &ctx).unwrap(), html);
    }

    #[test]
    fn empty_markers() {
        let html = "<p>before</p><!--lith:embed--><!--/lith:embed--><p>after</p>";
        let tera = tera::Tera::default();
        let ctx = Context::new();
        assert_eq!(
            process(html, &tera, &ctx).unwrap(),
            "<p>before</p><p>after</p>"
        );
    }

    #[test]
    fn island_passthrough_no_component() {
        let html = "<p>before</p><!--lith:embed--><b>bold</b><!--/lith:embed--><p>after</p>";
        let tera = tera::Tera::default();
        let ctx = Context::new();
        assert_eq!(
            process(html, &tera, &ctx).unwrap(),
            "<p>before</p><b>bold</b><p>after</p>"
        );
    }

    #[test]
    fn unclosed_marker_passthrough() {
        let html = "<p>before</p><!--lith:embed--><b>unclosed</b><p>after</p>";
        let tera = tera::Tera::default();
        let ctx = Context::new();
        assert_eq!(
            process(html, &tera, &ctx).unwrap(),
            "<p>before</p><!--lith:embed--><b>unclosed</b><p>after</p>"
        );
    }

    #[test]
    fn multiple_islands() {
        let html = "<!--lith:embed--><b>one</b><!--/lith:embed--><p>middle</p><!--lith:embed--><i>two</i><!--/lith:embed-->";
        let tera = tera::Tera::default();
        let ctx = Context::new();
        assert_eq!(
            process(html, &tera, &ctx).unwrap(),
            "<b>one</b><p>middle</p><i>two</i>"
        );
    }

    #[test]
    fn component_renders_inside_island() {
        let mut tera = tera::Tera::default();
        tera.add_raw_template(
            "shortcodes/greet.html",
            "{% component greet(name: String) %}<span>Hello {{ name }}!</span>{% endcomponent greet %}",
        )
        .unwrap();
        let mut ctx = Context::new();
        ctx.insert("name", &"World");
        let html = "<p>before</p><!--lith:embed-->{{ <greet name=\"World\" /> }}<!--/lith:embed--><p>after</p>";
        assert_eq!(
            process(html, &tera, &ctx).unwrap(),
            "<p>before</p><span>Hello World!</span><p>after</p>"
        );
    }

    #[test]
    fn component_with_kwargs_from_context() {
        let mut tera = tera::Tera::default();
        tera.add_raw_template(
            "shortcodes/alert.html",
            "{% component alert(msg: String, level=\"info\") %}<div class=\"{{ level }}\">{{ msg }}</div>{% endcomponent alert %}",
        )
        .unwrap();
        let mut ctx = Context::new();
        ctx.insert("msg", &"Something happened");
        let html = "<!--lith:embed-->{{ <alert msg=\"Something happened\" level=\"warning\" /> }}<!--/lith:embed-->";
        assert_eq!(
            process(html, &tera, &ctx).unwrap(),
            "<div class=\"warning\">Something happened</div>"
        );
    }

    #[test]
    fn component_with_body() {
        let mut tera = tera::Tera::default();
        tera.add_raw_template(
            "shortcodes/callout.html",
            "{% component callout(type: String) %}<aside class=\"{{ type }}\">{{ body }}</aside>{% endcomponent callout %}",
        )
        .unwrap();
        let ctx = Context::new();
        let html = "<!--lith:embed-->{% <callout type=\"warning\"> %}Be careful!{% </callout> %}<!--/lith:embed-->";
        assert_eq!(
            process(html, &tera, &ctx).unwrap(),
            "<aside class=\"warning\">Be careful!</aside>"
        );
    }
}
