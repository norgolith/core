use std::convert::Infallible;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use colored::Colorize;
use eyre::{Result, eyre};
use futures_util::{SinkExt, StreamExt};
use hyper::header::{CACHE_CONTROL, EXPIRES, PRAGMA};
use hyper::{Body, Request, Response, StatusCode, header::CONTENT_TYPE};
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tracing::{debug, error, instrument, warn};

use crate::shared;

use super::server::ServerState;

const LIVE_RELOAD_SCRIPT: &str = include_str!("../../resources/assets/livereload.js");
const LIVE_RELOAD_PORT: u16 = 35729;
const WS_HELLO_MESSAGE: &str = r#"{"command":"hello","protocols":["http://livereload.com/protocols/official-7"],"serverName":"norgolith"}"#;
const WS_RELOAD_MESSAGE: &str = r#"{"command":"reload","path":"/"}"#;

async fn fast_path_lookup(state: &ServerState, url_path: &str) -> Option<String> {
    let pages = state.rendered_pages.read().await;
    pages.get(url_path).cloned()
}

pub(super) fn rewrite_urls(body: String, root_url: &str, routes_url: &str) -> String {
    body.replace(&root_url.replace("://", ":&#x2F;&#x2F;"), routes_url)
}

fn html_response(body: String, status: StatusCode) -> Result<Response<Body>> {
    Ok(Response::builder()
        .header(CONTENT_TYPE, "text/html; charset=utf-8")
        .status(status)
        .body(Body::from(body))?)
}

#[instrument(skip(html))]
pub(super) fn inject_livereload_script(html: &mut String) {
    debug!("Injecting LiveReload script");

    if let Some(pos) = html.rfind("</body>") {
        html.insert_str(
            pos,
            &format!(
                r#"<script src="/livereload.js?port={}&amp;mindelay=10"></script>"#,
                LIVE_RELOAD_PORT
            ),
        );
    }
}

#[instrument(skip(path))]
pub(super) async fn read_asset(path: &Path) -> Result<(Vec<u8>, String)> {
    debug!(path = %path.display(), "Reading asset");

    let content = tokio::fs::read(path)
        .await
        .map_err(|e| eyre!("Failed to read asset: {}", e))?;
    let mime_type = mime_guess::from_path(path)
        .first_or_octet_stream()
        .as_ref()
        .to_string();

    debug!(mime_type = %mime_type, "Determined asset MIME type");
    Ok((content, mime_type))
}

pub(super) fn handle_not_found(state: &ServerState) -> Response<Body> {
    let tera = state.tera.try_read().ok();
    let config = state.config.try_read().ok();
    if let (Some(tera), Some(config)) = (tera, config)
        && tera.get_template_names().any(|n| n == "404.html")
    {
        let posts = state.posts.try_read().ok();
        let collections = posts
            .as_ref()
            .map(|p| shared::precompute_collection_subsets(p, &config))
            .unwrap_or_default();
        let shared_context = posts
            .as_ref()
            .map(|p| shared::build_shared_context(p, &config, &collections))
            .unwrap_or_else(|| shared::build_shared_context(&[], &config, &collections));
        if let Ok(rendered) = tera.render("404.html", &shared_context) {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(CONTENT_TYPE, "text/html; charset=utf-8")
                .body(Body::from(rendered))
                .expect("Could not build Not Found response");
        }
    }
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("not found"))
        .expect("Could not build Not Found response")
}

pub(super) async fn resolve_url_norg_path(
    content_dir: &Path,
    path: &Path,
) -> std::io::Result<PathBuf> {
    use tokio::fs;
    let mut path = content_dir.join(path);
    // try "{path}.norg"
    if path.file_name().is_some() {
        let mut norg_path = path.to_string_lossy().into_owned();
        norg_path.push_str(".norg");
        let norg_path = PathBuf::from(norg_path);
        if fs::metadata(&norg_path).await.is_ok_and(|m| m.is_file()) {
            return Ok(norg_path);
        }
    }
    // try {path}/index.norg
    let metadata = fs::metadata(&path).await?;
    if metadata.is_dir() {
        path.push("index.norg");
    }
    Ok(path)
}

