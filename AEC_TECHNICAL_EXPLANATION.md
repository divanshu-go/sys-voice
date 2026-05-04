# How AEC Really Works: Input/Output Signal Coupling on macOS

## The Core Problem: Echo

When you play audio through a speaker and record with a microphone in the same room, the sound from the speaker naturally travels through the air and enters the microphone. This is **echo**.

```
Speaker Output (e.g., person talking on Zoom call)
         ↓
    [Air coupling]
         ↓
Microphone Input (you hear the person + their own voice echoing back)
```

For video conferencing, this is terrible because:
- Remote person hears themselves echoing back
- Creates howling/feedback loops
- Makes conversation impossible

## How AEC Solves It

AEC works by **knowing what was played** and **subtracting it from what was recorded**.

```
OUTPUT Signal ──→ [AEC Reference Engine] ──→ Predicts what echo will appear in input
                                                         ↓
INPUT Signal ────────→ [Subtraction] ←─────────────── Prediction
                            ↓
                    Clean voice (no echo)
```

### The Algorithm (Simplified)

1. **Record the output** that was sent to the speaker ("reference signal")
2. **Measure time delay** between output and the echo appearing in input
3. **Model the acoustic path** (how sound travels from speaker to mic)
4. **Generate echo prediction** = reference signal filtered through the acoustic path
5. **Subtract prediction** from input: `clean_voice = input - echo_prediction`
6. **Continuously adapt** as the acoustic environment changes

## macOS VoiceProcessingIO: The Hardware-Software Bridge

On macOS, the `VoiceProcessingIO` AudioUnit is specially designed for this exact problem.

### Architecture

```
┌────────────────────────────────────────────────────────┐
│              VoiceProcessingIO AudioUnit              │
├────────────────────────────────────────────────────────┤
│                                                        │
│  INPUT BUS (Bus 1)                OUTPUT BUS (Bus 0)  │
│  ├─ Microphone input            ├─ Speaker output    │
│  ├─ Raw samples from mic        ├─ Audio to play     │
│  │                              │                    │
│  └─→ [AEC Engine] ←─────────────┴─ Uses output as    │
│         ↓                           reference signal  │
│    [AGC Engine]                                       │
│         ↓                                              │
│    [Noise Suppression]                               │
│         ↓                                              │
│    Processed output (clean voice)                     │
│                                                        │
└────────────────────────────────────────────────────────┘
```

### The Critical Requirement: Same AudioUnit

**Both input and output MUST go through the same VoiceProcessingIO instance** for AEC to work.

This is why:
- The AEC engine inside VoiceProcessingIO needs **direct access** to both signals
- Only then can it model the acoustic path
- Only then can it generate accurate echo predictions
- Only then can it subtract properly

### What Happens If You Use Different Devices

```
❌ WRONG (No AEC):
Speaker from App₁ ──→ System Output
                            ↓
                      [Air coupling]
                            ↓
                      Microphone ──→ Input to App₂

App₂ never sees the output signal, so can't perform AEC!


✅ RIGHT (AEC Works):
App Audio Output ─────┐
                      └──→ [VoiceProcessingIO] ←── Microphone Input
                            └──→ Clean audio output
```

## Which Devices Benefit from AEC?

### ✅ Devices WHERE AEC is Essential

1. **Speakerphones** (built-in speaker + mic in same device)
   - Laptop/desktop with built-in speaker + mic
   - Phone speakerphone mode
   - Smartspeaker with voice commands
   - Problem: Speaker output naturally couples into the mic
   - Solution: AEC subtracts speaker output from mic input

2. **Open Microphones in Video Conferencing**
   - You're on a Zoom call with speaker enabled
   - Remote person's voice plays from your speaker
   - Your mic picks up that sound
   - Without AEC: Remote person hears themselves echoing
   - With AEC: Remote person hears only your voice

3. **Podcasting/Live Streaming with Monitor Mix**
   - You want to hear yourself while recording
   - Audio plays to headphones/monitor output
   - Mic picks up ambient sound (not the monitor)
   - Less critical if using headphones, but still useful

