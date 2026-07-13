use std::sync::Arc;
use std::time::Duration;

use miette::{IntoDiagnostic, Result};
use futures_util::Stream;
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};
use tokio::runtime::Handle;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error, info, instrument};

use crate::fs;

use super::server::ServerState;

#[derive(Default, Debug, Clone)]
pub(super) struct FileActions {
    pub reload_templates: bool,
    pub reload_assets: bool,
    pub reload_content: bool,
    pub reload_config: bool,
}

fn is_relevant_event(event: &notify::Event) -> bool {
    matches!(
        event.kind,
        notify::EventKind::Create(_)
            | notify::EventKind::Remove(_)
            | notify::EventKind::Modify(notify::event::ModifyKind::Data(_))
    )
}

#[instrument(level = "debug", skip(event))]
async fn is_template_change(event: &notify::Event) -> bool {
    let Some(path) = event.paths.first() else {
        return false;
    };
    let is_template = path
        .extension()
        .is_some_and(|ext| ext == "html" || ext == "xml");
    let Some(parent_dir) = path.parent() else {
        return false;
    };

    is_relevant_event(event)
        && is_template
        && fs::find_in_previous_dirs("dir", "templates", &mut parent_dir.to_path_buf()).is_ok()
}

#[instrument(level = "debug", skip(event))]
async fn is_content_change(event: &notify::Event) -> bool {
    let Some(path) = event.paths.first() else {
        return false;
    };
    let Some(parent_dir) = path.parent() else {
        return false;
    };

    is_relevant_event(event)
        && fs::find_in_previous_dirs("dir", "content", &mut parent_dir.to_path_buf()).is_ok()
}

#[instrument(level = "debug", skip(event))]
async fn is_asset_change(event: &notify::Event) -> bool {
    let Some(path) = event.paths.first() else {
        return false;
    };
    let Some(parent_dir) = path.parent() else {
        return false;
    };

    // FIXME: find from given path instead of traversing file system
    is_relevant_event(event)
        && fs::find_in_previous_dirs("dir", "assets", &mut parent_dir.to_path_buf()).is_ok()
}

#[instrument(name = "watcher", skip(result, state))]
pub(super) async fn process_debounced_events(result: DebounceEventResult, state: Arc<ServerState>) {
    match result {
        DebounceEventResult::Ok(events) => {
            debug!("Processing {} file events", events.len());
            handle_file_events(events, state).await
        }
        DebounceEventResult::Err(errors) => {
            error!("Watcher errors: {:?}", errors);
        }
    }
}

#[instrument(level = "debug", skip(actions, state))]
async fn execute_actions(actions: FileActions, state: Arc<ServerState>) {
    debug!(
        "Executing actions: templates={}, assets={}, reload={}, config={}",
        actions.reload_templates,
        actions.reload_assets,
        actions.reload_content,
        actions.reload_config,
    );

    use crate::shared;

    // Config reload supersedes content/template/asset reloads since it re-collects posts too
    if actions.reload_config {
        match state.reload_config().await {
            Ok(_) => {}
            Err(e) => error!("Config reload failed: {}", e),
        }
        state.rebuild_rendered_pages().await;
        return;
    }

    // Handle asset reloads
    if actions.reload_assets
        && let Err(e) = state.send_reload()
    {
        error!("Asset reload error: {}", e);
    }

    // Handle template reloads
    if actions.reload_templates {
        match state.reload_templates().await {
            Ok(_) => {
                state.rebuild_rendered_pages().await;
                if let Err(e) = state.send_reload() {
                    error!("Template reload signal error: {}", e);
                }
            }
            Err(e) => error!("Template reload failed: {}", e),
        }
    }

    if actions.reload_content {
        let collections = state.config.read().await.collections.clone();
        match shared::collect_all_posts_metadata(
            &state.paths.content,
            &state.routes_url,
            &collections,
        ) {
            Ok(new_posts) => {
                let mut posts_lock = state.posts.write().await;
                *posts_lock = new_posts;
            }
            Err(e) => error!("Failed to update pages metadata: {}", e),
        }

        state.rebuild_rendered_pages().await;

        if let Err(e) = state.send_reload() {
            error!("Reload signal error: {}", e);
        }
    }
}

