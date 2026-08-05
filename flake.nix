{
  description = "Norg static site generator and plugin SDK";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs =
    {
      self,
      nixpkgs,
      ...
    }:
    let
      systems = nixpkgs.lib.systems.doubles.linux ++ nixpkgs.lib.systems.doubles.darwin;
      eachSystem = f: nixpkgs.lib.genAttrs systems (s: f nixpkgs.legacyPackages.${s});
    in
    {
      formatter = eachSystem (pkgs: pkgs.nixfmt-tree);

      packages = eachSystem (
        pkgs:
        let
          corePackage = (pkgs.lib.importTOML "${self}/core/Cargo.toml").package;
          sdkPackage = (pkgs.lib.importTOML "${self}/sdk/Cargo.toml").package;
          mcpPackage = (pkgs.lib.importTOML "${self}/norgolith-mcp/Cargo.toml").package;
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = corePackage.name;
            version = corePackage.version;
            src = pkgs.lib.cleanSource "${self}";

            cargoLock = {
              lockFile = "${self}/Cargo.lock";
              allowBuiltinFetchGit = true;
            };
            useNextest = true;
            dontUseCargoParallelTests = true;

            nativeBuildInputs = [
              pkgs.pkg-config
            ];

            buildInputs = [
              pkgs.libgit2
              pkgs.openssl
              pkgs.zlib
            ];

            env = {
              LIBGIT2_NO_VENDOR = true;
              OPENSSL_NO_VENDOR = true;
            };

            __darwinAllowLocalNetworking = true;

            meta = {
              description = corePackage.description;
              homepage = corePackage.repository;
              license = pkgs.lib.licenses.gpl2Only;
              maintainers = corePackage.authors;
            };

            # For other makeRustPlatform features see:
            # https://github.com/NixOS/nixpkgs/blob/master/doc/languages-frameworks/rust.section.md#cargo-features-cargo-features
          };

          norgolith-plugin-sdk = pkgs.rustPlatform.buildRustPackage {
            pname = sdkPackage.name;
            version = sdkPackage.version;
            src = pkgs.lib.cleanSource "${self}";

            cargoLock = {
              lockFile = "${self}/sdk/Cargo.lock";
              allowBuiltinFetchGit = true;
            };
            cargoRoot = "sdk";
            buildAndTestSubdir = "sdk";

            meta = {
              description = sdkPackage.description;
              homepage = sdkPackage.repository;
              license = pkgs.lib.licenses.gpl2Only;
              maintainers = sdkPackage.authors;
            };
          };

          norgolith-mcp = pkgs.rustPlatform.buildRustPackage {
            pname = mcpPackage.name;
            version = mcpPackage.version;
            src = pkgs.lib.cleanSource "${self}";

            cargoLock = {
              lockFile = "${self}/norgolith-mcp/Cargo.lock";
              allowBuiltinFetchGit = true;
            };
            cargoRoot = "norgolith-mcp";
            buildAndTestSubdir = "norgolith-mcp";

            meta = {
              description = mcpPackage.description;
              homepage = mcpPackage.repository;
              license = pkgs.lib.licenses.gpl2Only;
              maintainers = mcpPackage.authors;
            };
          };
        }
      );

      devShells = eachSystem (pkgs: {
        default = pkgs.mkShell {
          nativeBuildInputs = [
            pkgs.rustPlatform.rustLibSrc

            pkgs.cargo
            pkgs.rustc
            pkgs.clippy
            pkgs.rustfmt
            pkgs.cargo-edit
            pkgs.cargo-nextest
            pkgs.rust-analyzer
            pkgs.pkg-config # Required by git2 crate
            pkgs.openssl # Required by git2 crate

            # Documentation site dev tools
            pkgs.tailwindcss_4
            pkgs.mprocs
            pkgs.watchman
            pkgs.tailwindcss-language-server
          ];

          env = {
            # Many editors rely on this rust-src PATH variable
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";

            PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
          };
        };
      });
    };

  nixConfig = {
    extra-substituters = [ "https://ntbbloodbath.cachix.org" ];
    extra-trusted-public-keys = [
      "ntbbloodbath.cachix.org-1:L4DjjGwDB6O3BJ4SmtYTZbvWKLi+1v/hRlLWKOtq+f0="
    ];
  };
}
