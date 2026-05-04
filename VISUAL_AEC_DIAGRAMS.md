# AEC Architecture: Visual Diagrams

## Diagram 1: The Complete Echo Cancellation Loop

```
┌────────────────────────────────────────────────────────────────────────┐
│                                                                        │
│                         REMOTE PERSON'S COMPUTER                      │
│                                                                        │
│  Their audio: "Hello, can you hear me?"                               │
│       │                                                                │
│       └──────────────→ [Network/Internet]                            │
│                              │                                        │
│                              ↓                                        │
│                                                                        │
├────────────────────────────────────────────────────────────────────────┤
│                                                                        │
│                         YOUR COMPUTER (sys-voice)                     │
│                                                                        │
│  ┌──────────────────────────────────────────────────────────────┐    │
│  │  Network receives: [0.12, 0.14, 0.15, ...]                 │    │
│  │  Your app: capture.play_audio(samples)                     │    │
│  └──────────────────────────────────────────────────────────────┘    │
│                              ↓                                        │
│  ┌──────────────────────────────────────────────────────────────┐    │
│  │  VoiceProcessingIO AudioUnit                               │    │
│  │  ┌──────────────────────────────────────────────────────┐   │    │
│  │  │                                                      │   │    │
│  │  │  OUTPUT BUS: Sends to speaker                       │   │    │
│  │  │  Samples: [0.12, 0.14, 0.15, ...]                  │   │    │
│  │  │  ├─ Render callback provides audio                  │   │    │
│  │  │  ├─ AEC ENGINE SEES THIS (reference signal)        │   │    │
│  │  │  │                                                  │   │    │
│  │  │  INPUT BUS: Receives from microphone                │   │    │
│  │  │  Samples: [0.12, 0.05, 0.02, ...]                  │   │    │
│  │  │  ├─ Mic picks up output echo [0.12...]             │   │    │
│  │  │  ├─ Mic picks up your voice [0.05...]              │   │    │
│  │  │  ├─ Mic picks up noise [0.02...]                   │   │    │
│  │  │  ├─ AEC ENGINE SEES THIS (measurement signal)      │   │    │
│  │  │  │                                                  │   │    │
│  │  │  AEC ENGINE LOGIC:                                  │   │    │
│  │  │  "I can see both OUTPUT [0.12, 0.14, 0.15]         │   │    │
│  │  │   and INPUT [0.12, 0.05, 0.02]"                    │   │    │
│  │  │                                                      │   │    │
│  │  │  "The [0.12] in INPUT came from OUTPUT"             │   │    │
│  │  │  "That's the echo!"                                 │   │    │
│  │  │                                                      │   │    │
│  │  │  Prediction: ECHO ≈ [0.114, 0.133, 0.140, ...]     │   │    │
│  │  │  Subtraction: CLEAN = INPUT - ECHO                  │   │    │
│  │  │              = [0.006, -0.083, -0.120, ...]         │   │    │
│  │  │              (after adaptive filtering and AGC)     │   │    │
│  │  │              ≈ [0.04, 0.29, 0.10, ...]              │   │    │
│  │  │                                                      │   │    │
│  │  └──────────────────────────────────────────────────────┘   │    │
│  │              ↓                                               │    │
│  │  Processed output: [0.04, 0.29, 0.10, ...]                │    │
│  │  (Echo REMOVED! Voice PRESERVED! Noise SUPPRESSED!)       │    │
│  │              ↓                                               │    │
│  │  Input callback: Sends to app                              │    │
│  │              ↓                                               │    │
│  │  Your app receives: clean_voice = [0.04, 0.29, 0.10, ...]│    │
│  │  Your app sends to network                                │    │
│  └──────────────────────────────────────────────────────────────┘    │
│                              ↓                                        │
│  Real world (acoustic coupling):                                      │
│  ┌──────────────────────────────────────────────────────────────┐    │
│  │  Speaker outputs sound waves                               │    │
│  │  [0.12, 0.14, 0.15, ...] → air                            │    │
│  │                                                             │    │
│  │  Sound bounces around:                                     │    │
│  │  - Off walls                                               │    │
│  │  - Off furniture                                           │    │
│  │  - Off your face                                           │    │
│  │                                                             │    │
│  │  Your voice travels in:                                    │    │
│  │  [your_voice] → air                                        │    │
│  │                                                             │    │
│  │  Microphone receives BOTH:                                 │    │
│  │  [echo] + [your_voice] + [noise]                          │    │
│  └──────────────────────────────────────────────────────────────┘    │
│                                                                        │
│  Clean voice: [0.04, 0.29, 0.10, ...]                               │
│       │                                                                │
│       └──────────────→ [Network/Internet]                            │
│                                                                        │
└────────────────────────────────────────────────────────────────────────┘
       │
       ↓
┌────────────────────────────────────────────────────────────────────────┐
│                                                                        │
│                    REMOTE PERSON'S COMPUTER                          │
│                                                                        │
│  Their speaker receives: [0.04, 0.29, 0.10, ...]                    │
│  What they hear: YOUR VOICE (no echo! no feedback!)                 │
│                                                                        │
│  Remote person thinks: "Great conversation, no echo!"                 │
│                                                                        │
└────────────────────────────────────────────────────────────────────────┘
```

