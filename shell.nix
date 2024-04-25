let pkgs = import <nixos-23.11> { };

in pkgs.mkShell {
  nativeBuildInputs = [
    # Rust
    pkgs.rustc
    pkgs.rustup
    pkgs.rustfmt
    pkgs.cargo

    # deps
    pkgs.pkg-config
    pkgs.openssl

    # LSP
    pkgs.rust-analyzer
  ];

  mkShell = ''
    PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
  '';
}
