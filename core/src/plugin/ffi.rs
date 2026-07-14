use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic::AssertUnwindSafe;
use std::sync::mpsc;
use std::time::Duration;

use miette::{Result, bail};
use tracing::{debug, error, info, trace, warn};

/// Core-provided logging callback for plugins. Level: 0=trace, 1=debug, 2=info, 3=warn, 4=error.
pub extern "C" fn core_log_handler(level: u32, msg: *const c_char) {
    if msg.is_null() {
        return;
    }
    let text = unsafe { CStr::from_ptr(msg) }.to_string_lossy();
    match level {
        0 => trace!(target: "plugin", "{}", text),
        1 => debug!(target: "plugin", "{}", text),
        2 => info!(target: "plugin", "{}", text),
        3 => warn!(target: "plugin", "{}", text),
        _ => error!(target: "plugin", "{}", text),
    }
}

/// Wrapper to make raw pointer results Send-safe across threads
struct HookResult(Option<*mut c_char>);

// SAFETY: we only send the pointer across threads, actual deref happens on the receiving side
unsafe impl Send for HookResult {}

pub use norgolith_plugin_sdk::{PluginFn, PluginInfo};

/// Function pointer type for freeing plugin-allocated strings
pub type FreeStringFn = extern "C" fn(*mut c_char);

/// Call a plugin hook with catch_unwind + thread timeout
///
/// Returns `Ok(None)` if the plugin returned NULL (no change)
/// Returns `Ok(Some(json))` if the plugin returned modified content
/// Returns `Err(msg)` on panic, timeout, or invalid output
///
/// # Safety Note
/// The returned pointer is freed with `libc::free`. This assumes the plugin's global allocator is
/// compatible with libc malloc (true for the default system allocator). Plugins compiled with
/// jemalloc or mimalloc will cause UB. This is an acceptable trade-off for MVP; a future version
/// can add a `plugin_free` callback to let each plugin provide its own deallocator
pub fn call_hook_safe(f: PluginFn, input: &str, timeout: Duration) -> Result<Option<String>> {
    let c_input =
        CString::new(input).map_err(|e| miette::miette!("Failed to prepare plugin input data: {}", e))?;

    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let result =
            std::panic::catch_unwind(AssertUnwindSafe(|| HookResult(Some(f(c_input.as_ptr())))));
        let _ = tx.send(result);
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(HookResult(ptr))) => {
            let ptr = ptr.unwrap();
            if ptr.is_null() {
                return Ok(None);
            }
            let result = unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned();
            unsafe { libc::free(ptr as *mut libc::c_void) };
            Ok(Some(result))
        }
        Ok(Err(panic)) => {
            let msg = match panic.downcast_ref::<&str>() {
                Some(s) => s.to_string(),
                None => match panic.downcast_ref::<String>() {
                    Some(s) => s.clone(),
                    None => "unknown panic".to_string(),
                },
            };
            Err(miette::miette!("Plugin panicked: {}", msg))
        }
        Err(_timeout) => Err(miette::miette!(
            "Plugin hook timed out after {}ms",
            timeout.as_millis()
        )),
    }
}

pub fn parse_hook_response(plugin_name: &str, json: &str) -> Result<Option<String>> {
    let val: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| miette::miette!("Invalid response from plugin '{}': {}", plugin_name, e))?;

    if let Some(status) = val.get("status").and_then(|v| v.as_str())
        && status == "error"
    {
        let msg = val
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        bail!("plugin '{}' returned error: {}", plugin_name, msg);
    }

    match val.get("html").and_then(|v| v.as_str()) {
        Some(html) => Ok(Some(html.to_string())),
        None => Ok(None),
    }
}

pub fn parse_status_response(plugin_name: &str, json: &str) -> Result<()> {
    let val: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| miette::miette!("Invalid response from plugin '{}': {}", plugin_name, e))?;

    if let Some(status) = val.get("status").and_then(|v| v.as_str())
        && status == "error"
    {
        let msg = val
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        bail!("plugin '{}' returned error: {}", plugin_name, msg);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hook_response_with_html() {
        let json = r#"{"html": "<h1>Hello</h1>"}"#;
        assert_eq!(
            parse_hook_response("test_plugin", json).unwrap(),
            Some("<h1>Hello</h1>".to_string())
        );
    }

    #[test]
    fn test_parse_hook_response_null_html() {
        let json = r#"{"html": null}"#;
        assert_eq!(parse_hook_response("test_plugin", json).unwrap(), None);
    }

    #[test]
    fn test_parse_hook_response_error() {
        let json = r#"{"status": "error", "message": "something broke"}"#;
        assert!(parse_hook_response("test_plugin", json).is_err());
    }
}