### ❌ Devices WHERE AEC is NOT Needed

1. **Headset Use** (headphones + separate microphone)
   - Speaker output goes to headphones (isolated)
   - Microphone is separate from speaker
   - Almost no acoustic coupling
   - AEC would do nothing, but doesn't hurt

2. **Perfect Isolation Scenarios**
   - Recording studio with speaker in different room
   - Professional microphone on shock mount away from speakers
   - AEC unnecessary because no coupling

### 🟡 Devices WHERE AEC Helps But Not Critical

1. **Directional Microphones**
   - Designed to reject sound from speaker direction
   - Already reduce echo naturally
   - AEC provides additional cleanup

2. **Poor Room Acoustics**
   - Open office with multiple people
   - Ambient noise already present
   - AEC helps, but limited by noise floor

## Real MacOS Implementation

Here's how the signal flow works in sys-voice:

```
┌─────────────────────────────────────────────────────────┐
│  User's App (e.g., Zoom Client)                         │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  1. Start recording:                                   │
│     capture_handle = CaptureHandle::new(config)        │
│                             ↓                           │
│  2. Internal setup:                                     │
│     - Create VoiceProcessingIO AudioUnit                │
│     - Enable Input Bus (connects to microphone)         │
│     - Enable Output Bus (connects to speaker)           │
│     - AEC engine inside automatically connected         │
│                                                         │
│  3. Loop:                                               │
│     while let Some(clean_voice) = capture_handle       │
│         .recv_async().await                            │
│     {                                                   │
│         process_voice(clean_voice)                      │
│     }                                                   │
│                                                         │
│  4. Play audio (send to speaker):                      │
│     capture_handle.play_audio(remote_person_audio, sr) │
│              ↓                                           │
│     Remote person's voice → Speaker                     │
│     At SAME TIME: Speaker couples into mic             │
│     But AEC SEES the output signal                      │
│     So it SUBTRACTS the coupling                        │
│     Result: clean_voice has NO echo                     │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

## Signal Flow Diagram: The Complete Picture

```
EXTERNAL WORLD:
┌─────────────────────────────────────────────┐
│   Remote Person Talking                     │
│   (transmitted over network)                │
└────────────┬────────────────────────────────┘
             │
             ↓
        ┌──────────────────────────────────────────────────────┐
        │  Your Application (Zoom, Teams, etc.)              │
        │                                                     │
        │  Input Path: "What I'm saying"                    │
        │  ─────────────────────────────────────────→        │
        │      Microphone                                    │
        │          ↓                                          │
        │  [Raw mic input: my voice + remote person echo]   │
        │          ↓                                          │
        │      VoiceProcessingIO (Input Bus)                │
        │          ↓                                          │
        │      [AEC Engine]                                 │
        │      (also sees Output Bus)                       │
        │          ↓                                          │
        │  [Processed output: ONLY my voice]                │
        │          ↑                                          │
        │          │                                          │
        │  Output Path: "What I'm playing"                  │
        │  ←─────────────────────────────────────────        │
        │      Remote Person's voice (from network)         │
        │          ↓                                          │
        │      VoiceProcessingIO (Output Bus)               │
        │          ↓                                          │
        │      Speaker                                       │
        │          ↓                                          │
        │  [SOUND in room]                                  │
        │          │                                          │
        │          │ ACOUSTIC COUPLING                       │
        │          │ (sound travels through air)             │
        │          ↓                                          │
        │      Microphone                                    │
        │      (picks up echo)                               │
        │          ↓                                          │
        │  [AEC SUBTRACTS THIS]                            │
        │          ↓                                          │
        │  [Result: clean voice]                            │
        │                                                     │
        │  Send clean voice back to remote person            │
        │  (Remote person does NOT hear their own echo!)    │
        │                                                     │
        └──────────────────────────────────────────────────────┘
             ↓
        ┌──────────────────────────────────────────────────────┐
        │  Network                                            │
        │  (Send clean audio back to remote person)          │
        └──────────────────────────────────────────────────────┘
