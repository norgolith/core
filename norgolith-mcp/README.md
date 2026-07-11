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

## Tools

| Tool | Description |
|------|-------------|
| `search_docs(query)` | Search all documentation for a query string |

## See Also

- [Norgolith documentation](https://norgolith.dev/docs)
- [Norgolith repository](https://github.com/norgolith/core)
