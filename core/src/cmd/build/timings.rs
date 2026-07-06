use colored::Colorize;

pub(super) struct BuildTimings {
    pub config_ms: u128,
    pub tera_ms: u128,
    pub plugins_ms: u128,
    pub prepare_dir_ms: u128,
    pub collect_posts_ms: u128,
    pub collections_ms: u128,
    pub shared_ctx_ms: u128,
    pub cache_open_ms: u128,
    pub content_ms: u128,
    pub categories_ms: u128,
    pub feeds_ms: u128,
    pub seo_ms: u128,
    pub assets_ms: u128,
    pub error_pages_ms: u128,
    pub cache_save_ms: u128,
    pub page_file_ms: u128,
    pub page_meta_ms: u128,
    pub page_schema_ms: u128,
    pub page_draft_ms: u128,
    pub page_cache_get_ms: u128,
    pub page_load_ms: u128,
    pub page_render_ms: u128,
    pub page_href_ms: u128,
    pub page_minify_ms: u128,
    pub page_write_ms: u128,
    pub page_count: usize,
}

impl BuildTimings {
    pub fn new() -> Self {
        Self {
            config_ms: 0,
            tera_ms: 0,
            plugins_ms: 0,
            prepare_dir_ms: 0,
            collect_posts_ms: 0,
            collections_ms: 0,
            shared_ctx_ms: 0,
            cache_open_ms: 0,
            content_ms: 0,
            categories_ms: 0,
            feeds_ms: 0,
            seo_ms: 0,
            assets_ms: 0,
            error_pages_ms: 0,
            cache_save_ms: 0,
            page_file_ms: 0,
            page_meta_ms: 0,
            page_schema_ms: 0,
            page_draft_ms: 0,
            page_cache_get_ms: 0,
            page_load_ms: 0,
            page_render_ms: 0,
            page_href_ms: 0,
            page_minify_ms: 0,
            page_write_ms: 0,
            page_count: 0,
        }
    }