```

## Code Example: How It Works

```rust
use sys_voice::{AecConfig, CaptureHandle, Channels, DuckingLevel};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create AEC with INPUT and OUTPUT through same VoiceProcessingIO
    let config = AecConfig {
        sample_rate: 48000,
        channels: Channels::Mono,
        enable_advanced_ducking: true,  // Reduce other app audio
        ducking_level: DuckingLevel::Mid,
        voice_processing_enable_agc: Some(true),
        voice_processing_bypass: None,
        input_device_id: None,  // Use default mic
        output_device_id: None, // Use default speaker
    };

    let capture = CaptureHandle::new(config)?;
    let stream = capture.start_playback_stream(48000)?;

    // Simulate Zoom-like app
    tokio::spawn(async move {
        // Simulated remote person's audio arriving from network
        let remote_audio = vec![0.1f32; 48000]; // 1 second of audio
        
        // Play through speaker (this couples into microphone naturally)
        let _ = stream.send(remote_audio);
    });

    // Record audio
    // The INPUT here is: [my voice + remote person echo from speaker]
    // But VoiceProcessingIO's AEC SEES both signals
    // So it subtracts the echo perfectly
    // Result: We capture [clean my voice]
    loop {
        if let Some(Ok(samples)) = capture.recv_async().await {
            println!("Captured {} samples (no echo!)", samples.len());
            // Send these clean samples to remote person
            // They will NOT hear their own voice coming back
        }
    }
}
```

## Why This Matters for Your System

Your sys-voice implementation correctly handles this by:

1. **Using VoiceProcessingIO** for both input and output
   - Not separate audio units
   - Single unified audio engine

2. **Setting up both buses**
   ```rust
   // Input Bus (Microphone)
   AudioUnitSetProperty(unit, kAudioOutputUnitProperty_EnableIO, 
                        kAudioUnitScope_Input, INPUT_BUS, ...)
   
   // Output Bus (Speaker)
   AudioUnitSetProperty(unit, kAudioOutputUnitProperty_EnableIO,
                        kAudioUnitScope_Output, OUTPUT_BUS, ...)
   ```

3. **Connecting callbacks to both**
   - Input callback: receives processed audio from AEC
   - Render callback: provides audio to speaker (AEC sees this)
   - AEC engine automatically correlates them

4. **Optional device selection**
   - Users can choose which speaker/mic to use
   - But they're still coupled through same VoiceProcessingIO

## Summary: When AEC is Actually Useful

| Scenario | AEC Needed? | Why |
|----------|-----------|-----|
| Speakerphone (speaker + mic in same device) | ✅ YES | Output couples directly into input |
| Video call with speaker enabled | ✅ YES | Remote person hears their own echo |
| Headset with isolated speaker/mic | ❌ NO | No coupling between speaker and mic |
| Recording studio (speaker in different room) | ❌ NO | Acoustic isolation prevents coupling |
| Noise-canceling headphones with voice commands | ✅ YES | Microphone picks up monitor output leakage |
| Open office speakerphone | ✅ YES (+ Noise Suppression) | Multiple echo paths + ambient noise |

## The Bottom Line for Your Boss

Tell your boss:

> **"AEC is useful when the OUTPUT (what we're playing) can be heard by the INPUT (what we're recording). The magic is that by connecting both input and output to the same audio engine (VoiceProcessingIO on macOS), the system can measure the coupling and mathematically subtract it. This is why speakerphone conversations work without creating feedback loops — AEC listens to what was played and removes it from what was recorded.**

> **On macOS, we use VoiceProcessingIO which has AEC built-in. When you play audio through the output bus and record through the input bus of the same AudioUnit, the AEC engine automatically has access to both signals and can perform perfect echo cancellation. That's why we need both signals flowing through the same component — they're not independent; they're designed to work together."**

This is the core insight that makes professional voice applications work!