## Diagram 2: With vs Without AEC

### ❌ WITHOUT AEC (Separate Components)

```
Your App
├─ Playback: Send to SPEAKER COMPONENT
│  └─ [0.12, 0.14, 0.15, ...] → Speaker
│
└─ Recording: Receive from MIC COMPONENT
   └─ Mic input: [0.12, 0.05, 0.02, ...]
      ├─ [0.12] is echo (but app doesn't know)
      ├─ [0.05] is your voice
      └─ [0.02] is noise

Result: Send [0.12, 0.05, 0.02, ...] to remote person
        Remote person hears: "Hello... Hello..." (echo) + "Yeah, I'm listening" (voice)
        🔴 TERRIBLE
```

### ✅ WITH AEC (Same Component)

```
Your App
│
└─ SINGLE VoiceProcessingIO COMPONENT
   ├─ OUTPUT BUS: [0.12, 0.14, 0.15, ...]
   │  └─ Goes to speaker
   │  └─ AEC ENGINE SEES THIS
   │
   └─ INPUT BUS: [0.12, 0.05, 0.02, ...]
      ├─ Comes from microphone
      ├─ AEC ENGINE SEES THIS
      │
      └─ AEC ENGINE ALGORITHM
         ├─ Recognizes [0.12] from OUTPUT
         ├─ Calculates echo prediction
         ├─ Subtracts from input
         └─ Result: [0.04, 0.29, 0.10, ...]

Result: Send [0.04, 0.29, 0.10, ...] to remote person
        Remote person hears: Only "Yeah, I'm listening" (clean voice)
        No echo!
        🟢 PERFECT
```

