use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(name = "extract")]
#[command(about = "Extract PDF pages to PNG images with parallel processing")]
struct Args {
    /// Number of parallel workers (default: number of CPU cores)
    #[arg(short, long)]
    workers: Option<usize>,
    
    /// Skip files that have already been extracted
    #[arg(long, default_value = "true")]
    skip_existing: bool,
    
    /// DPI for image conversion (default: 200)
    #[arg(long, default_value = "200")]
    dpi: u32,
    
    /// Archive directory (default: "Snowden archive")
    #[arg(long, default_value = "Snowden archive")]
    archive_dir: PathBuf,
    
    /// Output directory (default: "Snowden-PNGs")
    #[arg(long, default_value = "Snowden-PNGs")]
    output_dir: PathBuf,
}

#[derive(Debug)]
struct ProcessingStats {
    processed: AtomicUsize,
    skipped: AtomicUsize,
    errors: AtomicUsize,
    total_pages: AtomicUsize,
}

impl ProcessingStats {
    fn new() -> Self {
        Self {
            processed: AtomicUsize::new(0),
            skipped: AtomicUsize::new(0),
            errors: AtomicUsize::new(0),
            total_pages: AtomicUsize::new(0),
        }
    }
}

fn check_if_extracted(pdf_path: &Path, output_dir: &Path) -> Result<bool> {
    let pdf_name = pdf_path.file_stem()
        .context("Failed to get PDF file stem")?
        .to_string_lossy();
    
    let first_page = output_dir.join(format!("{}_page001.png", pdf_name));
    Ok(first_page.exists())
}

fn extract_pdf_to_pngs(
    pdf_path: &Path,
    output_dir: &Path,
    skip_existing: bool,
    dpi: u32,
    stats: Arc<ProcessingStats>,
) -> Result<()> {
    let pdf_name = pdf_path.file_stem()
        .context("Failed to get PDF file stem")?
        .to_string_lossy()
        .to_string();

    // Check if already extracted
    if skip_existing && check_if_extracted(pdf_path, output_dir)? {
        stats.skipped.fetch_add(1, Ordering::Relaxed);
        return Ok(());
    }

    // Get file size for logging
    let metadata = std::fs::metadata(pdf_path)?;
    let file_size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
    
    println!("Processing: {} ({:.2} MB)", pdf_path.file_name().unwrap().to_string_lossy(), file_size_mb);

    match extract_pdf_pages_with_pdftoppm(pdf_path, output_dir, &pdf_name, dpi) {
        Ok(page_count) => {
            stats.processed.fetch_add(1, Ordering::Relaxed);
            stats.total_pages.fetch_add(page_count, Ordering::Relaxed);
            println!("  ✓ Completed: {} ({} pages)", pdf_path.file_name().unwrap().to_string_lossy(), page_count);
        }
        Err(e) => {
            stats.errors.fetch_add(1, Ordering::Relaxed);
            eprintln!("  ✗ ERROR processing {}: {}", pdf_path.file_name().unwrap().to_string_lossy(), e);
        }
    }

    Ok(())
}

fn extract_pdf_pages_with_pdftoppm(
    pdf_path: &Path,
    output_dir: &Path,
    pdf_name: &str,
    dpi: u32,
) -> Result<usize> {
    // Use pdftoppm (from poppler-utils) to convert PDF to PNG
    let output_prefix = output_dir.join(format!("{}_page", pdf_name));
    
    let output = Command::new("pdftoppm")
        .arg("-png")
        .arg("-r")
        .arg(dpi.to_string())
        .arg(pdf_path)
        .arg(&output_prefix)
        .output()
        .context("Failed to execute pdftoppm - is poppler-utils installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("pdftoppm failed: {}", stderr));
    }

    // Count the generated files to determine page count
    let page_count = count_generated_pages(output_dir, pdf_name)?;
    
    // Rename files to match our naming convention (page001, page002, etc.)
    rename_generated_files(output_dir, pdf_name, page_count)?;
    
    Ok(page_count)
}

