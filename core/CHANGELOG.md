# Changelog

## [1.4.0](https://github.com/norgolith/core/compare/norgolith-v1.3.1...norgolith-v1.4.0) (2026-09-01)


### Features

* **build:** add backlinks to page metadata ([9a8943c](https://github.com/norgolith/core/commit/9a8943cbf5de27ea48062e010bbf862e49958ed2))
* **check:** add json and github output formats ([942fc96](https://github.com/norgolith/core/commit/942fc96019662f182128ca81e169f7e7c905e881))
* **cli:** add lith check command for schema validation ([824e336](https://github.com/norgolith/core/commit/824e3365e27ff764714001aeddcdf6381bead537))
* **dev:** restore schema validation as warnings in dev server ([200d416](https://github.com/norgolith/core/commit/200d4161059b5300aea91035c6632d021ad123e8))
* **plugins:** expose fingerprint map to hooks ([47d728c](https://github.com/norgolith/core/commit/47d728ce20a95560cc69bdcef5664a78af090ce4))
* **schema:** add datetime field type ([9a02bf1](https://github.com/norgolith/core/commit/9a02bf155c15ae252ee44f97ed8a130c0993d876))
* **schema:** add error codes and severity to ValidationError ([3d531c9](https://github.com/norgolith/core/commit/3d531c9149b5a4878872761b6fa748ed9e792b67))
* **schema:** add Integer and Float field types ([c77bad0](https://github.com/norgolith/core/commit/c77bad0cde5beec9807d8bb0f916a3ee44c7f5f4))
* **schema:** add min_length option for String fields ([f58da5f](https://github.com/norgolith/core/commit/f58da5f8b71625faab770b46d23a854e922a527e))
* **schema:** add not_required rule action ([0665f7c](https://github.com/norgolith/core/commit/0665f7c070a229ad881997937a4dd0a844ed6cc5))
* **schema:** add one_of enum for string fields ([9b3a18d](https://github.com/norgolith/core/commit/9b3a18dd2b931e9967f4230bb6a1f07ad0d55878))
* **schema:** add source labels and doc URLs to validation errors ([7fee2b2](https://github.com/norgolith/core/commit/7fee2b28352a8938596377331e8cce4a6bfbfe04))
* **schema:** deny_unknown_fields on FieldDefinition ([6f179c1](https://github.com/norgolith/core/commit/6f179c1c3c953d028428215f28c10dbe796e857a))
* **schema:** resolve spans for nested objects and array items ([902adde](https://github.com/norgolith/core/commit/902addea483222261d66c7064058034e2746294a))
* **schema:** support * and ** wildcards in schema paths ([1c03391](https://github.com/norgolith/core/commit/1c03391ab37b981106dffee8a10891fcb465b756))
* **schema:** support nested paths in rule conditions ([e30f277](https://github.com/norgolith/core/commit/e30f27783a64a2803e39ea6353986d28b8a30091))
* **schema:** validate required fields have matching field defs ([e55ac02](https://github.com/norgolith/core/commit/e55ac027249be8c7e201df751226b88f28711cc5))
* **schema:** warn on near-miss unknown field keys ([ffa5b2b](https://github.com/norgolith/core/commit/ffa5b2bb9d5c684242534fbd715200938d3ef213))
* **theme:** add tangled support ([542a283](https://github.com/norgolith/core/commit/542a283fdb034c505a5405ff30232b16ea4a68db))


### Bug Fixes

* **cache:** key rendered HTML to fingerprint snapshot ([1f56c35](https://github.com/norgolith/core/commit/1f56c351dbcb23fed9e565fe26b5a3149b4c8f54))
* **cli:** render full miette diagnostics in lith check ([132067d](https://github.com/norgolith/core/commit/132067d2b83771457f8255fc87bcd3ebae369680))
* **converter:** slugify heading link anchors ([90a62a4](https://github.com/norgolith/core/commit/90a62a4052a81b740ed811594db64b5322464e66))
* **dev:** point config.rootUrl at the dev server ([e3158b0](https://github.com/norgolith/core/commit/e3158b0693f778ed5fba0e3023177d7c4d8d6aeb)), closes [#216](https://github.com/norgolith/core/issues/216)
* **dev:** skip missing dirs in file watcher ([2d39877](https://github.com/norgolith/core/commit/2d3987768209a63739cf7430c1b38ad59a4ecce3)), closes [#217](https://github.com/norgolith/core/issues/217)
* **schema:** add Object field validation arm ([b05d4b1](https://github.com/norgolith/core/commit/b05d4b14338a188e2188eb56d9895e6e5e996050))
* **schema:** anchor missing-field errors to the meta directive ([da9fd8a](https://github.com/norgolith/core/commit/da9fd8abac38727fd0748c83b6249ed6a3fb1906))
* **schema:** show matched schema path in error label ([7104ac0](https://github.com/norgolith/core/commit/7104ac0a8ac0695c027fa97519cdaefc85612aab))
* **seo:** dedup sitemap URLs and normalize lastmod ([a97ccaa](https://github.com/norgolith/core/commit/a97ccaa2d65f9cddb834e1e3591ce48a20ce8b75))


### Refactoring

* **schema:** thread file content for miette source snippets ([40497b0](https://github.com/norgolith/core/commit/40497b04d2bbdbb1d5a9e29befa877ed9ca3e9b4))


### Miscellaneous

* fix clippy warnings ([a1745c9](https://github.com/norgolith/core/commit/a1745c97a80561f395416f51c97e828bdee3525d))
* format source code ([9e7abd3](https://github.com/norgolith/core/commit/9e7abd3be825bad4a3bf7b7e9afae289b7c79605))


### Tests

* **new:** verify template metadata passes schema validation ([6160368](https://github.com/norgolith/core/commit/6160368473a4a6e4515d231637c105cea4456afc))
* **schema:** add tests for Object, serde round-trip ([e759ea0](https://github.com/norgolith/core/commit/e759ea0783caae308dc9c40f8bb3ae2ae77d82ef))
* **schema:** cover slash-tolerant path resolution ([f0e12c6](https://github.com/norgolith/core/commit/f0e12c6cd2bf8b893506e556e3c91c8817a89b14))

## [1.3.1](https://github.com/norgolith/core/compare/norgolith-v1.3.0...norgolith-v1.3.1) (2026-07-29)


### Bug Fixes

* **build:** strip norgolith- prefix from git tag version ([39db1d7](https://github.com/norgolith/core/commit/39db1d73549adbf1cd52077d72710580d9989289))
* **logging:** migrate user-facing warn!/error! tracing calls to eprintln!(miette!) ([1982316](https://github.com/norgolith/core/commit/1982316328272c1f33a2551ad41da0a04d2c2a2c))


### Miscellaneous

* format code ([88f713b](https://github.com/norgolith/core/commit/88f713ba1ae6c9ba5be5448e1e3b948a897b2361))
* make clippy happy ([95a5d36](https://github.com/norgolith/core/commit/95a5d366857155cbd61ccaedd804ac76f3f55934))
* regenerate lockfile ([95a5d36](https://github.com/norgolith/core/commit/95a5d366857155cbd61ccaedd804ac76f3f55934))

## [1.3.0](https://github.com/norgolith/core/compare/norgolith-v1.2.0...norgolith-v1.3.0) (2026-07-29)


### Features

* add search index generation + dev server endpoint + docs theme UI ([895844d](https://github.com/norgolith/core/commit/895844d57e7a4e17634af00aa848beac4e3e7ec4))
* asset fingerprinting + config flag + template warnings ([250bf88](https://github.com/norgolith/core/commit/250bf88b7895da06c3bc23f5b5902c8dbbd75b76))
* cache rendered HTML in dev server BuildCache ([aed2895](https://github.com/norgolith/core/commit/aed28959a33868e85712279d71ad7fa041037747))
* **cli:** polish prompt labels, help text, and error handling ([6860829](https://github.com/norgolith/core/commit/686082937e055be436c18bf4884e9d64eb7fe4ef))
* **ux:** add init feedback and plugin build spinner ([3726e6e](https://github.com/norgolith/core/commit/3726e6e485ef9781df34746cff8e847eb2e47a6f))
* **ux:** add spinners to plugin blocking operations ([aa22aa8](https://github.com/norgolith/core/commit/aa22aa870a956cf919db34d214fbc636aed1a610))
* **ux:** check theme min_version at build, surface cargo warnings ([c7950ab](https://github.com/norgolith/core/commit/c7950abf051c368eb8cbd60cb87804e36e2453ee))
* **ux:** surface invisible errors to user via miette warnings ([5f7717a](https://github.com/norgolith/core/commit/5f7717a7984c59cf6bbb47049987103bd06b66cc))


### Bug Fixes

* **converter:** sanitize heading IDs with slugify ([6d3f164](https://github.com/norgolith/core/commit/6d3f164c60d7ca4233b10fc7dc99b66eb2207c00))
* **init:** create templates/partials/seo_head.html on lith init ([9caeb53](https://github.com/norgolith/core/commit/9caeb53f4ca932dc8d2f181f4c311e5eb66adacf))
* **search:** scope body extraction to &lt;main&gt; element ([72fa4eb](https://github.com/norgolith/core/commit/72fa4eb77bba51ae518bc039c3650f2fbfb241d9))
* **ux:** route init success output consistency, add closing checkmark ([e7d7212](https://github.com/norgolith/core/commit/e7d7212093d0470882fabc2b9453626cbb8c30f1))
* **ux:** surface build cache save failure to user ([bb726c0](https://github.com/norgolith/core/commit/bb726c05b852b6303a5c0c177c2aeaf873bbe8f5))


### Performance

* **build:** cache rendered HTML for incremental rebuilds ([5d91f7b](https://github.com/norgolith/core/commit/5d91f7bfac10d3bbf6e73329b42ab92db62316a5))
* **build:** parallelize file I/O and category pages ([54d2201](https://github.com/norgolith/core/commit/54d2201ec0119823bfc872250f8a8bc694083e5b))
* replace minify-js with oxc_minifier for JS asset minification ([d83c357](https://github.com/norgolith/core/commit/d83c357ceec0eff9b52cb34af2cd98b3d5fa89d9))


### Refactoring

* **core:** replace spinoff with indicatif for theme spinners ([340ddac](https://github.com/norgolith/core/commit/340ddaced1c8d901108f6c67537598cc7ec1cc13))
* **search:** trim ~15 lines of unnecessary stuff ([cd77e5f](https://github.com/norgolith/core/commit/cd77e5f9126585ae76e6e04f8ac24eca4d0e1ba5))
* **ux:** use native miette `help` in fingerprint warnings ([52f8ba5](https://github.com/norgolith/core/commit/52f8ba58fb31aea78c0c10adbc7538125536b6af))


### CI

* **release:** regenerate lock file before locked build ([f465882](https://github.com/norgolith/core/commit/f465882a1bff2742005b8eaa60bdeba7f2a34c8f))


### Miscellaneous

* **deps:** bump inquire, toml, whoami, notify-debouncer-full, tokio-tungstenite, serial_test ([afa9cea](https://github.com/norgolith/core/commit/afa9cea2fe16c97cdc47b43a0a60c415c9b3a8fa))
* factor complex result type into ConversionResult alias ([c40fb24](https://github.com/norgolith/core/commit/c40fb24bed7b04ac1788b955c838efaa2145847e))

## [1.2.0](https://github.com/norgolith/core/compare/norgolith-v1.1.2...norgolith-v1.2.0) (2026-07-27)


### Features

* better SEO and robots default values ([db06eb8](https://github.com/norgolith/core/commit/db06eb8a3fcf01f376b6a502e50a580d22ba8c36))
* **build:** add progress bars with indicatif ([ec7e0a3](https://github.com/norgolith/core/commit/ec7e0a3fd27dd742e300d403a7d82fb6713db265))
* **dev:** add progress bars to dev server startup ([b46417a](https://github.com/norgolith/core/commit/b46417a4628da3df9381d324a3db6f67e2b9da54))
* **seo:** improve built-in SEO and fix sitemap not generating ([965b6ea](https://github.com/norgolith/core/commit/965b6ea21169709426184ae76cff32af8cc88965))


### Bug Fixes

* **converter:** handle empty Norg documents after metadata strip ([853a943](https://github.com/norgolith/core/commit/853a943758a238acdce1e2898925222cce0b1bbd))
* **metadata:** insert rel_path into metadata table ([e231984](https://github.com/norgolith/core/commit/e23198420292c096b43f23a559017214e722ed84))
* **plugin:** improve error messages when cargo was not found in PATH ([9b5add2](https://github.com/norgolith/core/commit/9b5add2e89a6a71ba70cefe7f56415c967ace1ff))
* prefix collection names in render_category_index to avoid variable collision ([92e7c68](https://github.com/norgolith/core/commit/92e7c6813e4ad2a016a56821cd18b7cfa5203af9))
* **server:** proper dev server startup timing order ([9ec9f1f](https://github.com/norgolith/core/commit/9ec9f1f9e05c79a1537d148b0abaf4dfedad8f38))
* **tera:** register missing tera-contrib filters ([b974f8f](https://github.com/norgolith/core/commit/b974f8fcde33a7b38bc0682903c65e0f28b206d8))


### Performance

* **build:** swap minify-html for minify-html-onepass ([c8d1e07](https://github.com/norgolith/core/commit/c8d1e072ce1e837531d3cc813fad0c850f032b16))
* **dev:** parallelize render_all_pages with rayon ([3ca0eb0](https://github.com/norgolith/core/commit/3ca0eb01244af5aa0b3a94a95ef51756c47b6cbe))
* **metadata:** parallelize collect_all_posts_metadata with rayon ([121c2a2](https://github.com/norgolith/core/commit/121c2a255c6cedfeba70d159d1f36fc0aca4c532))


### Refactoring

* nuke plugin.toml version field ([ed511fc](https://github.com/norgolith/core/commit/ed511fc28353fe48910f8a9602c1dba7dd16842a))
* replace eyre with miette, improve error handling ([#197](https://github.com/norgolith/core/issues/197)) ([b6efc0b](https://github.com/norgolith/core/commit/b6efc0b8327a7f50bf3c2a8dc9b06180c601aacd))


### Documentation

* add FFI plugin version guidance ([ed511fc](https://github.com/norgolith/core/commit/ed511fc28353fe48910f8a9602c1dba7dd16842a))
* remove plugin.toml version field from documentation ([ed511fc](https://github.com/norgolith/core/commit/ed511fc28353fe48910f8a9602c1dba7dd16842a))
* use tree-sitter plugin for syntax highlight, tera code blocks instead of django ([e6a3e33](https://github.com/norgolith/core/commit/e6a3e33c2ccb4eab676385210afef2cf65b0f1d2))


### Miscellaneous

* release master ([#188](https://github.com/norgolith/core/issues/188)) ([141e452](https://github.com/norgolith/core/commit/141e4525ae1070b03a4c1db86d8e9f1808d739ca))
* release master ([#194](https://github.com/norgolith/core/issues/194)) ([5786b31](https://github.com/norgolith/core/commit/5786b31d83953d6f2d6a5af710519feb57393179))
* release master ([#198](https://github.com/norgolith/core/issues/198)) ([0e18531](https://github.com/norgolith/core/commit/0e1853101ca9e351a31e04320507b105fba8272b))
* update Cargo.lock ([d4c9f6a](https://github.com/norgolith/core/commit/d4c9f6a97176a7eb14365f346b18db732b87cb81))

## [1.1.2](https://github.com/norgolith/core/compare/norgolith-v1.1.1...norgolith-v1.1.2) (2026-07-14)


### Refactoring

* replace eyre with miette, improve error handling ([#197](https://github.com/norgolith/core/issues/197)) ([b6efc0b](https://github.com/norgolith/core/commit/b6efc0b8327a7f50bf3c2a8dc9b06180c601aacd))

## [1.1.1](https://github.com/norgolith/core/compare/norgolith-v1.1.0...norgolith-v1.1.1) (2026-07-13)


### Bug Fixes

* **metadata:** insert rel_path into metadata table ([e231984](https://github.com/norgolith/core/commit/e23198420292c096b43f23a559017214e722ed84))
* **tera:** register missing tera-contrib filters ([b974f8f](https://github.com/norgolith/core/commit/b974f8fcde33a7b38bc0682903c65e0f28b206d8))


### Documentation

* use tree-sitter plugin for syntax highlight, tera code blocks instead of django ([e6a3e33](https://github.com/norgolith/core/commit/e6a3e33c2ccb4eab676385210afef2cf65b0f1d2))

## [1.1.0](https://github.com/norgolith/core/compare/norgolith-v1.0.1...norgolith-v1.1.0) (2026-07-10)


### Bug Fixes

* **converter:** handle empty Norg documents after metadata strip ([853a943](https://github.com/norgolith/core/commit/853a943758a238acdce1e2898925222cce0b1bbd))
* **plugin:** improve error messages when cargo was not found in PATH ([9b5add2](https://github.com/norgolith/core/commit/9b5add2e89a6a71ba70cefe7f56415c967ace1ff))
* prefix collection names in render_category_index to avoid variable collision ([92e7c68](https://github.com/norgolith/core/commit/92e7c6813e4ad2a016a56821cd18b7cfa5203af9))


### Refactoring

* nuke plugin.toml version field ([ed511fc](https://github.com/norgolith/core/commit/ed511fc28353fe48910f8a9602c1dba7dd16842a))


### Documentation

* add FFI plugin version guidance ([ed511fc](https://github.com/norgolith/core/commit/ed511fc28353fe48910f8a9602c1dba7dd16842a))
* remove plugin.toml version field from documentation ([ed511fc](https://github.com/norgolith/core/commit/ed511fc28353fe48910f8a9602c1dba7dd16842a))

## [1.0.1](https://github.com/norgolith/core/compare/norgolith-v1.0.0...norgolith-v1.0.1) (2026-07-08)


### Bug Fixes

* categories render with Tera v2 compat ([60d7d65](https://github.com/norgolith/core/commit/60d7d652a7bc83ca01071d786d908c0238fa1b83))
* **dev:** fix server path resolution ([3a248b8](https://github.com/norgolith/core/commit/3a248b8b35b288670dbade72a5a85ea4c9e635c7))


### Documentation

* add 1.0 release blog post ([3a248b8](https://github.com/norgolith/core/commit/3a248b8b35b288670dbade72a5a85ea4c9e635c7))


### Miscellaneous

* pass clippy ([113b65a](https://github.com/norgolith/core/commit/113b65abe747421ceeae30f8c062a537f1e17771))

## [1.0.0](https://github.com/norgolith/core/compare/norgolith-v0.5.0...norgolith-v1.0.0) (2026-07-07)


### ⚠ BREAKING CHANGES

* **templating:** Custom Tera functions will use v2 Kwargs API. Template `is defined` tests will be replaced with optional chaining `?.` Deprecated filters will be removed. See templating-migration.norg docs
* **templating:** Template syntax changed from {{ now(format="%Y") }} to {{ now() | date(format="%Y") }}
* bump to 2024 edition
* **templating:** Custom Tera functions will use v2 Kwargs API. Template `is defined` tests will be replaced with optional chaining `?.` Deprecated filters will be removed. See templating-migration.norg docs

### Features

* add templated 404 and 500 error pages ([6604bb7](https://github.com/norgolith/core/commit/6604bb786d915c5ddbe5e18971a9872bd54cae2b)), closes [#94](https://github.com/norgolith/core/issues/94)
* bump to 2024 edition ([f9dc92d](https://github.com/norgolith/core/commit/f9dc92de8bebd8bf5ff5c65598c1c33f56e6931e))
* **flake:** add norgolith-plugin-sdk build ([a3ad5d1](https://github.com/norgolith/core/commit/a3ad5d1b63fbfb57ff1ff620e42a1bc5844b702c))
* **plugin:** add CLI flags for git and crates.io sources ([bbb66f6](https://github.com/norgolith/core/commit/bbb66f6d24270c89ab7feedf038460a91b01c9f7))
* **plugin:** add crates.io source for plugin install ([f129d47](https://github.com/norgolith/core/commit/f129d4700b78c005657ef512ace4f1e7bd0f9ef2))
* **plugin:** add git source for plugin install ([f2204e0](https://github.com/norgolith/core/commit/f2204e03d2d7e87f8c677f59507a7ef6b0620c39))
* **plugin:** per-plugin config from norgolith.toml ([b1c0a42](https://github.com/norgolith/core/commit/b1c0a42f43be0f85cf733cf0baaaf6856be9f5e3))
* **plugin:** SDK logging bridge ([ee7e92a](https://github.com/norgolith/core/commit/ee7e92afdadf88904bfd7715e5d93b8b4bb8552a))
* **templating:** add shortcode support via Tera v2 components ([4cb8b08](https://github.com/norgolith/core/commit/4cb8b08b7bbf8db84f97f3c3d864098ee132a8a6)), closes [#63](https://github.com/norgolith/core/issues/63)
* **templating:** migrate Tera v1 to v2 ([18bb527](https://github.com/norgolith/core/commit/18bb52745b28b9db38f4196973880cd53b9245fc))
* **templating:** migrate Tera v1 to v2 ([#182](https://github.com/norgolith/core/issues/182)) ([06f3484](https://github.com/norgolith/core/commit/06f348431959f2542853c82718d604ae053cc33d))
* **templating:** remove NowFunction, use tera-contrib now() + date filter ([395b270](https://github.com/norgolith/core/commit/395b270ff55d8122fa111799bdb4da040a33268b))
* **theme:** add min_version field, init prompt, info display ([ad996b9](https://github.com/norgolith/core/commit/ad996b9d54d4a7ce49e9c5a82dbced43809457ec))
* **theme:** check min_version on pull ([1ec5c31](https://github.com/norgolith/core/commit/1ec5c318ea2ee5978820f98fd2fb548cb77f8a46))
* **theme:** check min-version on update ([57e034c](https://github.com/norgolith/core/commit/57e034c9f899989684b804be2a29e59d3f99d78b))


### Bug Fixes

* **docs:** handle nested @-delimited blocks in rust-norg parser ([fa66ed7](https://github.com/norgolith/core/commit/fa66ed78deff3f6ad9b1250704548c6ae1e3d7f7))
* **templating:** Tera v2 compat - template loading, shims, syntax ([6484d46](https://github.com/norgolith/core/commit/6484d464c4cc7063fab55c114316ff1a018e1f91))


### Refactoring

* **build:** split cmd/build.rs into assets, content, timings modules ([4f6d134](https://github.com/norgolith/core/commit/4f6d13431f7af772dacd7585c57298d27ed8e1e2))
* **cli:** remove redundant extension validation in new_asset ([670f912](https://github.com/norgolith/core/commit/670f9125f082c3c9a3e1836ea54d25a7d7083172))
* **config:** demote config/ directory to single config.rs ([2468ff2](https://github.com/norgolith/core/commit/2468ff24c510a78e51776fda9f612abe29f044e4))
* deduplicate plugin ABI constants between core and SDK ([ae1fb01](https://github.com/norgolith/core/commit/ae1fb01c3edecdaddee536c615d11a48b178002b))
* **dev:** deduplicate rewrite_urls logic ([198eaf3](https://github.com/norgolith/core/commit/198eaf368fb9027cf7983b016da36c60c2e0b8e5))
* **dev:** extract fast_path_lookup, rewrite_urls, html_response helpers ([9b634b2](https://github.com/norgolith/core/commit/9b634b25a2929af72aebc0d6e377a42d1b63a862))
* **dev:** split cmd/dev.rs into server, handlers, watcher modules ([6865741](https://github.com/norgolith/core/commit/6865741ba85eafb018574fb7e3707bafd433f8e2))
* **new:** replace AssetType enum with inline match ([670f912](https://github.com/norgolith/core/commit/670f9125f082c3c9a3e1836ea54d25a7d7083172))
* **plugin:** extract run_post_convert and run_post_render methods ([59641a0](https://github.com/norgolith/core/commit/59641a0016a079d7c4b6ec22d070b42d2890facf))
* **plugin:** extract shared build+install helpers ([6d7e43e](https://github.com/norgolith/core/commit/6d7e43ecc293eed49b10c16f3849e2fddb12797b))
* **plugin:** remove unused parse_status_response ([e21d91f](https://github.com/norgolith/core/commit/e21d91fc8b4e7c810ed8390b56d791089eff99d8))
* remove redundant titlecasing, inline metadata wrapper, drop cli delegates ([f11797b](https://github.com/norgolith/core/commit/f11797bafa97e59e2091eada5b63e5ae0f7da337))
* **sdk:** remove unused PreBuildContext and PostBuildContext ([e21d91f](https://github.com/norgolith/core/commit/e21d91fc8b4e7c810ed8390b56d791089eff99d8))
* **shared:** split shared/mod.rs into metadata and render submodules ([d6bfe7a](https://github.com/norgolith/core/commit/d6bfe7a75746f48e4d34d6fd535eb23e7fbe8a10))
* **tera:** merge tera_functions.rs and init_tera into tera module ([48d440b](https://github.com/norgolith/core/commit/48d440b3112fec6930a965d183f1f5f67d00817b))
* unify SitePaths, introduce BuildContext to kill too_many_arguments ([377097a](https://github.com/norgolith/core/commit/377097a3feb4e026b8935f7086e438649a56479e))


### Miscellaneous

* **deps:** bump hyper from 0.14.28 to 0.14.32 in /core ([#174](https://github.com/norgolith/core/issues/174)) ([60722c3](https://github.com/norgolith/core/commit/60722c33257e68dfbda9237decb4bb7f5295dce1))
* **deps:** bump minify-html from 0.15.0 to 0.18.1 in /core ([#173](https://github.com/norgolith/core/issues/173)) ([669b8ec](https://github.com/norgolith/core/commit/669b8ec8dd9f3d299268ec2a4342b4611055b061))
* **deps:** bump mockall from 0.13.1 to 0.15.0 in /core ([#175](https://github.com/norgolith/core/issues/175)) ([f4fb5b7](https://github.com/norgolith/core/commit/f4fb5b7e188e87b958cfb723cc4efa4734c7ce06))
* **deps:** bump tokio from 1.52.2 to 1.52.3 in /core ([#176](https://github.com/norgolith/core/issues/176)) ([ed550e3](https://github.com/norgolith/core/commit/ed550e35cd884ad2dea7e03f23e7db26a38feaa8))
* **deps:** bump tracing-subscriber from 0.3.19 to 0.3.23 in /core ([f46ec86](https://github.com/norgolith/core/commit/f46ec868247fa4b3f5fa621cf65a444aa6088c5f))
* format source code ([ea514a8](https://github.com/norgolith/core/commit/ea514a836f5c3965f9a6714b92bc37709e2c85fc))
* remove unused num_cpus and mockall dependencies ([001a58a](https://github.com/norgolith/core/commit/001a58ad896c0a10bcf067ad4f202689289407e3))


### Tests

* **plugin:** add unit tests for install sources ([6e1622a](https://github.com/norgolith/core/commit/6e1622ad90a46881ea7d949c1621e65724dfb004))
* **plugin:** fix CI git fixture by using explicit signature ([e2c2ad1](https://github.com/norgolith/core/commit/e2c2ad16c5ac80b81182ce7abede314241d2fe43))

## [0.5.0](https://github.com/norgolith/core/compare/norgolith-v0.4.0...norgolith-v0.5.0) (2026-07-01)


### Features

* add configurable content collections and categoriesDir ([e6d1a5c](https://github.com/norgolith/core/commit/e6d1a5cc75b7fc3c13e3945f0115d51631f68668))
* **build:** expose lith version with git commit hash for dev builds ([111ee70](https://github.com/norgolith/core/commit/111ee70318e090c72f41d197fba20e4d4b09d0b8))
* **config:** add SiteConfig field validation ([af6a40f](https://github.com/norgolith/core/commit/af6a40f230cb01a177c84b2572c1c69354156ac6))
* **dev:** config hot-reloading ([1728505](https://github.com/norgolith/core/commit/1728505404b5c27661d9913910fcdeb7a5b179f9))
* **dev:** pre-render all pages into memory for instant responses ([485bbb0](https://github.com/norgolith/core/commit/485bbb0f9e44ac09248be94c1deda303b0129c67))
* incremental builds via content-hash caching ([466cd8d](https://github.com/norgolith/core/commit/466cd8d1d639a2cf4dd243be3376b3eae27e234c))
* **plugin:** add C ABI types and PluginManager data structures ([47f1876](https://github.com/norgolith/core/commit/47f1876))
* **plugin:** add plugin loading and validation ([e0959e0](https://github.com/norgolith/core/commit/e0959e0))
* **plugin:** add safety wrappers with catch_unwind, timeout, and memory management ([9389e6d](https://github.com/norgolith/core/commit/9389e6d))
* **plugin:** add Landlock sandbox for filesystem confinement ([3886a41](https://github.com/norgolith/core/commit/3886a41))
* **plugin:** wire hook points into build pipeline ([728d41e](https://github.com/norgolith/core/commit/728d41e))
* **plugin:** add CLI commands for plugin management ([9dfd509](https://github.com/norgolith/core/commit/9dfd509))
* **sdk:** implement bridge functions and working register_plugin! macro ([48f3f9b](https://github.com/norgolith/core/commit/48f3f9b1eeec076be6b87c36773944395c4ee8e5))
* add SEO (sitemap.xml, robots.txt) and OpenGraph meta tags ([d24c924](https://github.com/norgolith/core/commit/d24c924))
* use rust-norg performance increase branch (experimental) ([34f11c4](https://github.com/norgolith/core/commit/34f11c4a6d023331c1a4deee528cb6532912d275))
* use XDG_CACHE_HOME for incremental build cache ([c2c6370](https://github.com/norgolith/core/commit/c2c63707fc867c7dae98163572ec34d08eb0c623))


### Bug Fixes

* **build:** join validation errors with newline for readability ([3106055](https://github.com/norgolith/core/commit/3106055c892de6c2e12e195345c3794b45eb0c59))
* **build:** log WalkDir errors instead of silently discarding ([412f0e1](https://github.com/norgolith/core/commit/412f0e1fcfe791ff07ee0a7fdd27372ae4722633))
* **build:** only validate RSS templates as RSS ([7c73022](https://github.com/norgolith/core/commit/7c7302216cdce40ee9ed8b278631bf1c967602fa))
* **build:** replace bare unwraps with proper error handling ([607e46d](https://github.com/norgolith/core/commit/607e46d44032d9e55a08b645a327fd02755a0d64))
* **cache:** populate build cache for incremental builds ([4e6d4fe](https://github.com/norgolith/core/commit/4e6d4fe3ca9bbfb05f3bad1b3818d18593eac98d))
* **config:** do not allow negative numbers in RSS ttl values ([9e3a7ee](https://github.com/norgolith/core/commit/9e3a7eec6d745a88ba66f389cce15dea9eb2452c))
* **dev:** acquire posts lock once in category index handler ([f784044](https://github.com/norgolith/core/commit/f784044bc0af9b676f602f8c354ffbd41f4dcd5a))
* **dev:** don't crash dev server when browser can't open ([2b6a2de](https://github.com/norgolith/core/commit/2b6a2de30dd6ceb63f9f60390003f3aa9a1d53f7))
* **dev:** posts list empty in templates due to collection key collision ([992c804](https://github.com/norgolith/core/commit/992c804bd77de47e70cdf86c038154c3f7fcb85d))
* **dev:** uppercase Ok in send_reload ([fd81af1](https://github.com/norgolith/core/commit/fd81af14a2cfa487432356abdb0ff81fbb071bf3))
* **dev:** use strip_prefix result directly instead of contains string check ([ddd2f90](https://github.com/norgolith/core/commit/ddd2f9042631df6e220126f996f7c1af23f35ad9))
* **docs:** improve site layout and center the content ([f8e8ced](https://github.com/norgolith/core/commit/f8e8ced214962208f5b04a0013396ca8453f5866))
* **fs:** remove redundant empty-dir check in find_in_previous_dirs ([2b6a2de](https://github.com/norgolith/core/commit/2b6a2de30dd6ceb63f9f60390003f3aa9a1d53f7))
* **init:** typo in Norgolith ([d7009c6](https://github.com/norgolith/core/commit/d7009c6cfa9eea5110048c9af877f05a13009dd0))
* **net:** eliminate TOCTOU port race in dev server ([f901bb3](https://github.com/norgolith/core/commit/f901bb36b916d410244e717d71e0dd1d7a3eb1e7))
* **plugin:** harden plugin system ([4253d30](https://github.com/norgolith/core/commit/4253d304cc98788a10389252621b7efb6606131c))
* **plugin:** remove double JSON extraction in hook handlers ([cb39e6c](https://github.com/norgolith/core/commit/cb39e6c1db29e05ccb8ce02fd19a6a3bc93dcf06))
* **plugin:** rewrite plugin list output with vertical per-plugin layout ([3df145b](https://github.com/norgolith/core/commit/3df145b))
* **preview:** add percent-decoding to sanitize_path ([f1f3cc7](https://github.com/norgolith/core/commit/f1f3cc791c0cd7b0f2649f47bf45aca6797bef64))
* **schema:** return `ConstraintViolation` schema error instead of panicking on invalid regex patterns ([071c23b](https://github.com/norgolith/core/commit/071c23b0ad9fd01bf0d445d5b15e89754995eafa))
* **schema:** send warning message when a condition is absent from post metadata ([a446158](https://github.com/norgolith/core/commit/a4461584419501367f27a12d4e4c8828a191b06f))
* **schema:** validate array item types against items definition ([f43ad49](https://github.com/norgolith/core/commit/f43ad495966ef4cf2c86092f6ebbb6665781cd0b))
* **shared:** use starts_with for collection permalink matching ([03a56c5](https://github.com/norgolith/core/commit/03a56c569603c9e63353993db0cbdc3ee5420dfc))
* **shared:** warn on invalid post date instead of silently sorting to epoch ([420e4cf](https://github.com/norgolith/core/commit/420e4cf080c1ef7dbdb7d12e8c610ac0728a7ecb))
* **shared:** warn on metadata conversion errors instead of silently dropping ([70c87ea](https://github.com/norgolith/core/commit/70c87ea388d37e668a2d8d7239cd183a3a7764b9))
* **tera:** escape HTML in TOC output to prevent XSS ([bbd6607](https://github.com/norgolith/core/commit/bbd66074a4f29957e632323b272560ae763c0412))
* **tera:** replace panic-inducing unwraps with proper error propagation ([1d04c86](https://github.com/norgolith/core/commit/1d04c860a87f01bf4771c3038987ff173addd6db))
* **theme:** handle root path in backup dir resolution ([988ecf2](https://github.com/norgolith/core/commit/988ecf200916206e450fac9fdf6d5bc728b5ef2c))
* **theme:** move blocking I/O off tokio runtime ([6e9cc5f](https://github.com/norgolith/core/commit/6e9cc5f959a37a4121af6ca184049283a535117d))
* **theme:** use to_string_lossy for non-UTF-8 filenames ([ae291a0](https://github.com/norgolith/core/commit/ae291a03a22a957714a250c92c9ef489333dc371))


### Performance Improvements

* **build:** buffer rendered pages and write sequentially ([efbac38](https://github.com/norgolith/core/commit/efbac382f2292c8773c7ad6d5ed7bb04c1978758))
* **build:** cache href regex with OnceLock ([1d86e9b](https://github.com/norgolith/core/commit/1d86e9b7c39de3645fca0c47b879eda656d94ae4))
* **build:** migrate build.rs to sync rayon parallelism ([6c3e083](https://github.com/norgolith/core/commit/6c3e0834a880709b35156137313039fd803f4664))
* **build:** use RwLock for build cache ([0f8d545](https://github.com/norgolith/core/commit/0f8d545a8e4a6da2a1a80f4f6736ec1cf1f04e17))
* **cache:** avoid recomputing global hash on save ([b41e6f6](https://github.com/norgolith/core/commit/b41e6f6d2417c74dde5b55bc8743ad2962a74ea7))
* eliminate double parsing for collection posts ([8ee9910](https://github.com/norgolith/core/commit/8ee991023cf91167fb7eba9a890f4976ac66f2cc))
* optimize build pipeline with shared context, VecDeque, and parallel metadata ([0500ec7](https://github.com/norgolith/core/commit/0500ec7078b8f530dd3930671707c4b0938d6a81))
* pass carryover tags by reference in HTML converter ([a7d8e25](https://github.com/norgolith/core/commit/a7d8e257b75f03448ae8d19265fa033afe45fd74))
* skip HTML conversion for draft posts ([ec57c38](https://github.com/norgolith/core/commit/ec57c3801069ba31d31cda760dce221dbb136314))
* skip HTML conversion for draft posts in dev server too ([c628321](https://github.com/norgolith/core/commit/c62832127abeb3b8ab5181387d7c832a74289eee))


### Refactoring

* **converter:** eliminate format! allocations and drain-after-clone ([d303740](https://github.com/norgolith/core/commit/d303740))
* **theme:** extract find_theme_dir() helper ([adc67de](https://github.com/norgolith/core/commit/adc67de))
* **workspace:** migrate to monorepo with core/ and sdk/ crates ([3e27e27](https://github.com/norgolith/core/commit/3e27e273181d2f01af4d2c5b9057b32432f15d1a))


### Documentation

* document plugins ([0d584d8](https://github.com/norgolith/core/commit/0d584d85ed4a5e97699dd580b52e2512ce476c3a))

## [0.4.0](https://github.com/NTBBloodbath/norgolith/compare/v0.3.2...v0.4.0) (2026-05-15)


### Features

* auto-discover and render all XML templates as feeds ([11ba1e7](https://github.com/NTBBloodbath/norgolith/commit/11ba1e775c7466365bb39dafcd8ed5630099d805)), closes [#111](https://github.com/NTBBloodbath/norgolith/issues/111)
* **build:** add styled per-step output to lith build command ([9471821](https://github.com/NTBBloodbath/norgolith/commit/94718219ca4f9b97b4e644031d07eaf616c403b6))
* **dev:** add colored, compact request logging to dev server ([ca22a39](https://github.com/NTBBloodbath/norgolith/commit/ca22a3954eef0f477a894989e63b1d1a93d826e2))
* **dev:** add Ctrl-D for graceful development server shutdown ([39932de](https://github.com/NTBBloodbath/norgolith/commit/39932de1fc78f92b072be7643f7f4a616b3af361))
* **dev:** higher padding between HTTP Path and HTTP status indicator ([609dd76](https://github.com/NTBBloodbath/norgolith/commit/609dd76b4592f8ceac97bc888066f0149a3c8a33))
* **dev:** resolve symlinks in watched site paths ([140a173](https://github.com/NTBBloodbath/norgolith/commit/140a173e0d45f5466e3e30cfa5e8c80ee3781901))


### Bug Fixes

* **converter:** handle NorgAST::List variant introduced in latest rust-norg ([8066641](https://github.com/NTBBloodbath/norgolith/commit/80666411eeca341ba29491bd4533caa4b6954441))
* **converter:** repair unreachable panic for NorgAST::List with Quote type ([8066641](https://github.com/NTBBloodbath/norgolith/commit/80666411eeca341ba29491bd4533caa4b6954441))

## [0.3.2](https://github.com/NTBBloodbath/norgolith/compare/v0.3.1...v0.3.2) (2026-05-06)


### Bug Fixes

* **build:** pin tracing-subscriber to 0.3.19 and update dependencies ([e9bfe6c](https://github.com/NTBBloodbath/norgolith/commit/e9bfe6c72d6e18960dc55052b4974da972232b48))
* **build:** remove `String::leak()` in `minify_css_asset` ([fa19733](https://github.com/NTBBloodbath/norgolith/commit/fa1973378cfc7bb629e342e513104ef4f799b947))
* **clippy:** avoid owned PathBuf allocation in posts filter comparison ([acf7b62](https://github.com/NTBBloodbath/norgolith/commit/acf7b6252f864604fb3d19850091555c3417b844))
* keep public .git directory during build ([6fd806d](https://github.com/NTBBloodbath/norgolith/commit/6fd806db4d9d34d1a4315ff81376d40f2f424821))
* load XML templates (rss.xml) from theme/templates ([069d958](https://github.com/NTBBloodbath/norgolith/commit/069d95814b539f0bbc7c2ac78a604683aafc7923))
* **schema:** correct array min/max constraint operators and add tests ([d1cf410](https://github.com/NTBBloodbath/norgolith/commit/d1cf410326b157f0e12326192dadd3e4cbe5b873))
* **shared:** handle sourceless Tera errors in category render functions ([78a66f3](https://github.com/NTBBloodbath/norgolith/commit/78a66f3e44894577d3da719d34e137d7c6e63aa2))
* **shared:** properly pass the layout name when failing to render a template in `render_norg_page` ([15b7a48](https://github.com/NTBBloodbath/norgolith/commit/15b7a48e90868a567af4e3584ff36c60d6fc8ae4))
* **shared:** sort posts by `created` field using RFC3339 date parsing ([69b0855](https://github.com/NTBBloodbath/norgolith/commit/69b08550ce194185132df6edd65f2ad2e6314c22))
