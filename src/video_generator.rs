use anyhow::{Context, Result};
use clap::Parser;
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use std::fs::create_dir_all;
use std::process::Command;
use std::collections::VecDeque;
use image::GenericImageView;
use rayon::prelude::*;
use std::sync::Arc;

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
    
    /// Generate mobile-friendly video (9:16 aspect ratio with stacked images)
    #[arg(long, default_value = "false")]
    mobile_format: bool,
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

struct CircularImageQueue {
    images: VecDeque<PathBuf>,
    all_images: Vec<PathBuf>,
}

impl CircularImageQueue {
    fn new(mut images: Vec<PathBuf>) -> Self {
        let mut rng = thread_rng();
        images.shuffle(&mut rng);
        
        let all_images = images.clone();
        let queue = VecDeque::from(images);
        
        CircularImageQueue {
            images: queue,
            all_images,
        }
    }
    
    fn next_images(&mut self, count: usize) -> Vec<PathBuf> {
        let mut result = Vec::with_capacity(count);
        
        for _ in 0..count {
            if self.images.is_empty() {
                // Refill and shuffle when empty
                let mut rng = thread_rng();
                let mut fresh_images = self.all_images.clone();
                fresh_images.shuffle(&mut rng);
                self.images = VecDeque::from(fresh_images);
            }
            
            if let Some(image) = self.images.pop_front() {
                result.push(image);
            }
        }
        
        result
    }
}

fn smart_crop_image(img: &image::DynamicImage, target_width: u32, target_height: u32) -> image::DynamicImage {
    let orig_width = img.width();
    let orig_height = img.height();
    
    // Calculate aspect ratios
    let orig_ratio = orig_width as f32 / orig_height as f32;
    let target_ratio = target_width as f32 / target_height as f32;
    
    if orig_ratio > target_ratio {
        // Image is wider than target - crop horizontally 
        let new_width = (orig_height as f32 * target_ratio) as u32;
        let x_offset = (orig_width - new_width) / 2;
        let cropped = img.crop_imm(x_offset, 0, new_width, orig_height);
        cropped.resize_exact(target_width, target_height, image::imageops::FilterType::Lanczos3)
    } else {
        // Image is taller than target - crop vertically
        let new_height = (orig_width as f32 / target_ratio) as u32;
        let y_offset = (orig_height - new_height) / 2;
        let cropped = img.crop_imm(0, y_offset, orig_width, new_height);
        cropped.resize_exact(target_width, target_height, image::imageops::FilterType::Lanczos3)
    }
}

fn create_mobile_stacked_frame(images: &[PathBuf], frame_number: u32, temp_dir: &Path) -> Result<PathBuf> {
    const MOBILE_WIDTH: u32 = 1080;
    const MOBILE_HEIGHT: u32 = 1920;
    const STACK_HEIGHT: u32 = MOBILE_HEIGHT / 3; // 640px per image
    
    // Create a new blank image for the mobile frame
    let mut mobile_frame = image::RgbImage::new(MOBILE_WIDTH, MOBILE_HEIGHT);
    
    for (i, image_path) in images.iter().enumerate() {
        if i >= 3 { break; } // Only use first 3 images
        
        let img = match image::open(image_path) {
            Ok(img) => img,
            Err(e) => {
                println!("Warning: Skipping corrupted image {}: {}", image_path.display(), e);
                continue;
            }
        };
        
        // Smart crop to fit 1080x640
        let cropped = smart_crop_image(&img, MOBILE_WIDTH, STACK_HEIGHT);
        let rgb_img = cropped.to_rgb8();
        
        // Copy to the correct position in the stacked frame
        let y_offset = i as u32 * STACK_HEIGHT;
        
        for (x, y, pixel) in rgb_img.enumerate_pixels() {
            if y_offset + y < MOBILE_HEIGHT {
                mobile_frame.put_pixel(x, y_offset + y, *pixel);
            }
        }
    }
    
    // Save the stacked frame
    let frame_path = temp_dir.join(format!("frame_{:06}.png", frame_number));
    let dynamic_img = image::DynamicImage::ImageRgb8(mobile_frame);
    dynamic_img.save(&frame_path)
        .context("Failed to save mobile stacked frame")?;
    
    Ok(frame_path)
}

#[derive(Clone)]
struct FrameJob {
    frame_number: usize,
    images: Vec<PathBuf>,
    mobile_format: bool,
}