#[instrument(skip(request_path, paths, state))]
pub(super) async fn handle_asset(
    request_path: &str,
    paths: &crate::shared::SitePaths,
    state: &Arc<ServerState>,
) -> Result<Response<Body>> {
    let asset_path = request_path.trim_start_matches("/assets/");
    debug!(path = %asset_path, "Handling asset request");

    let site_path = paths.assets.join(asset_path);

    debug!(site_assets = %site_path.display(), "Checking site assets path");
    let (content, mime_type) = match read_asset(&site_path).await {
        Ok(asset) => {
            debug!("Asset found in site directory");
            asset
        }
        Err(_) => {
            debug!("Asset not found in site directory, checking theme assets");
            let theme_path = paths.theme_assets.join(asset_path);
            match read_asset(&theme_path).await {
                Ok(asset) => {
                    debug!("Asset found in theme directory");
                    asset
                }
                Err(_) => {
                    error!(asset_path = %request_path, "Asset not found in site or theme directories");
                    return Ok(handle_not_found(state));
                }
            }
        }
    };
    Ok(Response::builder()
        .header(CONTENT_TYPE, mime_type)
        .status(StatusCode::OK)
        .header(
            CACHE_CONTROL,
            "no-store, no-cache, must-revalidate, proxy-revalidate",
        )
        .header(PRAGMA, "no-cache")
        .header(EXPIRES, 0)
        .body(Body::from(content))?)
}

async fn handle_xml_feed(request_path: &str, state: &Arc<ServerState>) -> Result<Response<Body>> {
    let template_name = request_path.trim_start_matches('/');
    debug!(template = %template_name, "Handling XML feed request");

    // Fast path: lookup in pre-rendered memory cache
    if let Some(html) = fast_path_lookup(state, request_path).await {
        return Ok(Response::builder()
            .header(CONTENT_TYPE, "application/xml; charset=utf-8")
            .status(StatusCode::OK)
            .body(Body::from(html))?);
    }

    // Slow path: render on demand
    let tera = state.tera.read().await;
    if !tera.get_template_names().any(|n| n == template_name) {
        return Ok(handle_not_found(state));
    }

    let config = state.config.read().await.clone();
    let posts = state.posts.read().await.clone();
    let collections = shared::precompute_collection_subsets(&posts, &config);
    let shared_context = shared::build_shared_context(&posts, &config, &collections);
    let context = shared_context;

    let content = tera
        .render(template_name, &context)
        .map_err(|e| eyre!("{}: {}", "Failed to render XML feed template".bold(), e))?;

    Ok(Response::builder()
        .header(CONTENT_TYPE, "application/xml; charset=utf-8")
        .status(StatusCode::OK)
        .body(Body::from(content))?)
}

async fn handle_norg_content(path: PathBuf, state: Arc<ServerState>) -> Result<Response<Body>> {
    let rel_path = path.strip_prefix(&state.paths.content)?.to_path_buf();
    let url_path = format!("/{}", rel_path.with_extension("").display());

    // Fast path: lookup in pre-rendered memory cache
    if let Some(html) = fast_path_lookup(&state, &url_path).await {
        let mut body = html;
        inject_livereload_script(&mut body);
        return html_response(body, StatusCode::OK);
    }

    // Slow path: not in cache (e.g. file changed since last render), render on demand
    let tera = state.tera.read().await;

    let Ok(content) = tokio::fs::read_to_string(&path).await else {
        return Ok(handle_not_found(&state));
    };

    let metadata = shared::extract_metadata_from_content(&content, &rel_path, &state.routes_url);
    let is_draft = metadata
        .get("draft")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if is_draft && !state.build_drafts {
        return Ok(handle_not_found(&state));
    }

    let cache_key = rel_path.with_extension("");
    let metadata = {
        let cache_guard = state.cache.read().await;
        cache_guard.get(&cache_key, &content)
    };
    let metadata = if let Some(cached) = metadata {
        match serde_json::from_value(cached.clone()) {
            Ok(md) => md,
            Err(_) => shared::load_metadata_from_content(&content, &rel_path, &state.routes_url),
        }
    } else {
        let md = shared::load_metadata_from_content(&content, &rel_path, &state.routes_url);
        if let Ok(json_val) = serde_json::to_value(&md) {
            let mut cache_guard = state.cache.write().await;
            cache_guard.insert(&cache_key, &content, json_val);
        }
        md
    };

    let config = state.config.read().await.clone();
    let posts = state.posts.read().await.clone();
    let collections = shared::precompute_collection_subsets(&posts, &config);
    let shared_context = shared::build_shared_context(&posts, &config, &collections);
    let mut body = shared::render_norg_page(&tera, &metadata, &shared_context)?;

    body = rewrite_urls(body, &config.root_url, &state.routes_url);

    inject_livereload_script(&mut body);
    html_response(body, StatusCode::OK)
}

