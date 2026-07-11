# Norgolith MCP Server

MCP (Model Context Protocol) server for Norgolith documentation. Lets AI assistants browse, read, and search Norgolith docs directly.

## Installation

Build from source:

```bash
cargo build --release -p norgolith-mcp
```

The binary is at `target/release/norgolith-mcp`.

## Configuration

Add to your MCP client config (e.g., `opencode.json`):

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "norgolith": {
      "type": "local",
      "command": "norgolith-mcp",
      "enabled": true
    }
  }
}
```

Make sure `norgolith-mcp` is in your `PATH`, or use the full path to the binary.

## Resources

Each documentation page is available as a resource with URI `norgolith://{path}` and MIME type `text/x-norg`.

| URI | Description |
| --- | ----------- |
| `norgolith://docs/commands` | CLI commands reference |
| `norgolith://docs/configuration` | Site configuration reference |
| `norgolith://docs/content-schemas` | Content schema validation |
| `norgolith://docs/contributing` | Contributing guide |
| `norgolith://docs/getting-started` | Quick start guide |
| `norgolith://docs/index` | Documentation index |
| `norgolith://docs/installation` | Installation guide |
| `norgolith://docs/plugins` | Plugin development guide |
| `norgolith://docs/templating` | Templating reference |
| `norgolith://docs/templating-migration` | Tera v2 migration guide |
| `norgolith://docs/theming` | Theming guide |
| `norgolith://index` | Site landing page |

Source code resources (MIME type `text/x-rust`):

| URI | Description |
| --- | ----------- |
| `norgolith://src/sdk/lib.rs` | Plugin SDK API |
| `norgolith://src/core/plugin/mod.rs` | Plugin loader core |
| `norgolith://src/core/plugin/ffi.rs` | C ABI bridge |
| `norgolith://src/core/plugin/manifest.rs` | Plugin manifest parsing |
| `norgolith://src/core/plugin/sandbox.rs` | Landlock sandboxing |

## Tools

| Tool | Description |
|------|-------------|
| `search_docs(query)` | Search all documentation for a query string |
| `read_source(path)` | Read a source file from the repo (monorepo only) |

## See Also

- [Norgolith documentation](https://norgolith.dev/docs)
- [Norgolith repository](https://github.com/norgolith/core)