fn count_generated_pages(output_dir: &Path, pdf_name: &str) -> Result<usize> {
    let mut count = 0;
    
    // pdftoppm generates files with names like "prefix-1.png", "prefix-2.png", etc.
    for entry in std::fs::read_dir(output_dir)? {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        
        // Check if this file matches our pattern
        if file_name.starts_with(&format!("{}_page-", pdf_name)) && file_name.ends_with(".png") {
            count += 1;
        }
    }
    
    Ok(count)
}

fn rename_generated_files(output_dir: &Path, pdf_name: &str, page_count: usize) -> Result<()> {
    // pdftoppm generates files like "prefix-1.png", "prefix-2.png"
    // We want to rename them to "pdfname_page001.png", "pdfname_page002.png"
    
    for i in 1..=page_count {
        let old_name = output_dir.join(format!("{}_page-{}.png", pdf_name, i));
        let new_name = output_dir.join(format!("{}_page{:03}.png", pdf_name, i));
        
        if old_name.exists() {
            std::fs::rename(&old_name, &new_name)
                .with_context(|| format!("Failed to rename {} to {}", old_name.display(), new_name.display()))?;
        }
    }
    
    Ok(())
}

fn find_pdf_files(archive_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut pdf_files = Vec::new();
    
    for entry in WalkDir::new(archive_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Some(extension) = entry.path().extension() {
                if extension.to_string_lossy().to_lowercase() == "pdf" {
                    pdf_files.push(entry.path().to_path_buf());
                }
            }
        }
    }
    
    pdf_files.sort();
    Ok(pdf_files)
}

fn main() -> Result<()> {
    let args = Args::parse();
    
    // Set number of rayon threads
    if let Some(workers) = args.workers {
        rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build_global()
            .context("Failed to set thread pool size")?;
    }
    
    let num_workers = rayon::current_num_threads();
    
    println!("SnowdenCore PDF Extractor (Rust)");
    println!("Workers: {}", num_workers);
    println!("Archive: {}", args.archive_dir.display());
    println!("Output: {}", args.output_dir.display());
    println!();
    
    // Create output directory
    std::fs::create_dir_all(&args.output_dir)?;
    
    // Find PDF files
    let pdf_files = find_pdf_files(&args.archive_dir)?;
    
    if pdf_files.is_empty() {
        println!("No PDF files found in {}", args.archive_dir.display());
        return Ok(());
    }
    
    println!("Found {} PDF files", pdf_files.len());
    
    // Check if pdftoppm is available
    if Command::new("pdftoppm").arg("-h").output().is_err() {
        eprintln!("Error: pdftoppm not found. Please install poppler-utils:");
        eprintln!("  Ubuntu/Debian: sudo apt-get install poppler-utils");
        eprintln!("  CentOS/RHEL: sudo yum install poppler-utils");
        eprintln!("  macOS: brew install poppler");
        std::process::exit(1);
    }
    
    // Setup progress tracking
    let stats = Arc::new(ProcessingStats::new());
    let progress = ProgressBar::new(pdf_files.len() as u64);
    progress.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .context("Failed to set progress bar template")?
            .progress_chars("#>-"),
    );
    
    // Process files in parallel using rayon
    pdf_files.par_iter().for_each(|pdf_file| {
        let result = extract_pdf_to_pngs(
            pdf_file,
            &args.output_dir,
            args.skip_existing,
            args.dpi,
            stats.clone(),
        );
        
        if let Err(e) = result {
            eprintln!("Error processing {}: {}", pdf_file.display(), e);
        }
        
        progress.inc(1);
    });
    
    progress.finish_with_message("Complete!");
    
    // Print final statistics
    let processed = stats.processed.load(Ordering::Relaxed);
    let skipped = stats.skipped.load(Ordering::Relaxed);
    let errors = stats.errors.load(Ordering::Relaxed);
    let total_pages = stats.total_pages.load(Ordering::Relaxed);
    
    println!();
    println!("{}", "=".repeat(60));
    println!("Complete!");
    println!("Processed: {} files", processed);
    println!("Skipped (already extracted): {} files", skipped);
    println!("Errors: {} files", errors);
    println!("Total: {} files", processed + skipped + errors);
    println!("Total pages extracted: {}", total_pages);
    println!("Output directory: {}", args.output_dir.display());
    
    Ok(())
}