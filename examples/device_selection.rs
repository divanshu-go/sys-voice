//! Device Selection Example - Demonstrates listing and selecting audio devices.
//!
//! Run with: cargo run --example device_selection
//!
//! This example shows how to:
//! 1. Enumerate available audio input/output devices on macOS
//! 2. Display device information (ID, name, capabilities, sample rate)
//! 3. Select a specific device for AEC capture and playback

use sys_voice::{AecConfig, CaptureHandle, Channels, DuckingLevel, available_audio_devices};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Audio Device Selection Example");
    println!("===============================\n");

    // List available devices
    println!("Fetching audio devices...\n");
    let devices = available_audio_devices()?;

    if devices.is_empty() {
        println!("No audio devices found!");
        return Ok(());
    }

    // Display device info
    let mut input_devices = Vec::new();
    let mut output_devices = Vec::new();

    for dev in &devices {
        if dev.is_input {
            input_devices.push(dev);
        }
        if dev.is_output {
            output_devices.push(dev);
        }
    }

    println!("INPUT DEVICES:");
    println!("==============");
    for (idx, dev) in input_devices.iter().enumerate() {
        println!(
            "  [{}] {} (ID: {}, {} Hz)",
            idx, dev.name, dev.id, dev.sample_rate as i32
        );
    }
    println!();

    println!("OUTPUT DEVICES:");
    println!("===============");
    for (idx, dev) in output_devices.iter().enumerate() {
        println!(
            "  [{}] {} (ID: {}, {} Hz)",
            idx, dev.name, dev.id, dev.sample_rate as i32
        );
    }
    println!();

    // For this example, use the first input and output devices
    if let (Some(first_input), Some(first_output)) = (input_devices.first(), output_devices.first()) {
        println!("Using first input device: {} (ID: {})", first_input.name, first_input.id);
        println!(
            "Using first output device: {} (ID: {})",
            first_output.name, first_output.id
        );
        println!();

        // Create AEC config with selected devices
        let config = AecConfig {
            sample_rate: 48000,
            channels: Channels::Mono,
            enable_advanced_ducking: true,
            ducking_level: DuckingLevel::Mid,
            voice_processing_enable_agc: Some(true),
            voice_processing_bypass: None,
            input_device_id: Some(first_input.id),
            output_device_id: Some(first_output.id),
        };

        // Create capture handle with selected devices
        println!("Creating AEC capture with selected devices...");
        match CaptureHandle::new(config) {
            Ok(handle) => {
                println!("✓ AEC capture initialized successfully!");
                println!("  Native sample rate: {} Hz", handle.native_sample_rate());
            }
            Err(e) => {
                println!("✗ Failed to initialize AEC: {}", e);
                println!("  Note: Device selection requires the OS to support it.");
            }
        }
    }

    println!();
    println!("Device enumeration complete.");

    Ok(())
}