    pub fn print_summary(&self, total_ms: u128) {
        let overhead = total_ms
            .saturating_sub(self.config_ms)
            .saturating_sub(self.tera_ms)
            .saturating_sub(self.prepare_dir_ms)
            .saturating_sub(self.collect_posts_ms)
            .saturating_sub(self.collections_ms)
            .saturating_sub(self.shared_ctx_ms)
            .saturating_sub(self.cache_open_ms)
            .saturating_sub(self.content_ms)
            .saturating_sub(self.categories_ms)
            .saturating_sub(self.feeds_ms)
            .saturating_sub(self.seo_ms)
            .saturating_sub(self.assets_ms)
            .saturating_sub(self.error_pages_ms)
            .saturating_sub(self.cache_save_ms);

        println!();
        println!("{}", "=== Build Timing Breakdown ===".bold());
        println!(
            "  {:<30} {:>6}ms  ({:>4.1}%)",
            "Config load+validate",
            self.config_ms,
            pct(self.config_ms, total_ms)
        );
        println!(
            "  {:<30} {:>6}ms  ({:>4.1}%)",
            "Tera init",
            self.tera_ms,
            pct(self.tera_ms, total_ms)
        );
        println!(
            "  {:<30} {:>6}ms  ({:>4.1}%)",
            "Prepare build dir",
            self.prepare_dir_ms,
            pct(self.prepare_dir_ms, total_ms)
        );
        println!(
            "  {:<30} {:>6}ms  ({:>4.1}%)",
            "Collect post metadata",
            self.collect_posts_ms,
            pct(self.collect_posts_ms, total_ms)
        );
        println!(
            "  {:<30} {:>6}ms  ({:>4.1}%)",
            "Collection subsets",
            self.collections_ms,
            pct(self.collections_ms, total_ms)
        );
        println!(
            "  {:<30} {:>6}ms  ({:>4.1}%)",
            "Shared context",
            self.shared_ctx_ms,
            pct(self.shared_ctx_ms, total_ms)
        );
        println!(
            "  {:<30} {:>6}ms  ({:>4.1}%)",
            "Cache open",
            self.cache_open_ms,
            pct(self.cache_open_ms, total_ms)
        );
        println!(
            "  {:<30} {:>6}ms  ({:>4.1}%)",
            "Content build (all pages)",
            self.content_ms,
            pct(self.content_ms, total_ms)
        );
        println!(
            "  {:<30} {:>6}ms  ({:>4.1}%)",
            "Category pages",
            self.categories_ms,
            pct(self.categories_ms, total_ms)
        );
        println!(
            "  {:<30} {:>6}ms  ({:>4.1}%)",
            "XML feeds",
            self.feeds_ms,
            pct(self.feeds_ms, total_ms)
        );
        println!(
            "  {:<30} {:>6}ms  ({:>4.1}%)",
            "SEO (sitemap+robots)",
            self.seo_ms,
            pct(self.seo_ms, total_ms)
        );
        println!(
            "  {:<30} {:>6}ms  ({:>4.1}%)",
            "Asset copy",
            self.assets_ms,
            pct(self.assets_ms, total_ms)
        );
        println!(
            "  {:<30} {:>6}ms  ({:>4.1}%)",
            "Error pages",
            self.error_pages_ms,
            pct(self.error_pages_ms, total_ms)
        );
        println!(
            "  {:<30} {:>6}ms  ({:>4.1}%)",
            "Cache save",
            self.cache_save_ms,
            pct(self.cache_save_ms, total_ms)
        );
        println!(
            "  {:<30} {:>6}ms  ({:>4.1}%)",
            "Overhead/other",
            overhead,
            pct(overhead, total_ms)
        );
        println!("  {}", "─".repeat(50));
        println!("  {:<30} {:>6}ms", "TOTAL", total_ms);

        if self.page_count > 0 {
            println!();
            println!(
                "{}",
                "=== Per-Page Sub-Timing (sums across all pages) ===".bold()
            );
            println!(
                "  {:<30} {:>6}ms  (avg {:>4.1}ms)",
                "File read (I/O)",
                self.page_file_ms,
                avg(self.page_file_ms, self.page_count)
            );
            println!(
                "  {:<30} {:>6}ms  (avg {:>4.1}ms)",
                "Metadata extract",
                self.page_meta_ms,
                avg(self.page_meta_ms, self.page_count)
            );
            println!(
                "  {:<30} {:>6}ms  (avg {:>4.1}ms)",
                "Schema validation",
                self.page_schema_ms,
                avg(self.page_schema_ms, self.page_count)
            );
            println!(
                "  {:<30} {:>6}ms  (avg {:>4.1}ms)",
                "Draft check",
                self.page_draft_ms,
                avg(self.page_draft_ms, self.page_count)
            );
            println!(
                "  {:<30} {:>6}ms  (avg {:>4.1}ms)",
                "Cache lock get",
                self.page_cache_get_ms,
                avg(self.page_cache_get_ms, self.page_count)
            );
            println!(
                "  {:<30} {:>6}ms  (avg {:>4.1}ms)",
                "Parse+HTML convert",
                self.page_load_ms,
                avg(self.page_load_ms, self.page_count)
            );
            println!(
                "  {:<30} {:>6}ms  (avg {:>4.1}ms)",
                "Template render",
                self.page_render_ms,
                avg(self.page_render_ms, self.page_count)
            );
            println!(
                "  {:<30} {:>6}ms  (avg {:>4.1}ms)",
                "Href rewrite",
                self.page_href_ms,
                avg(self.page_href_ms, self.page_count)
            );
            println!(
                "  {:<30} {:>6}ms  (avg {:>4.1}ms)",
                "HTML minify",
                self.page_minify_ms,
                avg(self.page_minify_ms, self.page_count)
            );
            println!(
                "  {:<30} {:>6}ms  (avg {:>4.1}ms)",
                "File write",
                self.page_write_ms,
                avg(self.page_write_ms, self.page_count)
            );
            let page_sum = self.page_file_ms
                + self.page_meta_ms
                + self.page_schema_ms
                + self.page_draft_ms
                + self.page_cache_get_ms
                + self.page_load_ms
                + self.page_render_ms
                + self.page_href_ms
                + self.page_minify_ms
                + self.page_write_ms;
            println!("  {}", "─".repeat(50));
            println!(
                "  {:<30} {:>6}ms  (sum of above)",
                "Page sub-timing sum", page_sum
            );
            println!(
                "  {:<30} {:>6}ms  (content_ms - page sub-sum)",
                "Lock/scheduling gap",
                self.content_ms.saturating_sub(page_sum)
            );
        }
    }
}

pub(super) fn pct(ms: u128, total: u128) -> f64 {
    if total == 0 {
        0.0
    } else {
        ms as f64 / total as f64 * 100.0
    }
}
pub(super) fn avg(ms: u128, n: usize) -> f64 {
    if n == 0 { 0.0 } else { ms as f64 / n as f64 }
}
