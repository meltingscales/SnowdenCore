{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    rustc
    cargo
    poppler_utils
    yt-dlp
    ffmpeg
    jq
  ];

  shellHook = ''
    echo "Development environment loaded with:"
    echo "  - Rust and poppler-utils for PDF extraction"
    echo "  - yt-dlp, ffmpeg, and jq for YouTube downloads"
    echo ""
    echo "Available commands:"
    echo "  just setup          - Build the Rust binary"
    echo "  just extract        - Extract PDFs with parallel processing"
    echo "  just download-youtube - Download YouTube videos and extract MP3s"
    echo "  just count          - Count PDF files"
    echo "  just count-pngs     - Count extracted PNG files"
  '';
}