# Changelog

## [1.1.0](https://github.com/norgolith/core/compare/norgolith-plugin-sdk-v1.0.0...norgolith-plugin-sdk-v1.1.0) (2026-07-10)


### Bug Fixes

* **sdk:** fix Rust 2024 edition unsafe attribute in register_plugin macro ([407f9ec](https://github.com/norgolith/core/commit/407f9eca41295ec7073de3b7d77219e70112ec06))


### Refactoring

* nuke plugin.toml version field ([ed511fc](https://github.com/norgolith/core/commit/ed511fc28353fe48910f8a9602c1dba7dd16842a))


### Documentation

* add FFI plugin version guidance ([ed511fc](https://github.com/norgolith/core/commit/ed511fc28353fe48910f8a9602c1dba7dd16842a))
* remove plugin.toml version field from documentation ([ed511fc](https://github.com/norgolith/core/commit/ed511fc28353fe48910f8a9602c1dba7dd16842a))

## [1.0.0](https://github.com/norgolith/core/compare/norgolith-plugin-sdk-v0.1.0...norgolith-plugin-sdk-v1.0.0) (2026-07-07)


### ⚠ BREAKING CHANGES

* bump to 2024 edition

### Features

* bump to 2024 edition ([f9dc92d](https://github.com/norgolith/core/commit/f9dc92de8bebd8bf5ff5c65598c1c33f56e6931e))
* **flake:** add norgolith-plugin-sdk build ([a3ad5d1](https://github.com/norgolith/core/commit/a3ad5d1b63fbfb57ff1ff620e42a1bc5844b702c))
* **plugin:** per-plugin config from norgolith.toml ([b1c0a42](https://github.com/norgolith/core/commit/b1c0a42f43be0f85cf733cf0baaaf6856be9f5e3))
* **plugin:** SDK logging bridge ([ee7e92a](https://github.com/norgolith/core/commit/ee7e92afdadf88904bfd7715e5d93b8b4bb8552a))
* **sdk:** build as cdylib for cross-language FFI plugins ([0ae1ee2](https://github.com/norgolith/core/commit/0ae1ee2ed030bfe83e36b7f4b23194c2d0878a8c))
* **sdk:** implement bridge functions and working register_plugin! macro ([48f3f9b](https://github.com/norgolith/core/commit/48f3f9b1eeec076be6b87c36773944395c4ee8e5))


### Bug Fixes

* **sdk:** change crate-type from cdylib to rlib ([0626ed5](https://github.com/norgolith/core/commit/0626ed5796393b080606580e9b1f51b29f97871c))


### Refactoring

* **plugin:** remove unused parse_status_response ([e21d91f](https://github.com/norgolith/core/commit/e21d91fc8b4e7c810ed8390b56d791089eff99d8))
* **sdk:** remove unused PreBuildContext and PostBuildContext ([e21d91f](https://github.com/norgolith/core/commit/e21d91fc8b4e7c810ed8390b56d791089eff99d8))
* **workspace:** migrate to monorepo with core/ and sdk/ crates ([3e27e27](https://github.com/norgolith/core/commit/3e27e273181d2f01af4d2c5b9057b32432f15d1a))


### Documentation

* add manual changelogs for v0.5.0 and SDK v0.1.0 ([cb2a070](https://github.com/norgolith/core/commit/cb2a07055e6bfd620f629eaf46ae781cd0421017))
* document plugins ([0d584d8](https://github.com/norgolith/core/commit/0d584d85ed4a5e97699dd580b52e2512ce476c3a))
* update domain links ([4b0eeb2](https://github.com/norgolith/core/commit/4b0eeb2222ef7908669914ea499efe03d00f5259))


### CI

* remove last-release-sha, tags now match norgolith-v* format ([4b0eeb2](https://github.com/norgolith/core/commit/4b0eeb2222ef7908669914ea499efe03d00f5259))


### Miscellaneous

* format source code ([ea514a8](https://github.com/norgolith/core/commit/ea514a836f5c3965f9a6714b92bc37709e2c85fc))

## 0.1.0 (2026-07-01)


### Features

* **sdk:** implement bridge functions and working register_plugin! macro ([48f3f9b](https://github.com/norgolith/core/commit/48f3f9b1eeec076be6b87c36773944395c4ee8e5))


### Refactoring

* **workspace:** migrate to monorepo with core/ and sdk/ crates ([3e27e27](https://github.com/norgolith/core/commit/3e27e273181d2f01af4d2c5b9057b32432f15d1a))


### Documentation

* document plugins ([0d584d8](https://github.com/norgolith/core/commit/0d584d85ed4a5e97699dd580b52e2512ce476c3a))