async fn handle_file_events(
    events: Vec<notify_debouncer_full::DebouncedEvent>,
    state: Arc<ServerState>,
) {
    let mut actions = FileActions::default();

    for event in events {
        if let Some(path) = event.paths.first() {
            handle_single_event(&event, path, &mut actions, &state).await;
        }
    }

    execute_actions(actions, state).await;
}

#[instrument(level = "debug", skip(event, path, actions, state))]
async fn handle_single_event(
    event: &notify::Event,
    path: &std::path::Path,
    actions: &mut FileActions,
    state: &Arc<ServerState>,
) {
    if !is_relevant_event(event) {
        return;
    }
    debug!(event = ?event.kind, path = %path.display(), "Processing file event");

    // Exclude temp (Neo)vim backup files
    if path.to_string_lossy().ends_with('~') {
        debug!("Ignoring temporary editor backup file");
        return;
    }

    if path == state.paths.config_file {
        info!("Config modified: norgolith.toml");
        actions.reload_config = true;
        return;
    }

    if is_template_change(event).await {
        if let Ok(rel) = path.strip_prefix(&state.paths.theme_templates) {
            info!("Template modified: {}", rel.display());
            actions.reload_templates = true;
        } else if let Ok(rel) = path.strip_prefix(&state.paths.templates) {
            info!("Template modified: {}", rel.display());
            actions.reload_templates = true;
        }
    }

    if is_asset_change(event).await {
        if let Ok(rel) = path.strip_prefix(&state.paths.theme_assets) {
            info!("Asset modified: {}", rel.display());
            actions.reload_assets = true;
        } else if let Ok(rel) = path.strip_prefix(&state.paths.assets) {
            info!("Asset modified: {}", rel.display());
            actions.reload_assets = true;
        }
    }

    debug!(?actions.reload_content, "reload_content");
    if !actions.reload_content
        && is_content_change(event).await
        && path.strip_prefix(&state.paths.content).is_ok()
    {
        debug!(path = %path.display(), "Content modified");
        actions.reload_content = true;
    }
}

#[instrument(skip(state, rt))]
pub(super) async fn setup_file_watcher(
    state: Arc<ServerState>,
    rt: Handle,
) -> Result<(
    Debouncer<RecommendedWatcher, RecommendedCache>,
    impl Stream<Item = DebounceEventResult>,
)> {
    debug!("Setting up file watcher");

    let (debouncer_tx, debouncer_rx) = tokio::sync::mpsc::channel(16);

    let mut debouncer: Debouncer<RecommendedWatcher, RecommendedCache> = new_debouncer(
        Duration::from_millis(200),
        None,
        move |result: DebounceEventResult| {
            let tx = debouncer_tx.clone();
            rt.spawn(async move {
                if let Err(e) = tx.send(result).await {
                    error!("Debouncer error: {:?}", e);
                }
            });
        },
    ).into_diagnostic()?;

    debouncer.watch(&state.paths.config_file, RecursiveMode::NonRecursive).into_diagnostic()?;
    debouncer.watch(&state.paths.templates, RecursiveMode::Recursive).into_diagnostic()?;
    debouncer.watch(&state.paths.content, RecursiveMode::Recursive).into_diagnostic()?;
    debouncer.watch(&state.paths.assets, RecursiveMode::Recursive).into_diagnostic()?;
    if state.paths.theme_assets.exists() {
        debouncer.watch(&state.paths.theme_assets, RecursiveMode::Recursive).into_diagnostic()?;
    }
    if state.paths.theme_templates.exists() {
        debouncer.watch(&state.paths.theme_templates, RecursiveMode::Recursive).into_diagnostic()?;
    }

    Ok((debouncer, ReceiverStream::new(debouncer_rx)))
}
