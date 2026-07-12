<div align="center">

<img src="./res/norgolith_text.png" alt="Norgolith logo"/>

[![CI](https://github.com/norgolith/core/actions/workflows/release.yml/badge.svg)](https://github.com/norgolith/core/actions)
[![norgolith-plugin-sdk](https://img.shields.io/crates/v/norgolith-plugin-sdk?label=crates.io%20plugin-sdk)](https://crates.io/crates/norgolith-plugin-sdk)
[![norgolith-mcp](https://img.shields.io/crates/v/norgolith-mcp?label=crates.io%20mcp)](https://crates.io/crates/norgolith-mcp)
[![License: GPL v2](https://img.shields.io/badge/license-GPLv2-blue.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.html)

</div>

---

The monolithic Norg static site generator built with Rust. Leverage [rust-norg] syntax
validation with Norg-to-HTML conversion to build static sites from Norg content.

## 🌟 Features

- **Norg-native content** with validation via [rust-norg]
- **Tera v2 templates** with shortcodes, optional chaining, and rich filters
- **C ABI plugin system** with Landlock sandboxing (filesystem confinement, timeout, panic isolation)
- **Incremental builds** via content-hash caching
- **Parallel builds** powered by Rayon
- **SEO** out of the box: sitemap.xml, robots.txt, OpenGraph, Twitter Cards
- **Custom error pages** (404.html, 500.html)
- **Live preview** dev server with hot reload and config hot-reloading
- **Plugin sources**: install from crates.io, Git repositories, or local paths

<details>
<summary>⚖️ Comparison with Hugo and Zola</summary>

| Dimension            | Hugo                              | Zola                | Norgolith                     |
| -------------------- | --------------------------------- | ------------------- | ----------------------------- |
| Language             | Go                                | Rust                | Rust                          |
| Templates            | Go templates                      | Tera v1             | Tera v2 + shortcodes          |
| Content format       | Markdown + 5 others               | Markdown only       | Norg only                     |
| Plugin system        | None                              | None                | C ABI + Landlock sandbox      |
| Content validation   | None                              | Link checking       | rust-norg parser + schemas    |
| Syntax highlighting  | Built-in (Chroma)                 | Built-in            | Via plugin (tree-sitter)      |
| Image processing     | Yes                               | Yes                 | Planned                       |
| Asset bundling       | Yes                               | No                  | Planned                       |
| Asset fingerprinting | Yes                               | No                  | Planned                       |
| Configuration        | TOML / YAML / JSON                | TOML                | TOML                          |
| Multilingual         | Yes                               | Yes                 | No                            |
| Shortcodes           | Built-in                          | No                  | Tera v2 `component()`         |
| Custom output        | JSON, RSS, etc                    | RSS, Atom           | RSS, Atom                     |
| Theme system         | Yes                               | Yes                 | Yes                           |
| Built-in search      | No                                | Yes                 | Via plugin                    |
| Build                | Parallel (pages)                  | Parallel            | Parallel + incremental        |

</details>

## 📝 Requirements

| Component | Requirement                      |
| --------- | -------------------------------- |
| Build     | Rust >= 1.85 (Rust 2024 edition) |

## ⚡ Install

Run `cargo install --release --path .` to compile and install Norgolith in your `~/.cargo/bin` directory.

<details>
<summary>🔨 AUR</summary>

Use Arch User Repository helper to install `norgolith-git`.

`paru -S norgolith-git`

</details>

<details>
<summary>📦 Nix Package</summary>

For latest release version: `nix-shell -p norgolith`.

For git version you can add Norgolith to NixOS configuration with flakes.

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    norgolith.url = "github:norgolith/core";
  };
  outputs =
    { nixpkgs, norgolith, ... }:
    {
      nixosConfigurations.mysystem = nixpkgs.lib.nixosSystem {
        modules = [
          {
            environment.systemPackages = [
              norgolith.packages.x86_64-linux.default
            ];
          }
        ];
      };
    };
}
```

</details>

<details>
<summary>📦 Plugin SDK</summary>

Install from crates.io into your plugin project:

```bash
cargo add norgolith-plugin-sdk
```

Or build from the flake:

```bash
nix build .#norgolith-plugin-sdk
```

Add it to your NixOS configuration:

```nix
environment.systemPackages = [
  norgolith.packages.x86_64-linux.norgolith-plugin-sdk
];
```

Scaffold a new plugin with `lith plugin new my-plugin`, which generates `plugin.toml`, `Cargo.toml`, and `src/lib.rs`.

**Hooks**: `pre_build`, `post_convert`, `post_render`, `post_build`.
**Sandboxing**: Landlock filesystem confinement, configurable timeout per hook, panic isolation.
**Logging**: `plugin_log!` macro bridges to Norgolith's tracing pipeline.

The canonical plugin is written in Rust using the SDK:

```rust
use norgolith_plugin_sdk::*;

register_plugin!("my-plugin",
    hooks: [post_render: my_hook]
);

fn my_hook(json: serde_json::Value) -> Result<Option<String>, String> {
    let ctx: TransformContext = serde_json::from_value(json)
        .map_err(|e| e.to_string())?;
    Ok(Some(ctx.html))
}
```

Plugins use Norgolith's C ABI, so any language that exports C functions can participate. A C plugin looks like:

```c
#include <stdint.h>
#include <string.h>

#define HOOK_POST_RENDER 4

typedef char* (*plugin_fn)(const char*);

typedef struct {
    uint32_t abi_version;
    const char* name;
    const char* version;
    void (*log_fn)(uint32_t, const char*);
} plugin_info;

char* post_render(const char* input) {
    /* parse input JSON, return modified HTML or NULL */
    return NULL;
}

void norgolith_plugin_init(plugin_info* info, uint32_t* mask, plugin_fn hooks[4]) {
    info->abi_version = 1;
    info->name = "my-c-plugin";
    info->version = "0.1.0";
    *mask = HOOK_POST_RENDER;
    hooks[2] = post_render;
}
```

For Lua, write a small C shim that embeds Lua and dispatches hook calls to a `.lua` script:

```c
// c-shim.c — compiles to a .so that Norgolith loads
#include <lua5.4/lua.h>
#include <lua5.4/lauxlib.h>

static lua_State *L;

void norgolith_plugin_init(plugin_info* info, uint32_t* mask, plugin_fn hooks[4]) {
    L = luaL_newstate();
    luaL_dofile(L, "my-plugin.lua");
    *mask = HOOK_POST_RENDER;
    hooks[2] = &bridge;
}

char* bridge(const char* input) {
    /* push input, call my-plugin.lua's post_render, return result */
}
```

See [sdk/README.md](./sdk/README.md) and [norgolith.dev/docs/plugins](https://norgolith.dev/docs/plugins) for full documentation.

</details>

<details>
<summary>🤖 MCP Server</summary>

Install from crates.io:

```bash
cargo install norgolith-mcp
```

Or build from source:

```bash
cargo build --release -p norgolith-mcp
```

Or build from the flake:

```bash
nix build .#norgolith-mcp
```

Add it to your NixOS configuration:

```nix
environment.systemPackages = [
  norgolith.packages.x86_64-linux.norgolith-mcp
];
```

Configure in your MCP client:

```json
{
  "mcp": {
    "norgolith": {
      "type": "local",
      "command": "norgolith-mcp",
      "enable": true
    }
  }
}
```

**Capabilities**: resources (`norgolith://docs/*`), tools (`search_docs`, `read_source`).
See [norgolith-mcp/README.md](./norgolith-mcp/README.md) for details.

</details>

## 🚀 Quick Start

```bash
lith init my-site
cd my-site
lith new -k post hello-world
lith dev --open
lith build
```

## 📖 Usage

The produced binary is called `lith` for short.

```
$ lith --help

The monolithic Norg static site generator

Usage: lith [OPTIONS] <COMMAND>

Commands:
  init     Initialize a new Norgolith site
  theme    Theme management
  dev      Run a site in development mode
  new      Create a new asset in the site and optionally open it using your preferred system editor. e.g. 'new -k norg post1.norg' -> 'content/post1.norg'
  build    Build a site for production
  plugin   Plugin management
  preview  Preview from build result
  help     Print this message or the help of the given subcommand(s)

Options:
  -v, --version            Print version
  -d, --dir <PROJECT_DIR>  Operate on the project in the given directory
  -h, --help               Print help
```

<details>
<summary>🔌 Plugin Development</summary>

Norgolith ships a C ABI plugin system. Plugins are shared libraries (`.so`, `.dylib`, `.dll`) loaded at runtime and sandboxed with Landlock.

**Hook pipeline**:

```
Content --> pre_build --> post_convert --> Tera render --> post_render --> post_write --> post_build
```

**Plugin install**:

```bash
lith plugin install ./my-plugin            # from local path
lith plugin install norgolith-tree-sitter  # from crates.io
lith plugin install --git https://github.com/norgolith/norgolith-tree-sitter.git
```

**Sandboxing**: each plugin gets filesystem confinement (configurable: none / read / write / read-write), a per-hook timeout (default 10s), and panic isolation.

See [sdk/README.md](./sdk/README.md) for the full plugin guide.

</details>

<details>
<summary>❄️ Developing with Nix</summary>

The repository includes a Nix flake for development and testing.

```sh
nix build .          # build, output in result/
nix run .            # build and run
nix develop .        # dev shell with all dependencies
```

For Nix-direnv integration, entering the directory activates the dev shell automatically.

</details>

## 📚 Documentation

The documentation site source lives in the `docs/` directory. To work on it:

```sh
# From repo root, enter dev shell (includes tailwindcss, mprocs).
nix develop

# Build lith
cargo build --release

# Start docs dev server
cd docs
mprocs
```

See [docs/README.md](./docs/README.md) for details.

## 🚀 Community

Join the Neorg community and get help or discuss about the project:

- [Discord server](https://discord.gg/T6EgTAX7ht)

## 💌 Supporting Norgolith

Developing and maintaining open-source projects takes time and effort. If you find Norgolith
valuable and would like to support its continued development, here are some ways you can help:

- **Star this repository on GitHub**: this helps raise awareness and shows the project is actively
  maintained.
- **Contribute code or documentation**: we welcome contributions from the community.
- **Spread the word**: let others know about Norgolith if you think they might benefit from it.
- **Financial Support (Optional)**: if you'd like to offer financial support, you can consider using
  my Ko-fi page (linked in the repository). Any amount is greatly appreciated and helps me invest
  further time in Norgolith's development.

### 💜 Sponsors

Huge thanks to the project sponsors for supporting my work!

[![Ladas552](https://images.weserv.nl/?url=github.com/Ladas552.png&h=60&w=60&fit=cover&mask=circle&maxage=7d)](https://github.com/Ladas552)


## 📖 License

This project is licensed under the GNU General Public License v2 (GPLv2).
You can find the license details in the [LICENSE](./LICENSE) file.

[rust-norg]: https://github.com/nvim-neorg/rust-norg
