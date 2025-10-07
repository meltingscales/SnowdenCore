use anyhow::{Context, Result};
use clap::Parser;
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use std::fs::create_dir_all;
use std::process::Command;

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

fn create_video_precise_timing(
    selected_pngs: &[&PathBuf],
    jump_cut_seconds: f64,
    mp3_path: &Path,
    output_path: &Path,
    framerate: u32,
) -> Result<()> {
    const WIDTH: u32 = 1280;
    const HEIGHT: u32 = 720;
    
    println!("Creating video with {} images...", selected_pngs.len());
    
    // Calculate exact frames per image - this is the key to precise timing
    let frames_per_image = (framerate as f64 * jump_cut_seconds).round() as u32;
    println!("Frames per image: {} (for {:.3}s at {}fps)", frames_per_image, jump_cut_seconds, framerate);
    
    // Create temporary directory for frames
    let temp_dir = PathBuf::from("temp_frames");
    create_dir_all(&temp_dir)?;
    
    // Create a precise frame list
    let mut frame_files = Vec::new();
    let mut frame_number = 0;
    
    for (i, png_path) in selected_pngs.iter().enumerate() {
        println!("Processing image {}/{}: {}", i + 1, selected_pngs.len(), png_path.display());
        
        // Load and resize image - skip if corrupted
        let resized = match image::open(png_path) {
            Ok(img) => img.resize_exact(WIDTH, HEIGHT, image::imageops::FilterType::Lanczos3),
            Err(e) => {
                println!("Warning: Skipping corrupted image {}: {}", png_path.display(), e);
                continue;
            }
        };
        
        // Create exactly frames_per_image copies of this image
        for _ in 0..frames_per_image {
            let frame_path = temp_dir.join(format!("frame_{:06}.png", frame_number));
            if let Err(e) = resized.save(&frame_path) {
                println!("Warning: Failed to save frame {}: {}", frame_path.display(), e);
                continue;
            }
            frame_files.push(frame_path);
            frame_number += 1;
        }
    }
    
    println!("Generated {} total frames", frame_files.len());
    
    // Create ffmpeg command with precise timing
    println!("Encoding video with ffmpeg...");
    let output = Command::new("ffmpeg")
        .arg("-y") // Overwrite output file
        .arg("-framerate").arg(framerate.to_string())
        .arg("-i").arg(temp_dir.join("frame_%06d.png"))
        .arg("-i").arg(mp3_path)
        .arg("-c:v").arg("libx264")
        .arg("-c:a").arg("aac")
        .arg("-pix_fmt").arg("yuv420p")
        .arg("-shortest") // Stop when shortest input ends
        .arg("-r").arg(framerate.to_string()) // Output framerate
        .arg(output_path)
        .output()
        .context("Failed to run ffmpeg")?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("ffmpeg failed: {}", stderr));
    }
    
    // Clean up temporary frames
    println!("Cleaning up temporary frames...");
    std::fs::remove_dir_all(&temp_dir).ok();
    
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
    create_video_precise_timing(
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