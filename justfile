# SnowdenCore justfile

# List all available commands
default:
    @just --list

# Generate hash of all files in Snowden archive
hash:
    ./generate-hash.bash

# Validate integrity of Snowden archive against stored hash
validate:
    ./validate-hash.bash

# Extract all PDF pages to PNG images (Rust implementation with parallelization)
extract:
    cargo run --release --bin extract

# Clean generated files
clean:
    rm -f hashes-merged.hash hashes-tmp.txt
    rm -rf Snowden-PNGs/
    rm -rf .venv/

# Clean only PNG outputs
clean-pngs:
    rm -rf Snowden-PNGs/

# Setup: install system dependencies and build Rust binary
setup:
    @echo "Building Rust binary..."
    cargo build --release --bin extract

# Run full workflow: validate archive, then extract PDFs
run: validate extract

# Count PDFs in archive
count:
    @echo "PDF files in archive:"
    @find "Snowden archive" -type f -name "*.pdf" | wc -l

# Count extracted PNG files
count-pngs:
    @echo "PNG files extracted:"
    @find "Snowden-PNGs" -type f -name "*.png" 2>/dev/null | wc -l || echo "0"

# Generate video from MP3 and random PNG images
# Usage: just generate-video JUMP_CUT_SECONDS SONG_NAME OUTPUT_VIDEO_NAME
# Example: just generate-video 0.1 "./mp3/somefile.mp3" "./somevideo.mp4"
generate-video JUMP_CUT_SECONDS SONG_NAME OUTPUT_VIDEO_NAME:
    cargo run --release --bin generate-video -- \
        --jump-cut-seconds {{JUMP_CUT_SECONDS}} \
        --song-path "{{SONG_NAME}}" \
        --output-video "{{OUTPUT_VIDEO_NAME}}"

# Generate video with custom PNG directory
# Usage: just generate-video-custom JUMP_CUT_SECONDS SONG_NAME OUTPUT_VIDEO_NAME PNG_DIR
generate-video-custom JUMP_CUT_SECONDS SONG_NAME OUTPUT_VIDEO_NAME PNG_DIR:
    cargo run --release --bin generate-video -- \
        --jump-cut-seconds {{JUMP_CUT_SECONDS}} \
        --song-path "{{SONG_NAME}}" \
        --output-video "{{OUTPUT_VIDEO_NAME}}" \
        --png-dir "{{PNG_DIR}}"