async fn handle_content(request_path: &str, state: Arc<ServerState>) -> Result<Response<Body>> {
    let trimmed = request_path.trim_end_matches('/');
    let req_path = PathBuf::from(trimmed.trim_start_matches('/'));
    debug!(?req_path);
    match resolve_url_norg_path(&state.paths.content, &req_path).await {
        Ok(path) => handle_norg_content(path, state).await,
        Err(io_err) => match io_err.kind() {
            std::io::ErrorKind::NotFound => Ok(handle_not_found(&state)),
            std::io::ErrorKind::PermissionDenied => Ok(Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body(Body::empty())
                .unwrap()),
            _ => Err(eyre!("Error reading '{}': {}", req_path.display(), io_err)),
        },
    }
}

async fn handle_category_index(state: &Arc<ServerState>) -> Result<Response<Body>> {
    let config = state.config.read().await.clone();
    let url_path = format!("/{}", config.categories_dir);

    // Fast path: lookup in pre-rendered memory cache
    if let Some(html) = fast_path_lookup(state, &url_path).await {
        let mut body = html;
        inject_livereload_script(&mut body);
        return html_response(body, StatusCode::OK);
    }

    // Slow path: render on demand
    let posts = state.posts.read().await.clone();
    let categories = shared::collect_all_posts_categories(&posts);
    let collections = shared::precompute_collection_subsets(&posts, &config);
    let shared_context = shared::build_shared_context(&posts, &config, &collections);
    let mut context = shared_context;
    context.insert("categories", &categories.into_iter().collect::<Vec<_>>());

    let tera = state.tera.read().await;
    let mut body = tera.render("categories.html", &context).map_err(|e| {
        if e.source().is_some() {
            let internal_err = e.source().unwrap();
            eyre!(
                "{}: {}",
                "Failed to render 'categories.html' template".bold(),
                internal_err
            )
        } else {
            eyre!("{}", "Failed to render 'categories.html' template".bold())
        }
    })?;
    body = rewrite_urls(body, &config.root_url, &state.routes_url);

    inject_livereload_script(&mut body);
    html_response(body, StatusCode::OK)
}

async fn handle_category(path: &str, state: &Arc<ServerState>) -> Result<Response<Body>> {
    let config = state.config.read().await.clone();
    let cat_prefix = format!("/{}/", config.categories_dir);
    let category = path.strip_prefix(&*cat_prefix).unwrap_or(path);

    // Fast path: lookup in pre-rendered memory cache
    if let Some(html) = fast_path_lookup(state, path).await {
        let mut body = html;
        inject_livereload_script(&mut body);
        return html_response(body, StatusCode::OK);
    }

    // Slow path: render on demand
    let posts = state.posts.read().await.clone();

    let category_posts: Vec<_> = posts
        .into_iter()
        .filter(|post| {
            post.get("categories")
                .and_then(|c| c.as_array())
                .map(|cats| cats.iter().any(|c| c.as_str() == Some(category)))
                .unwrap_or(false)
        })
        .collect();

    let mut context = tera::Context::new();
    context.insert("config", &config);
    context.insert("category", &category);
    context.insert("posts", &category_posts);
    context.insert(
        "lith_version",
        option_env!("LITH_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")),
    );

    let tera = state.tera.read().await;
    let mut body = tera.render("category.html", &context).map_err(|e| {
        if e.source().is_some() {
            let internal_err = e.source().unwrap();
            eyre!(
                "{}: {}",
                "Failed to render 'category.html' template".bold(),
                internal_err
            )
        } else {
            eyre!("{}", "Failed to render 'category.html' template".bold())
        }
    })?;

    body = rewrite_urls(body, &config.root_url, &state.routes_url);

    inject_livereload_script(&mut body);
    html_response(body, StatusCode::OK)
}

