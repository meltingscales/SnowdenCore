use anyhow::{Context, Result};
use clap::Parser;
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(name = "generate-video")]
#[command(about = "Generate video from MP3 and random PNG images")]
struct Args {
    /// Time between PNG images in seconds
    #[arg(short = 'j', long, default_value = "0.1")]
    jump_cut_seconds: f64,
    
    /// Path to MP3 file
    #[arg(short = 's', long)]
    song_path: PathBuf,
    
    /// Output video file path
    #[arg(short = 'o', long)]
    output_video: PathBuf,
    
    /// Directory containing PNG images
    #[arg(long, default_value = "Snowden-PNGs")]
    png_dir: PathBuf,
    
    /// Framerate for output video
    #[arg(long, default_value = "30")]
    framerate: u32,
}

fn get_mp3_duration(mp3_path: &Path) -> Result<f64> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(mp3_path)
        .output()
        .context("Failed to run ffprobe - is ffmpeg installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("ffprobe failed: {}", stderr));
    }

    let duration_str = String::from_utf8_lossy(&output.stdout);
    let duration: f64 = duration_str.trim().parse()
        .context("Failed to parse duration from ffprobe output")?;
    
    Ok(duration)
}

fn find_png_files(png_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut png_files = Vec::new();
    
    for entry in WalkDir::new(png_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Some(extension) = entry.path().extension() {
                if extension.to_string_lossy().to_lowercase() == "png" {
                    png_files.push(entry.path().to_path_buf());
                }
            }
        }
    }
    
    Ok(png_files)
}

fn select_random_pngs(png_files: &[PathBuf], needed_count: usize) -> Vec<&PathBuf> {
    let mut rng = thread_rng();
    
    if png_files.len() >= needed_count {
        // If we have enough unique PNGs, sample without replacement
        let mut selected: Vec<&PathBuf> = png_files.iter().collect();
        selected.shuffle(&mut rng);
        selected.into_iter().take(needed_count).collect()
    } else {
        // If we don't have enough unique PNGs, repeat them
        let mut selected = Vec::with_capacity(needed_count);
        for i in 0..needed_count {
            let index = i % png_files.len();
            selected.push(&png_files[index]);
        }
        selected.shuffle(&mut rng);
        selected
    }
}

fn create_video_with_ffmpeg(
    selected_pngs: &[&PathBuf],
    jump_cut_seconds: f64,
    mp3_path: &Path,
    output_path: &Path,
    framerate: u32,
) -> Result<()> {
    // Create a temporary file list for ffmpeg concat
    let filelist_path = "temp_filelist.txt";
    let mut filelist_content = String::new();
    
    for png_path in selected_pngs {
        filelist_content.push_str(&format!(
            "file '{}'\nduration {}\n",
            png_path.display(),
            jump_cut_seconds
        ));
    }
    // Add the last image again with a short duration to ensure it shows
    if let Some(last_png) = selected_pngs.last() {
        filelist_content.push_str(&format!("file '{}'\n", last_png.display()));
    }
    
    std::fs::write(filelist_path, filelist_content)
        .context("Failed to write temporary filelist")?;
    
    println!("Creating video with {} images...", selected_pngs.len());
    
    // Generate video using ffmpeg
    let output = Command::new("ffmpeg")
        .arg("-y") // Overwrite output file
        .arg("-f").arg("concat")
        .arg("-safe").arg("0")
        .arg("-i").arg(filelist_path)
        .arg("-i").arg(mp3_path)
        .arg("-vf").arg(format!("fps={},scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2", framerate))
        .arg("-c:v").arg("libx264")
        .arg("-c:a").arg("aac")
        .arg("-shortest") // Stop when shortest input ends
        .arg("-pix_fmt").arg("yuv420p")
        .arg(output_path)
        .output()
        .context("Failed to run ffmpeg")?;
    
    // Clean up temporary file
    std::fs::remove_file(filelist_path).ok();
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("ffmpeg failed: {}", stderr));
    }
    
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();
    
    println!("SnowdenCore Video Generator");
    println!("Song: {}", args.song_path.display());
    println!("Output: {}", args.output_video.display());
    println!("Jump cut: {} seconds", args.jump_cut_seconds);
    println!();
    
    // Check if MP3 file exists
    if !args.song_path.exists() {
        return Err(anyhow::anyhow!("MP3 file not found: {}", args.song_path.display()));
    }
    
    // Check if PNG directory exists
    if !args.png_dir.exists() {
        return Err(anyhow::anyhow!("PNG directory not found: {}", args.png_dir.display()));
    }
    
    // Get MP3 duration
    println!("Getting MP3 duration...");
    let mp3_duration = get_mp3_duration(&args.song_path)?;
    println!("MP3 duration: {:.2} seconds", mp3_duration);
    
    // Calculate how many images we need
    let needed_images = (mp3_duration / args.jump_cut_seconds).ceil() as usize;
    println!("Images needed: {}", needed_images);
    
    // Find all PNG files
    println!("Finding PNG files...");
    let png_files = find_png_files(&args.png_dir)?;
    println!("Found {} PNG files", png_files.len());
    
    if png_files.is_empty() {
        return Err(anyhow::anyhow!("No PNG files found in {}", args.png_dir.display()));
    }
    
    // Check if we have enough content (warn if we need to repeat)
    if png_files.len() < needed_images {
        println!("Warning: Only {} PNGs available, but {} needed. Images will be repeated.", 
                png_files.len(), needed_images);
    }
    
    // Select random PNGs
    println!("Selecting random PNG files...");
    let selected_pngs = select_random_pngs(&png_files, needed_images);
    
    // Create the video
    println!("Generating video...");
    create_video_with_ffmpeg(
        &selected_pngs,
        args.jump_cut_seconds,
        &args.song_path,
        &args.output_video,
        args.framerate,
    )?;
    
    println!("✓ Video created successfully: {}", args.output_video.display());
    
    // Show final stats
    let output_size = std::fs::metadata(&args.output_video)?
        .len() as f64 / (1024.0 * 1024.0);
    println!("Output file size: {:.2} MB", output_size);
    
    Ok(())
}