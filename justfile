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

# Extract all PDF pages to PNG images
extract:
    uv run extract_pdfs.py

# Clean generated files
clean:
    rm -f hashes-merged.hash hashes-tmp.txt
    rm -rf Snowden-PNGs/
    rm -rf .venv/

# Clean only PNG outputs
clean-pngs:
    rm -rf Snowden-PNGs/

# Setup: install system dependencies and Python packages
setup:
    @echo "Installing system dependencies..."
    @echo "Note: You may need to run: sudo apt-get install poppler-utils"
    uv sync

# Run full workflow: validate archive, then extract PDFs
run: validate extract

# Count PDFs in archive
count:
    @echo "PDF files in archive:"
    @find "Snowden archive" -type f -name "*.pdf" | wc -l