## Diagram 3: System Block Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                    sys-voice Implementation                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Application Layer (Your Code)                                 │
│  ┌──────────────────────────────────────┐                      │
│  │ let config = AecConfig {             │                      │
│  │   input_device_id: Some(mic_id),     │                      │
│  │   output_device_id: Some(speaker_id),│                      │
│  │   ...                                │                      │
│  │ };                                   │                      │
│  │                                      │                      │
│  │ let capture = CaptureHandle::new()?  │                      │
│  │ capture.play_audio(samples)?         │                      │
│  │ capture.recv_async().await?          │                      │
│  └──────────────────────────────────────┘                      │
│           │                      │                              │
│           ↓                      ↓                              │
│  ┌─────────────────────────────────────────────┐               │
│  │  Backend (src/backends/mod.rs)              │               │
│  │  Platform dispatcher                        │               │
│  └─────────────────────────────────────────────┘               │
│           │                                                     │
│           ↓                                                     │
│  ┌─────────────────────────────────────────────┐               │
│  │  macOS Backend (src/backends/macos.rs)      │               │
│  │                                              │               │
│  │  create_backend() {                         │               │
│  │    1. Find VoiceProcessingIO component      │               │
│  │    2. Create AudioUnit instance             │               │
│  │    3. Set device IDs (if provided)          │               │
│  │    4. Enable INPUT_BUS (microphone)         │               │
│  │    5. Enable OUTPUT_BUS (speaker)           │               │
│  │    6. Set input_callback()                  │               │
│  │    7. Set render_callback()                 │               │
│  │    8. Start audio processing                │               │
│  │  }                                           │               │
│  └─────────────────────────────────────────────┘               │
│           │                                                     │
│           ↓                                                     │
│  ┌─────────────────────────────────────────────┐               │
│  │  VoiceProcessingIO AudioUnit (OS-level)     │               │
│  │  ┌───────────────────────────────────────┐  │               │
│  │  │  INPUT_BUS (1)                        │  │               │
│  │  │  From: Microphone (system device)     │  │               │
│  │  │  To: AEC Engine input                 │  │               │
│  │  └───────────────────────────────────────┘  │               │
│  │                    │                         │               │
│  │  ┌─────────────────────────────────────┐    │               │
│  │  │  [AEC ENGINE] ← Core Apple AEC      │    │               │
│  │  │  [AGC ENGINE] ← Automatic Gain Ctrl │    │               │
│  │  │  [NOISE SUPPRESSION]                │    │               │
│  │  └─────────────────────────────────────┘    │               │
│  │                    │                         │               │
│  │  ┌───────────────────────────────────────┐  │               │
│  │  │  OUTPUT_BUS (0)                       │  │               │
│  │  │  From: AEC Engine output              │  │               │
│  │  │  To: Speaker (system device)          │  │               │
│  │  └───────────────────────────────────────┘  │               │
│  │                                              │               │
│  │  ⭐ CRITICAL FEATURE:                      │               │
│  │  Both buses see SAME AEC engine             │               │
│  │  So echo cancellation WORKS                │               │
│  └─────────────────────────────────────────────┘               │
│           │                                                     │
│           ↓                                                     │
│  ┌─────────────────────────────────────────────┐               │
│  │  Hardware                                    │               │
│  │  ├─ Microphone (system device)              │               │
│  │  └─ Speaker (system device)                 │               │
│  └─────────────────────────────────────────────┘               │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Diagram 4: Callback Sequence

```
Timeline: One buffer (1024 samples @ 48kHz = ~21ms)

T=0ms
   ├─ Network: Remote person's audio arrives
   ├─ App: capture.play_audio(remote_audio)
   └─ playback_buffer.push(samples)
       │
       └─ Queue size: 1024 samples

T=5ms (Render Callback)
   ├─ VoiceProcessingIO: "I need output samples!"
   ├─ render_callback() called
   ├─ Pop 1024 samples from playback_buffer
   ├─ Return to AudioUnit: "Here's output data"
   │  └─ AEC ENGINE SEES: [0.12, 0.14, 0.15, ...]
   ├─ Speaker: Plays sound waves
   │
   └─ Acoustic coupling in room:
       ├─ Sound bounces
       ├─ Echo forms
       └─ Microphone picks up: [0.12, 0.05, 0.02, ...]
           where [0.12] = echo
                 [0.05] = your voice
                 [0.02] = noise

T=21ms (Input Callback)
   ├─ VoiceProcessingIO: "Here's processed audio!"
   ├─ [Before callback] AEC processes:
   │  ├─ Sees INPUT: [0.12, 0.05, 0.02, ...]
   │  ├─ Remembers OUTPUT: [0.12, 0.14, 0.15, ...]
   │  ├─ Calculates: echo_prediction ≈ [0.11, 0.13, 0.14, ...]
   │  ├─ Subtracts: clean = input - echo ≈ [0.01, -0.08, ...]
   │  ├─ AGC: normalizes to [0.04, 0.29, 0.10, ...]
   │  └─ Noise suppression applied
   │
   ├─ input_callback() called with processed data
   ├─ AudioUnitRender() retrieves [0.04, 0.29, 0.10, ...]
   ├─ capture_tx.send(clean_voice)
   │
   └─ App receives processed audio
       └─ Send to remote person via network

T=22-40ms
   ├─ Remote person receives [0.04, 0.29, 0.10, ...]
   ├─ Remote person hears: "Yeah, I'm listening" (CLEAN)
   ├─ No echo! No feedback!
   └─ Perfect conversation!

T=41ms (Loop repeats)
   └─ New batch of remote person's audio arrives...
```