fn process_frame_job(job: &FrameJob, temp_dir: &Path, width: u32, height: u32) -> Result<()> {
    if job.mobile_format {
        // Create stacked mobile frame
        create_mobile_stacked_frame(&job.images, job.frame_number as u32, temp_dir)?;
    } else {
        // Desktop format - single image per frame
        if let Some(png_path) = job.images.first() {
            // Load and resize image - skip if corrupted
            let resized = match image::open(png_path) {
                Ok(img) => img.resize_exact(width, height, image::imageops::FilterType::Lanczos3),
                Err(e) => {
                    return Err(anyhow::anyhow!("Corrupted image {}: {}", png_path.display(), e));
                }
            };
            
            // Save frame
            let frame_path = temp_dir.join(format!("frame_{:06}.png", job.frame_number));
            resized.save(&frame_path)
                .context("Failed to save frame")?;
        }
    }
    Ok(())
}

fn create_video_precise_timing(
    png_files: Vec<PathBuf>,
    jump_cut_seconds: f64,
    mp3_path: &Path,
    output_path: &Path,
    framerate: u32,
    mobile_format: bool,
    mp3_duration: f64,
) -> Result<()> {
    let (width, height) = if mobile_format {
        (1080u32, 1920u32)
    } else {
        (1280u32, 720u32)
    };
    
    println!("Creating {} video...", if mobile_format { "mobile (9:16)" } else { "desktop (16:9)" });
    
    // Calculate unique frames needed (no duplicates)
    let unique_frames_needed = (mp3_duration / jump_cut_seconds).ceil() as usize;
    let images_per_frame = if mobile_format { 3 } else { 1 };
    
    // Input framerate = 1/jump_cut_seconds (each frame shows for jump_cut_seconds)
    let input_framerate = 1.0 / jump_cut_seconds;
    
    println!("Unique frames needed: {}, Images per frame: {}", unique_frames_needed, images_per_frame);
    println!("Input framerate: {:.1} fps (each frame shows for {:.1}s)", input_framerate, jump_cut_seconds);
    println!("Using {} CPU cores for parallel processing", rayon::current_num_threads());
    
    // Create temporary directory for frames
    let temp_dir = PathBuf::from("temp_frames");
    create_dir_all(&temp_dir)?;
    
    // Initialize circular queue for image reuse and collect all frame jobs
    let mut image_queue = CircularImageQueue::new(png_files);
    let mut frame_jobs = Vec::with_capacity(unique_frames_needed);
    
    println!("Collecting frame jobs...");
    for frame_index in 0..unique_frames_needed {
        let frame_images = image_queue.next_images(images_per_frame);
        
        frame_jobs.push(FrameJob {
            frame_number: frame_index,
            images: frame_images,
            mobile_format,
        });
    }
    
    println!("Processing {} frames in parallel...", frame_jobs.len());
    
    // Process all frames in parallel
    let temp_dir_arc = Arc::new(temp_dir.clone());
    let results: Vec<Result<()>> = frame_jobs
        .par_iter()
        .enumerate()
        .map(|(i, job)| {
            if i % 100 == 0 {
                println!("Processing batch starting at frame {}/{}", i + 1, frame_jobs.len());
            }
            process_frame_job(job, &temp_dir_arc, width, height)
        })
        .collect();
    
    // Check for any errors
    let mut successful_frames = 0;
    for (i, result) in results.iter().enumerate() {
        match result {
            Ok(()) => successful_frames += 1,
            Err(e) => println!("Warning: Frame {} failed: {}", i, e),
        }
    }
    
    println!("Generated {} successful frames (was previously {}x more)", 
             successful_frames, 
             (framerate as f64 * jump_cut_seconds).round() as u32);
    
    // Create ffmpeg command with precise timing using input framerate
    println!("Encoding video with ffmpeg...");
    let output = Command::new("ffmpeg")
        .arg("-y") // Overwrite output file
        .arg("-framerate").arg(format!("{:.6}", input_framerate)) // Input framerate controls timing
        .arg("-i").arg(temp_dir.join("frame_%06d.png"))
        .arg("-i").arg(mp3_path)
        .arg("-c:v").arg("libx264")
        .arg("-c:a").arg("aac")
        .arg("-pix_fmt").arg("yuv420p")
        .arg("-shortest") // Stop when shortest input ends
        .arg("-r").arg(framerate.to_string()) // Output framerate for smooth playback
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
    
    // Find all PNG files
    println!("Finding PNG files...");
    let png_files = find_png_files(&args.png_dir)?;
    println!("Found {} PNG files", png_files.len());
    
    if png_files.is_empty() {
        return Err(anyhow::anyhow!("No PNG files found in {}", args.png_dir.display()));
    }
    
    // Create the video
    println!("Generating video...");
    create_video_precise_timing(
        png_files,
        args.jump_cut_seconds,
        &args.song_path,
        &args.output_video,
        args.framerate,
        args.mobile_format,
        mp3_duration,
    )?;
    
    println!("✓ Video created successfully: {}", args.output_video.display());
    
    // Show final stats
    let output_size = std::fs::metadata(&args.output_video)?
        .len() as f64 / (1024.0 * 1024.0);
    println!("Output file size: {:.2} MB", output_size);
    
    Ok(())
}