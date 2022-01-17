{
  description = "Dura build and development environment";

  # Provides abstraction to boiler-code when specifying multi-platform outputs.
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlay ];
        };

        dura = pkgs.rustPlatform.buildRustPackage {
            pname = "dura";
            version = "unstable-${self.lastModifiedDate}";
            description = "A background process that saves uncommited changes on git";

            src = self;

            cargoLock = {
              lockFile = self + "/Cargo.lock";
            };

            buildInputs = [
              pkgs.openssl
            ];

            nativeBuildInputs = [
              pkgs.rust-bin.stable.latest.minimal
              pkgs.pkg-config
            ];
          };

        packages = flake-utils.lib.flattenTree {
            inherit dura;
        };

        apps = {
            dura = flake-utils.lib.mkApp { drv = packages.dura; };
        };
      in
      rec {
        defaultPackage = packages.dura;
        defaultApp = apps.dura;
        devShell = pkgs.mkShell {
          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";

          buildInputs = [
            pkgs.openssl
            pkgs.pkgconfig
            (pkgs.rust-bin.stable.latest.default.override { extensions = [ "rust-src" ]; })
          ];
        };

      });
}
