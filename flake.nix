{
  description = "Inngest Rust SDK";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          config.allowUnfree = true;
          overlays = [ rust-overlay.overlays.default ];
        };

        rustToolchain = pkgs.rust-bin.stable."1.94.0".default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
            "clippy"
            "rustfmt"
          ];
        };

        inngestRelease = {
          version = "1.17.9";
          x86_64-darwin = {
            os = "darwin";
            arch = "amd64";
            hash = "sha256-9gbox1f9nABWf/KP7QIJDBqWk6VR+sosJuY7HmwHC9g=";
          };
          aarch64-darwin = {
            os = "darwin";
            arch = "arm64";
            hash = "sha256-uk4e0/gOVEJFAr1Z2eVzCf8abR8IPHbYadQKmH5Y97w=";
          };
          x86_64-linux = {
            os = "linux";
            arch = "amd64";
            hash = "sha256-Vh9uCOBNzdKfZngrhupWjntWMJvNBzYg/sGPGaBoOeI=";
          };
          aarch64-linux = {
            os = "linux";
            arch = "arm64";
            hash = "sha256-iM6sackbdmz6Sbge2Xjx8kNlZ6uyUToE2NiojlSn82s=";
          };
        };

        inngestAsset =
          inngestRelease.${system}
            or (throw "Unsupported system for inngest CLI release: ${system}");

        inngestCli = pkgs.stdenvNoCC.mkDerivation {
          pname = "inngest";
          inherit (inngestRelease) version;

          src = pkgs.fetchurl {
            url =
              "https://github.com/inngest/inngest/releases/download/v${inngestRelease.version}/"
              + "inngest_${inngestRelease.version}_${inngestAsset.os}_${inngestAsset.arch}.tar.gz";
            inherit (inngestAsset) hash;
          };

          nativeBuildInputs = with pkgs; [
            gnutar
            gzip
          ];

          sourceRoot = ".";
          dontConfigure = true;
          dontBuild = true;

          installPhase = ''
            runHook preInstall
            install -Dm755 inngest $out/bin/inngest
            install -Dm644 LICENSE.md $out/share/licenses/inngest/LICENSE.md
            runHook postInstall
          '';
        };
      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            rustToolchain
            inngestCli

            # tools
            git-cliff

            # deps
            pkg-config
            openssl

            # LSP
            rust-analyzer
            yaml-language-server
          ];

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        };
      }
    );
}
