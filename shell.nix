{ pkgs ? import (fetchTarball
  "https://github.com/NixOS/nixpkgs/archive/refs/tags/23.05.tar.gz") { } }:

with pkgs;

mkShell {
  buildInputs = [
    # Rust
    pkgs.rustc
    pkgs.rustfmt
    pkgs.rustup

    # LSPs
    pkgs.rust-analyzer
  ];
}