#[instrument(skip(stream, reload_tx))]
pub(super) async fn handle_websocket(stream: TcpStream, reload_tx: Arc<broadcast::Sender<()>>) {
    let mut ws_stream = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => {
            debug!("New WebSocket connection");
            ws
        }
        Err(e) => {
            error!("WebSocket error: {}", e);
            return;
        }
    };

    let mut rx = reload_tx.subscribe();
    if let Err(e) = ws_stream
        .send(tokio_tungstenite::tungstenite::Message::Text(
            WS_HELLO_MESSAGE.into(),
        ))
        .await
    {
        error!("Failed to send hello message: {}", e);
        return;
    }

    loop {
        tokio::select! {
            _ = rx.recv() => {
                if let Err(e) = ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(WS_RELOAD_MESSAGE.into())).await {
                    error!("WebSocket send error: {}", e);
                    break;
                }
            }
            msg = ws_stream.next() => {
                match msg {
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) => break,
                    Some(Err(e)) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
}

async fn handle_request(req: Request<Body>, state: Arc<ServerState>) -> Result<Response<Body>> {
    let request_path = req.uri().path();
    debug!(path = %request_path, "Handling request");

    let categories_dir = {
        let config = state.config.read().await;
        config.categories_dir.clone()
    };
    match request_path {
        "/livereload.js" => Ok(Response::builder()
            .header(CONTENT_TYPE, "text/javascript")
            .body(LIVE_RELOAD_SCRIPT.into())?),
        path if path == format!("/{}", categories_dir) => handle_category_index(&state).await,
        path if path.starts_with(&format!("/{}/", categories_dir)) => {
            handle_category(path, &state).await
        }
        path if path.starts_with("/assets/") => handle_asset(path, &state.paths, &state).await,
        path if path.ends_with(".xml") => handle_xml_feed(path, &state).await,
        _ => handle_content(request_path, state).await,
    }
}

#[instrument(name = "serve_request", skip(req, state))]
pub(super) async fn handle_server_request(
    req: Request<Body>,
    state: Arc<ServerState>,
) -> Result<Response<Body>, Infallible> {
    let start = std::time::Instant::now();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path().to_owned();

    debug!(method = %method, path = %path, "Incoming request");

    let response = match handle_request(req, state.clone()).await {
        Ok(res) => res,
        Err(e) => {
            error!("{}", e);
            let e_str = e.to_string().replace("\x1b[1m", "").replace("\x1b[0m", "");
            {
                let tera = state.tera.try_read();
                let config = state.config.try_read();
                match (tera, config) {
                    (Ok(tera), Ok(config)) => {
                        if tera.get_template_names().any(|n| n == "500.html") {
                            let posts = state.posts.try_read().ok();
                            let collections = posts
                                .as_ref()
                                .map(|p| shared::precompute_collection_subsets(p, &config))
                                .unwrap_or_default();
                            let shared_context = posts
                                .as_ref()
                                .map(|p| shared::build_shared_context(p, &config, &collections))
                                .unwrap_or_else(|| {
                                    shared::build_shared_context(&[], &config, &collections)
                                });
    let mut context = shared_context;
                            context.insert("error_message", &e_str);
                            match tera.render("500.html", &context) {
                                Ok(rendered) => Response::builder()
                                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                                    .header(CONTENT_TYPE, "text/html; charset=utf-8")
                                    .body(Body::from(rendered))
                                    .unwrap(),
                                _ => Response::builder()
                                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                                    .body(Body::from(format!(
                                        "500 Internal Server Error\n\n{}",
                                        e_str
                                    )))
                                    .unwrap(),
                            }
                        } else {
                            Response::builder()
                                .status(StatusCode::INTERNAL_SERVER_ERROR)
                                .body(Body::from(format!(
                                    "500 Internal Server Error\n\n{}",
                                    e_str
                                )))
                                .unwrap()
                        }
                    }
                    _ => Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::from(format!(
                            "500 Internal Server Error\n\n{}",
                            e_str
                        )))
                        .unwrap(),
                }
            }
        }
    };

    let duration = start.elapsed();
    let status = response.status();

    if path != "/livereload.js" {
        let method_str = method.to_string();
        let method_colored = if method_str == "GET" {
            method_str.green().bold()
        } else {
            method_str.yellow().bold()
        };
        let status_code = status.as_u16();
        let status_colored = match status_code {
            200..=299 => status_code.to_string().green(),
            300..=399 => status_code.to_string().cyan(),
            400..=499 => status_code.to_string().yellow(),
            _ => status_code.to_string().red(),
        };
        let duration_str = format!("{:.1?}", duration);
        let duration_colored = if duration.as_millis() >= 500 {
            duration_str.yellow()
        } else {
            duration_str.dimmed()
        };
        println!(
            "  {} {:<60}  {}  {}",
            method_colored, path, status_colored, duration_colored
        );
    }

    Ok(response)
}