## Diagram 5: Why Same Component is Essential

```
❌ WRONG: Separate Components

Audio App
├─ Playback Component
│  ├─ OUTPUT BUS: Send [0.12, 0.14, 0.15] to speaker
│  └─ Has its own AEC engine (not connected to input)
│
├─ Recording Component  
│  ├─ INPUT BUS: Receive [0.12, 0.05, 0.02] from mic
│  └─ Has its own AEC engine (doesn't see output)
│
Result: Two components can't talk to each other
        No way to correlate signals
        AEC impossible!


✅ RIGHT: Same Component (VoiceProcessingIO)

Audio App
└─ VoiceProcessingIO AudioUnit
   ├─ OUTPUT BUS: Send [0.12, 0.14, 0.15] to speaker
   ├─ INPUT BUS: Receive [0.12, 0.05, 0.02] from mic
   ├─ ONE AEC ENGINE: Sees both buses
   │  └─ "I can compare output vs input!"
   │  └─ "I can identify and subtract echo!"
   │  └─ "I can deliver clean voice!"
   │
   Result: Both signals flow through same component
           AEC engine has access to both
           Echo cancellation WORKS!
```

## Diagram 6: Device Selection Integration

```
┌────────────────────────────────────────────────────────────────┐
│  User: available_audio_devices()                              │
│  Returns: [                                                    │
│    { id: 123, name: "Built-in Mic", is_input: true, ... },   │
│    { id: 124, name: "USB Mic", is_input: true, ... },         │
│    { id: 125, name: "Built-in Speaker", is_output: true, ...},│
│    { id: 126, name: "Headphones", is_output: true, ... }      │
│  ]                                                             │
│                                                                │
│  User selects: Built-in Mic (123) + Headphones (126)         │
│                                                                │
│  User creates: AecConfig {                                    │
│    input_device_id: Some(123),   ← Select this input device  │
│    output_device_id: Some(126),  ← Select this output device │
│    ...                                                         │
│  }                                                             │
│                                                                │
│  Backend applies:                                              │
│  set_property(audio_unit,                                      │
│    kAudioOutputUnitProperty_CurrentDevice,                    │
│    input_device_id)  ← Sets to device 123 (USB Mic)           │
│                                                                │
│  set_property(audio_unit,                                      │
│    kAudioOutputUnitProperty_CurrentDevice,                    │
│    output_device_id) ← Sets to device 126 (Headphones)        │
│                                                                │
│  VoiceProcessingIO uses selected devices:                      │
│  ├─ INPUT_BUS: Records from device 123 (USB Mic)             │
│  └─ OUTPUT_BUS: Plays to device 126 (Headphones)             │
│                                                                │
│  AEC between USB Mic and Headphones:                          │
│  └─ Perfect isolation (headphones don't couple to USB mic)    │
│     But if you have speakerphone, coupling is handled!        │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

## Summary

The key to AEC working is shown in all these diagrams:

**Same AudioUnit (VoiceProcessingIO)**
↓
**Both input and output buses**
↓  
**Shared AEC engine**
↓
**Can see both signals simultaneously**
↓
**Can correlate and subtract echo**
↓
**Clean voice output!**

This is why your sys-voice implementation works!
