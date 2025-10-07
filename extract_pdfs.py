#!/usr/bin/env python3
"""
Extract every page from PDF files in ./Snowden archive/ to separate PNG images.
Output directory: ./Snowden-PNGs/
"""

import os
import sys
from pathlib import Path
from pdf2image import convert_from_path
from tqdm import tqdm

def check_if_extracted(pdf_path, output_dir):
    """Check if a PDF has already been extracted."""
    pdf_name = Path(pdf_path).stem
    # Check if at least one page exists (page001.png)
    first_page = output_dir / f"{pdf_name}_page001.png"
    return first_page.exists()

def extract_pdf_to_pngs(pdf_path, output_dir, skip_existing=True):
    """Extract all pages from a PDF to separate PNG files."""
    pdf_name = Path(pdf_path).stem

    # Skip if already extracted
    if skip_existing and check_if_extracted(pdf_path, output_dir):
        return None  # Return None to indicate skipped

    try:
        # Get file size for logging
        file_size_mb = pdf_path.stat().st_size / (1024 * 1024)
        tqdm.write(f"Processing: {pdf_path.name} ({file_size_mb:.2f} MB)")

        # Convert PDF pages to images
        images = convert_from_path(pdf_path, dpi=200)
        num_pages = len(images)

        tqdm.write(f"  → {num_pages} pages to extract")

        for i, image in enumerate(images, start=1):
            # Create output filename: pdfname_page001.png
            output_filename = f"{pdf_name}_page{i:03d}.png"
            output_path = output_dir / output_filename

            # Save the image
            image.save(output_path, 'PNG')

        tqdm.write(f"  ✓ Completed: {pdf_path.name}")
        return num_pages

    except Exception as e:
        tqdm.write(f"  ✗ ERROR processing {pdf_path.name}: {e}")
        return 0

def main():
    # Setup directories
    archive_dir = Path("Snowden archive")
    output_dir = Path("Snowden-PNGs")

    # Create output directory if it doesn't exist
    output_dir.mkdir(exist_ok=True)

    # Find all PDF files
    pdf_files = sorted(archive_dir.glob("*.pdf"))

    if not pdf_files:
        print("No PDF files found in ./Snowden archive/")
        return

    print(f"Found {len(pdf_files)} PDF files")
    print(f"Output directory: {output_dir}/\n")

    total_pages = 0
    processed_files = 0
    skipped_files = 0

    # Process PDFs with progress bar
    for pdf_file in tqdm(pdf_files, desc="Extracting PDFs", unit="file"):
        pages = extract_pdf_to_pngs(pdf_file, output_dir)

        if pages is None:
            skipped_files += 1
        elif pages > 0:
            total_pages += pages
            processed_files += 1

    print(f"\n{'='*60}")
    print(f"Complete!")
    print(f"Processed: {processed_files} files")
    print(f"Skipped (already extracted): {skipped_files} files")
    print(f"Total: {len(pdf_files)} files")
    print(f"Total pages extracted: {total_pages}")
    print(f"Output directory: {output_dir}/")

if __name__ == "__main__":
    main()
