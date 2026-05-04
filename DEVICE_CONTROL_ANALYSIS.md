# Audio Device Control & Recording Scope Analysis

## Current Architecture Overview

Your macOS AEC backend uses **Apple's VoiceProcessingIO AudioUnit** (native OS-level component).

---

## 1. DEVICE INFO & SELECTION

### Current State: ❌ NOT IMPLEMENTED
- Backend uses **system default input/output devices only**
- No device enumeration
- No device selection capability

### How to Implement Device Listing

```rust
// Required imports (not yet in project)
use objc2_core_audio::{
    AudioHardwareGetProperty, AudioHardwareSetProperty,
    kAudioHardwarePropertyDevices,
    kAudioObjectPropertyElementMaster,
    kAudioObjectSystemObject,
};

pub struct AudioDeviceInfo {
    pub id: u32,
    pub name: String,
    pub is_input: bool,
    pub is_output: bool,
    pub sample_rate: f64,
    pub channel_count: u32,
}

pub fn list_audio_devices() -> Result<Vec<AudioDeviceInfo>, AecError> {
    // Query CoreAudio HAL (Hardware Abstraction Layer) for all devices
    // For each device: get ID, name, capabilities, formats
}
```

**Available APIs** (in `objc2-core-audio` v0.3.2):
- `kAudioHardwarePropertyDevices` - list all devices
- `kAudioDevicePropertyDeviceNameCFString` - device display name
- `kAudioDevicePropertyStreams` - get input/output streams
- `kAudioStreamPropertyVirtualFormat` - supported sample rates/channels

**Effort**: Medium (~2-3 hours)

---

## 2. INPUT DEVICE SELECTION

### Current State: ❌ NOT SUPPORTED
- VoiceProcessingIO always uses default mic

### How to Enable Device Selection

```rust
pub struct AecConfig {
    // ... existing fields ...
    
    /// Optional input device ID. None = use system default
    pub input_device_id: Option<u32>,
    /// Optional output device ID. None = use system default  
    pub output_device_id: Option<u32>,
}

pub fn create_backend_with_devices(
    config: AecConfig,
) -> Result<..., AecError> {
    if let Some(device_id) = config.input_device_id {
        // Set input device via AudioUnitSetProperty
        // kAudioOutputUnitProperty_CurrentDevice
    }
    if let Some(device_id) = config.output_device_id {
        // Set output device via AudioUnitSetProperty
        // kAudioOutputUnitProperty_CurrentDevice  
    }
}
```

**Key Property**: `kAudioOutputUnitProperty_CurrentDevice` (already in generated bindings)

**Effort**: Low (~1 hour)

---

## 3. OUTPUT DEVICE SELECTION

### Current State: ❌ NOT SUPPORTED
- VoiceProcessingIO always uses default speaker

### How to Enable
Same as input device - use `kAudioOutputUnitProperty_CurrentDevice` on the AudioUnit

**Effort**: Low (~1 hour, same work as input selection)

---

## 4. APP-LEVEL RECORDING CONTROL (ADVANCED)

### Current State: ❌ NOT POSSIBLE with VoiceProcessingIO

**Why:** VoiceProcessingIO operates at the **OS audio mix level**, not individual app level:
- It captures from the **aggregated system input** (all sources mixed by OS)
- It doesn't have API to filter specific applications
- No process-level audio routing

### Possible Workarounds (Complex):

#### Option A: Use AVCaptureSession (iOS-like, not typical on macOS)
```swift
// iOS has application audio capture, macOS does NOT have this natively
// Would require using Swift Objective-C bridge, not pure Rust
```

#### Option B: Multi-device Setup
```
If your boss wants to record specific app audio:
1. User routes that app's output to a "virtual audio device" 
   (e.g., BlackHole, Soundflower - third-party only)
2. Your app records from that virtual device
3. This is MANUAL routing by the user, not automated
```

#### Option C: System Audio Routing (Full OS Access Required)
```
Requires:
- User grants Accessibility + Audio permissions
- Use AVFoundation's audio routing APIs (macOS 13+)
- Intercept audio at HAL level (experimental, unsupported)
```

**Bottom Line**: App-level filtering requires third-party virtual audio setup or special OS permissions. Not feasible in current architecture.

---

## 5. MONITOR/WINDOW-SPECIFIC RECORDING

### Current State: ❌ NOT APPLICABLE
- Audio recording is **device-based**, not display-based
- Capturing screen audio ≠ selecting which monitor

### What's Technically Possible:

**Screen/Window Capture** (for video, not audio):
```rust
// ScreenCaptureKit (macOS 13.2+) - VIDEO ONLY
// Does not provide audio capture
// Audio must come from separate audio device/app
```

**Audio from specific apps**:
- Not possible natively on macOS
- Would require app routing to virtual audio device first

---

## 6. PERMISSION & RUNTIME REQUIREMENTS

### Current:
- ✅ Microphone permission (granted by system)
- ✅ Works in background
- ❌ No special permissions needed

### For Full Device Control:
- ⚠️ May need to add Audio Input/Output permissions to `Info.plist`
- ⚠️ May need Accessibility permission if routing apps to virtual devices

---

## IMPLEMENTATION ROADMAP

### Phase 1: Device Enumeration (Easy - 2-3 hours)
```rust
CaptureHandle::available_input_devices() -> Vec<AudioDeviceInfo>
CaptureHandle::available_output_devices() -> Vec<AudioDeviceInfo>
CaptureHandle::default_input_device() -> AudioDeviceInfo
CaptureHandle::default_output_device() -> AudioDeviceInfo
```

### Phase 2: Device Selection (Easy - 1 hour)
```rust
AecConfig {
    input_device_id: Some(device_id),
    output_device_id: Some(device_id),
    // ... other fields
}
```

### Phase 3: Virtual Audio Routing (Medium - requires user setup)
- Document how users set up virtual audio devices
- Add validation to detect virtual devices
- Provide example routing guide

### Phase 4: App Audio Capture (Hard - not recommended)
- **Not feasible** in current architecture
- Would require complete redesign around virtual audio + AVFoundation
- macOS doesn't have native app-level audio filtering like iOS does

---

## SUMMARY: What We Can & Cannot Do

| Feature | Status | Effort | Notes |
|---------|--------|--------|-------|
| **List audio devices** | ❌ TODO | 2-3 hrs | Use CoreAudio HAL |
| **Select input device** | ❌ TODO | 1 hr | Set property on AudioUnit |
| **Select output device** | ❌ TODO | 1 hr | Set property on AudioUnit |
| **Get device info (name, SR, channels)** | ❌ TODO | 2-3 hrs | Query CoreAudio properties |
| **Record specific app audio** | ❌ NOT POSSIBLE | N/A | macOS limitation; need 3rd-party virtual audio |
| **Record from monitor/screen** | ❌ NOT APPLICABLE | N/A | Audio ≠ Video; separate concern |
| **Process-level filtering** | ❌ NOT POSSIBLE | N/A | Requires OS-level audio routing (unsupported) |

---

## RECOMMENDATION TO BOSS

✅ **Implement Phase 1-2** (device enumeration + selection)
- Straightforward using existing CoreAudio bindings
- Adds real value to end users
- ~3-4 hours total work

❌ **Do NOT pursue Phase 3-4** (app/monitor control)
- macOS doesn't have app-level audio capture APIs like iOS
- Would require manual user setup with 3rd-party virtual audio tools
- Not a core AEC feature; out of scope

**For app-level recording on macOS, recommend users:**
1. Route target app to virtual audio device (BlackHole, Soundflower)
2. Your app records from that virtual device
3. Provide documentation/guide for this workflow

