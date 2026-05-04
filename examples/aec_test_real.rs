//! AEC Test Tool - Validates that OS-level Acoustic Echo Cancellation is working.
//!
//! Run with: cargo run --example aec_test
//!
//! Records microphone audio with AEC enabled while you speak and play media on your speakers.
//! If AEC is working correctly, the recording should contain your voice but have the speaker
//! audio significantly reduced.

use hound::{SampleFormat, WavSpec, WavWriter};
use std::time::Duration;
use sys_voice::{AecConfig, CaptureHandle, Channels, DuckingLevel};

const SAMPLE_RATE: u32 = 48000;
const DURATION_SECS: u64 = 30;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("AEC Test Tool");
    println!("=============");
    println!();
    println!("This will record for {} seconds with AEC enabled.", DURATION_SECS);
    println!();
    println!("Before you start, open a YouTube video on your MacBook speaker.");
    println!();
    println!("Press Enter to start recording...");
    println!();

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    println!("Recording... play your YouTube video and speak into the mic!");
    println!();
    println!("Expected result:");
    println!("- Your voice should be clearly audible");
    println!("- YouTube audio should be significantly reduced or absent");
    println!();
    println!("Recording to: aec_recording.wav");
    println!();

    let config = AecConfig {
        sample_rate: SAMPLE_RATE,
        channels: Channels::Mono,
        enable_advanced_ducking: true,
        ducking_level: DuckingLevel::Min,
        voice_processing_enable_agc: Some(false),
    };

    let handle = CaptureHandle::new(config)?;
    let mut recorded_samples: Vec<f32> = Vec::new();

    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(DURATION_SECS) {
        while let Some(result) = handle.try_recv() {
            match result {
                Ok(samples) => recorded_samples.extend_from_slice(&samples),
                Err(e) => eprintln!("Audio error: {e}"),
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    // Drain remaining samples
    while let Some(result) = handle.try_recv() {
        match result {
            Ok(samples) => recorded_samples.extend_from_slice(&samples),
            Err(e) => eprintln!("Audio error: {e}"),
        }
    }

    println!("Recording complete!");
    println!();

    drop(handle);

    let samples = &recorded_samples;
    let spec = WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };

    let mut writer = WavWriter::create("aec_recording.wav", spec)?;
    for sample in samples.iter() {
        writer.write_sample(*sample)?;
    }
    writer.finalize()?;

    println!(
        "Saved: aec_recording.wav ({} samples, {:.1} seconds)",
        samples.len(),
        samples.len() as f32 / SAMPLE_RATE as f32
    );
    println!();
    println!("To verify AEC is working:");
    println!("- Play aec_recording.wav");
    println!("- You should hear your voice clearly");
    println!("- YouTube audio should be significantly reduced or absent");
    println!("- If YouTube audio is still loud, AEC may not be active on your system");

    Ok(())
}
