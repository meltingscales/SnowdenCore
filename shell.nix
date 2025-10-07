{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    rustc
    cargo
    poppler_utils
  ];

  shellHook = ''
    echo "Development environment loaded with Rust and poppler-utils"
    echo "Run 'just setup' to build the Rust binary"
    echo "Run 'just extract' to extract PDFs with parallel processing"
  '';
}