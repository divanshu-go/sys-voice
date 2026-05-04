# Device Enumeration & Selection API Guide

## Complete Implementation of Device Control Features

This guide covers the three device control features you requested:

### 1. DEVICE INFO & SELECTION
**Query available devices and their capabilities**

```rust
use sys_voice::available_audio_devices;

// List all audio devices on the system
let devices = available_audio_devices()?;

for device in devices {
    println!("Device ID: {}", device.id);
    println!("  Name: {}", device.name);
    println!("  Input capable: {}", device.is_input);
    println!("  Output capable: {}", device.is_output);
    println!("  Sample rate: {} Hz", device.sample_rate);
    println!("  Channels: {}", device.channel_count);
}
```

### 2. INPUT DEVICE SELECTION
**Choose a specific microphone/input device**

```rust
use sys_voice::{AecConfig, CaptureHandle, Channels, DuckingLevel, available_audio_devices};

// List and filter input devices
let devices = available_audio_devices()?;
let input_devices: Vec<_> = devices.iter()
    .filter(|d| d.is_input)
    .collect();

// Show user list and get selection
println!("Available input devices:");
for (idx, device) in input_devices.iter().enumerate() {
    println!("  [{}] {} (ID: {})", idx, device.name, device.id);
}

// User selects device at index 1
let selected_device = input_devices[1];

// Create AEC with selected input device
let config = AecConfig {
    sample_rate: 48000,
    channels: Channels::Mono,
    enable_advanced_ducking: true,
    ducking_level: DuckingLevel::Mid,
    voice_processing_enable_agc: Some(true),
    voice_processing_bypass: None,
    input_device_id: Some(selected_device.id),   // ← Select input device
    output_device_id: None,                       // Default output
};

let capture = CaptureHandle::new(config)?;
```

### 3. OUTPUT DEVICE SELECTION
**Choose a specific speaker/output device**

```rust
use sys_voice::{AecConfig, CaptureHandle, available_audio_devices};

// List and filter output devices
let devices = available_audio_devices()?;
let output_devices: Vec<_> = devices.iter()
    .filter(|d| d.is_output)
    .collect();

// Select output device
let selected_output = output_devices[0];

// Create AEC with selected output device
let config = AecConfig {
    sample_rate: 48000,
    channels: Channels::Mono,
    enable_advanced_ducking: true,
    ducking_level: DuckingLevel::Mid,
    voice_processing_enable_agc: Some(true),
    voice_processing_bypass: None,
    input_device_id: None,                         // Default input
    output_device_id: Some(selected_output.id),   // ← Select output device
};

let capture = CaptureHandle::new(config)?;

// Play audio through selected output
let samples = vec![0.0f32; 48000];  // 1 second of silence
capture.play_audio(samples, 48000)?;
```

### Combined: Input + Output Device Selection

```rust
use sys_voice::{AecConfig, CaptureHandle, available_audio_devices, Channels, DuckingLevel};

let devices = available_audio_devices()?;
let inputs: Vec<_> = devices.iter().filter(|d| d.is_input).collect();
let outputs: Vec<_> = devices.iter().filter(|d| d.is_output).collect();

// Use external microphone and built-in speakers
let config = AecConfig {
    sample_rate: 48000,
    channels: Channels::Mono,
    enable_advanced_ducking: true,
    ducking_level: DuckingLevel::Mid,
    voice_processing_enable_agc: Some(true),
    voice_processing_bypass: None,
    input_device_id: Some(inputs[1].id),    // External USB microphone
    output_device_id: Some(outputs[0].id),  // Built-in speakers
};

let capture = CaptureHandle::new(config)?;
```

## Important Limitations

### App-Level Audio Capture
On macOS (and other systems), app-level audio capture is **limited by OS security policies**:

- ✅ **Can capture**: Your app's microphone input (with user permission)
- ✅ **Can capture**: Line-in devices and external audio inputs
- ❌ **Cannot capture**: System audio from other apps (Chrome, Spotify, etc.)
- ❌ **Cannot capture**: Internal audio routing without system extensions

### Workarounds for Multi-App Audio

To capture audio from multiple apps on macOS, use **virtual audio devices**:

1. **Loopback** (by Rogue Amoeba) - Professional virtual audio mixer
2. **BlackHole** - Free, open-source virtual audio driver
3. **VB-Audio Virtual Cable** - Cross-platform audio routing

Example workflow with BlackHole:
```
Chrome/Spotify → (route to BlackHole) → BlackHole virtual device → your app
```

Then select BlackHole as the input device:
```rust
let devices = available_audio_devices()?;
let blackhole = devices.iter().find(|d| d.name.contains("BlackHole")).unwrap();

let config = AecConfig {
    input_device_id: Some(blackhole.id),
    ..Default::default()
};
```

## API Reference

### `available_audio_devices() -> Result<Vec<AudioDeviceInfo>, AecError>`
Lists all available audio devices on the host.

**Returns**: Vector of `AudioDeviceInfo` structs

**Example**:
```rust
let devices = available_audio_devices()?;
```

### `struct AudioDeviceInfo`
Contains metadata about an audio device.

**Fields**:
- `id: u32` - Unique device identifier
- `name: String` - Human-readable device name
- `is_input: bool` - Whether device supports audio input
- `is_output: bool` - Whether device supports audio output
- `sample_rate: f64` - Nominal sample rate in Hz
- `channel_count: u32` - Number of audio channels

### `struct AecConfig`
Audio configuration with device selection.

**Device-related fields**:
- `input_device_id: Option<u32>` - None uses system default input
- `output_device_id: Option<u32>` - None uses system default output

**Other fields**:
- `sample_rate: u32` - Target sample rate (typically 48000)
- `channels: Channels` - Mono or Stereo output
- `enable_advanced_ducking: bool` - Enable VoiceProcessingIO ducking
- `ducking_level: DuckingLevel` - How much to duck other audio
- `voice_processing_enable_agc: Option<bool>` - Enable automatic gain control
- `voice_processing_bypass: Option<bool>` - Bypass voice processing

## Error Handling

Device selection errors are handled gracefully:

```rust
match available_audio_devices() {
    Ok(devices) => {
        // Process devices
    }
    Err(AecError::AecNotSupported) => {
        // Platform doesn't support device enumeration
    }
    Err(AecError::BackendError(msg)) => {
        // Core Audio API error
        eprintln!("Device error: {}", msg);
    }
    Err(e) => {
        // Other error
        eprintln!("Error: {}", e);
    }
}
```

## Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| macOS    | ✅ Full support | Uses Core Audio HAL |
| iOS      | ❌ Not yet | Needs AVAudioSession implementation |
| Windows  | ❌ Not yet | Needs WASAPI device enumeration |
| Linux    | ❌ Not yet | Needs PulseAudio/ALSA implementation |

## Running Examples

### List all audio devices:
```bash
cargo run --example device_selection
```

### Use AEC with test tone and default devices:
```bash
cargo run --example aec_test
```

## Complete Working Example

See `examples/device_selection.rs` for a complete working example that:
1. Enumerates all audio devices
2. Separates input and output devices
3. Displays device information
4. Creates AEC capture with selected devices
5. Handles errors gracefully
