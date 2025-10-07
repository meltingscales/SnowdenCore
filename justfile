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

# Download YouTube videos and extract MP3s
download-youtube:
    #!/usr/bin/env bash
    set -euo pipefail
    
    # Create directories
    mkdir -p videos mp3
    
    # Check if youtube-dl/yt-dlp is available
    if command -v yt-dlp >/dev/null 2>&1; then
        DOWNLOADER="yt-dlp"
    elif command -v youtube-dl >/dev/null 2>&1; then
        DOWNLOADER="youtube-dl"
    else
        echo "Error: Neither yt-dlp nor youtube-dl found. Please install one of them."
        exit 1
    fi
    
    echo "Using $DOWNLOADER for downloads..."
    
    # Read video IDs from JSON file
    if [[ ! -f "youtube-videos-to-mp3.json" ]]; then
        echo "Error: youtube-videos-to-mp3.json not found"
        exit 1
    fi
    
    # Parse JSON and download each video
    jq -r '.videos[]' youtube-videos-to-mp3.json | while read -r video_id; do
        echo "Processing video ID: $video_id"
        
        # Check if video already exists (any format)
        if ls videos/"$video_id".* >/dev/null 2>&1; then
            echo "  Video already downloaded, skipping download"
        else
            echo "  Downloading video..."
            $DOWNLOADER -f 'best[height<=720]/best/worst' -o "videos/%(id)s.%(ext)s" "https://www.youtube.com/watch?v=$video_id" || {
                echo "  Failed to download $video_id, trying audio-only..."
                $DOWNLOADER -f 'bestaudio/best' -o "videos/%(id)s.%(ext)s" "https://www.youtube.com/watch?v=$video_id" || {
                    echo "  Failed to download $video_id completely"
                    continue
                }
            }
        fi
        
        # Check if MP3 already exists
        if [[ -f "mp3/$video_id.mp3" ]]; then
            echo "  MP3 already exists, skipping extraction"
        else
            echo "  Extracting MP3..."
            # Find the downloaded video file
            video_file=$(ls videos/"$video_id".* 2>/dev/null | head -1)
            if [[ -n "$video_file" ]]; then
                ffmpeg -i "$video_file" -vn -acodec libmp3lame -ab 192k "mp3/$video_id.mp3" -y || {
                    echo "  Failed to extract MP3 from $video_id"
                    continue
                }
                echo "  ✓ Extracted MP3: $video_id.mp3"
            else
                echo "  Error: Could not find downloaded video file for $video_id"
            fi
        fi
    done
    
    echo "Download and extraction complete!"
    echo "Videos: $(ls videos/*.* 2>/dev/null | wc -l || echo 0)"
    echo "MP3s: $(ls mp3/*.mp3 2>/dev/null | wc -l || echo 0)"
