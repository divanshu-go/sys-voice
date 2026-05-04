# Sys-Voice: How Input/Output Coupling Works (Code Walkthrough)

This document shows the **exact code** where your sys-voice implementation couples input and output signals for AEC to work.

## The Setup Phase

### Step 1: Find and Create VoiceProcessingIO

```rust
// File: src/backends/macos.rs, line ~210
let audio_unit_description = AudioComponentDescription {
    componentType: kAudioUnitType_Output,
    componentSubType: kAudioUnitSubType_VoiceProcessingIO,  // ← Special component with AEC built-in
    componentManufacturer: kAudioUnitManufacturer_Apple,
    componentFlags: 0,
    componentFlagsMask: 0,
};

let component = unsafe { AudioComponentFindNext(...) };
let mut audio_unit = ptr::null_mut();
unsafe { AudioComponentInstanceNew(component, NonNull::from(&mut audio_unit)) };
```

**What's happening**:
- `VoiceProcessingIO` is a special macOS AudioUnit with built-in AEC engine
- Unlike other audio units, VoiceProcessingIO has TWO buses: Input and Output
- Creating it is the first step to enable input/output coupling

### Step 2: Enable BOTH Input and Output

```rust
// File: src/backends/macos.rs, line ~250
const INPUT_BUS: u32 = 1;
const OUTPUT_BUS: u32 = 0;

let enable_io: u32 = 1;
unsafe {
    set_property(
        audio_unit,
        kAudioOutputUnitProperty_EnableIO,
        kAudioUnitScope_Input,    // ← Enable microphone input
        INPUT_BUS,
        &enable_io,
        "failed to enable input IO",
    )?;
    
    set_property(
        audio_unit,
        kAudioOutputUnitProperty_EnableIO,
        kAudioUnitScope_Output,   // ← Enable speaker output
        OUTPUT_BUS,
        &enable_io,
        "failed to enable output IO",
    )?;
}
```

**The Critical Part**:
```
SAME AUDIOUNIT              SAME AUDIOUNIT
    ↓                            ↓
[VoiceProcessingIO]
├─ INPUT_BUS (1) ──→ Microphone input
└─ OUTPUT_BUS (0) ──→ Speaker output
    ↑
    AEC Engine sees BOTH!
```

This is why AEC works: Both input and output flow through the **same component**, so the AEC engine has access to both signals.

### Step 3: Configure Stream Format

```rust
// File: src/backends/macos.rs, line ~300
let stream_format = unsafe { build_stream_format(native_format.mSampleRate) };
unsafe {
    set_property(
        audio_unit,
        kAudioUnitProperty_StreamFormat,
        kAudioUnitScope_Output,  // Input data comes out of the Output scope
        INPUT_BUS,               // But on the Input bus
        &stream_format,
        "failed to set input stream format",
    )?;
    set_property(
        audio_unit,
        kAudioUnitProperty_StreamFormat,
        kAudioUnitScope_Input,   // Output data goes into the Input scope
        OUTPUT_BUS,              // On the Output bus
        &stream_format,
        "failed to set output stream format",
    )?;
}
```

**Note**: AudioUnit scope/bus naming is confusing:
- `kAudioUnitScope_Output, INPUT_BUS` = data OUTPUT from the Input bus (microphone processed data)
- `kAudioUnitScope_Input, OUTPUT_BUS` = data INPUT to the Output bus (speaker data)

## The Callback Phase: Where the Magic Happens

### Step 4: Set Input Callback (Microphone → AEC → App)

```rust
// File: src/backends/macos.rs, line ~315
let input_callback = AURenderCallbackStruct {
    inputProc: Some(input_callback),  // Function called when samples ready
    inputProcRefCon: input_state_refcon,
};
unsafe {
    set_property(
        audio_unit,
        kAudioOutputUnitProperty_SetInputCallback,
        kAudioUnitScope_Global,
        OUTPUT_BUS,
        &input_callback,
        "failed to set input callback",
    )?;
}

// The actual input callback function (line ~120)
unsafe extern "C-unwind" fn input_callback(
    in_ref_con: NonNull<c_void>,
    io_action_flags: NonNull<AudioUnitRenderActionFlags>,
    in_time_stamp: NonNull<objc2_core_audio_types::AudioTimeStamp>,
    in_bus_number: u32,
    in_number_frames: u32,
    _io_data: *mut AudioBufferList,
) -> i32 {
    let state = &*(in_ref_con.as_ptr() as *const InputCallbackState);
    
    // Allocate buffer for processed samples
    let sample_count = in_number_frames as usize;
    let mut samples = vec![0.0f32; sample_count];
    
    // Call AudioUnitRender on INPUT_BUS
    // This gets data that has been PROCESSED by AEC
    // i.e., input signal with the output signal SUBTRACTED OUT
    let audio_buffer = AudioBuffer {
        mNumberChannels: 1,
        mDataByteSize: (sample_count * mem::size_of::<f32>()) as u32,
        mData: samples.as_mut_ptr() as *mut c_void,
    };
    let mut audio_buffer_list = AudioBufferList {
        mNumberBuffers: 1,
        mBuffers: [audio_buffer],
    };

    // HERE'S THE KEY: AudioUnitRender on INPUT_BUS
    // VoiceProcessingIO returns processed audio (echo removed)
    let status = AudioUnitRender(
        state.audio_unit.0,
        io_action_flags.as_ptr(),
        in_time_stamp,
        in_bus_number,
        in_number_frames,
        NonNull::from(&mut audio_buffer_list),
    );
    
    if status != 0 {
        return status;
    }

    // Send processed samples to app
    let _ = state.capture_tx.try_send(samples);
    0
}
```

**What's happening**:
1. VoiceProcessingIO calls this callback when it has processed data
2. We call `AudioUnitRender` on the INPUT_BUS
3. The data we get back is **already processed by AEC**
4. Meaning: output signal has been subtracted from input signal
5. We send this clean audio to the application

### Step 5: Set Render Callback (App → Speaker)

```rust
// File: src/backends/macos.rs, line ~323
let render_callback_struct = AURenderCallbackStruct {
    inputProc: Some(render_callback),  // Called when output samples needed
    inputProcRefCon: render_state_refcon,
};
unsafe {
    set_property(
        audio_unit,
        kAudioUnitProperty_SetRenderCallback,
        kAudioUnitScope_Global,
        OUTPUT_BUS,
        &render_callback_struct,
        "failed to set render callback",
    )?;
}

// The actual render callback function (line ~165)
unsafe extern "C-unwind" fn render_callback(
    in_ref_con: NonNull<c_void>,
    _io_action_flags: NonNull<AudioUnitRenderActionFlags>,
    _in_time_stamp: NonNull<objc2_core_audio_types::AudioTimeStamp>,
    _in_bus_number: u32,
    _in_number_frames: u32,
    io_data: *mut AudioBufferList,
) -> i32 {
    if io_data.is_null() {
        return -50;
    }

    let state = &*(in_ref_con.as_ptr() as *const RenderCallbackState);
    let output_buffer = &mut (*io_data).mBuffers[0];
    let sample_count = (output_buffer.mDataByteSize as usize) / mem::size_of::<f32>();
    let output_samples = std::slice::from_raw_parts_mut(output_buffer.mData as *mut f32, sample_count);

    // Fill output buffer from playback queue
    if let Ok(mut buffer) = state.playback_buffer.try_lock() {
        for sample in output_samples.iter_mut() {
            *sample = buffer.samples.pop_front().unwrap_or(0.0);
        }
    } else {
        output_samples.fill(0.0);
    }

    0
}
```

**What's happening**:
1. VoiceProcessingIO calls this callback when it needs output samples
2. We provide samples from the playback queue
3. These samples are sent to the speaker
4. **At the exact same time**, the AEC engine sees these samples
5. The AEC engine correlates them with the input
6. This is how AEC knows what echo to subtract!

## The Complete Signal Flow in Code

```
┌─────────────────────────────────────────────────────────────────┐
│  Application Code (user's app)                                   │
│                                                                   │
│  1. Create AEC:                                                 │
│     let handle = CaptureHandle::new(config)?                    │
│                      ↓                                            │
│     Calls: create_backend(sender, config)                       │
│                                                                   │
│  2. Start playback stream:                                      │
│     let stream = handle.start_playback_stream(48000)?           │
│     stream.send(remote_audio)?  ← Audio from remote person      │
│                      ↓                                            │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ PLAYBACK PATH (What happens to remote audio)            │   │
│  │                                                          │   │
│  │ remote_audio (from network)                            │   │
│  │      ↓                                                  │   │
│  │ playback_buffer.push(remote_audio)                     │   │
│  │      ↓                                                  │   │
│  │ [Tokio async loop in backend]                          │   │
│  │      ↓                                                  │   │
│  │ VoiceProcessingIO needs output samples                 │   │
│  │      ↓                                                  │   │
│  │ Call: render_callback()                                │   │
│  │      ↓                                                  │   │
│  │ Pop samples from playback_buffer                       │   │
│  │      ↓                                                  │   │
│  │ Return to AudioUnit: here's what to send to speaker   │   │
│  │      ↓                                                  │   │
│  │ Speaker outputs: remote_audio to physical speaker     │   │
│  │      ↓                                                  │   │
│  │ ┌─────────────────────────────────────────────────┐  │   │
│  │ │  REAL WORLD: Acoustic Echo happens here!       │  │   │
│  │ │  ┌─────────────────────────────────────────┐   │  │   │
│  │ │  │ Speaker plays: "Hello, can you hear me"│   │  │   │
│  │ │  │    Sound travels through air            │   │  │   │
│  │ │  │       ↓                                  │   │  │   │
│  │ │  │  Microphone picks up (if in same room):│   │  │   │
│  │ │  │  "Hello, can you hear me" + other sound│   │  │   │
│  │ │  └─────────────────────────────────────────┘   │  │   │
│  │ └─────────────────────────────────────────────────┘  │   │
│  │      ↓                                                │   │
│  │ Microphone: [voice + echo + noise]                  │   │
│  └──────────────────────────────────────────────────────┘   │
│                      ↓                                        │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ CAPTURE PATH (What happens to microphone input)     │   │
│  │                                                      │   │
│  │ Microphone input: [voice + echo + noise]            │   │
│  │      ↓                                               │   │
│  │ VoiceProcessingIO (both buses active!)              │   │
│  │ ├─ Input Bus: receives mic data                     │   │
│  │ ├─ Output Bus: SEES what was just sent to speaker  │   │
│  │ ├─ AEC Engine: compares both                        │   │
│  │ │  └─ "That echo looks like the audio we sent"      │   │
│  │ │  └─ "Let me model the acoustic path"              │   │
│  │ │  └─ "Subtract the prediction from input"          │   │
│  │ │  └─ Result: [voice] (echo removed!)               │   │
│  │ ├─ AGC: normalize volume                             │   │
│  │ ├─ Noise Suppression: reduce background noise       │   │
│  │      ↓                                               │   │
│  │ Processed output: [clean voice, no echo]            │   │
│  │      ↓                                               │   │
│  │ Call: input_callback()                              │   │
│  │      ↓                                               │   │
│  │ AudioUnitRender() retrieves processed data          │   │
│  │      ↓                                               │   │
│  │ Send to app: clean voice samples                    │   │
│  │      ↓                                               │   │
│  │ capture_tx.send(clean_voice)                        │   │
│  │      ↓                                               │   │
│  │ channel_rx receives: clean_voice                    │   │
│  │      ↓                                               │   │
│  │ Backend task forwards to public interface           │   │
│  │      ↓                                               │   │
│  │ App code receives clean voice from handler.recv()   │   │
│  │      ↓                                               │   │
│  │ Send clean voice BACK to remote person              │   │
│  │ (Remote person does NOT hear their echo!)           │   │
│  │      ↓                                               │   │
│  │ Network → Remote person: hears only your voice      │   │
│  │                                                      │   │
│  └──────────────────────────────────────────────────────┘   │
│                      ↓                                        │
│  3. Receive processed audio:                                │
│     while let Some(clean_samples) = handle.recv_async()     │
│     {                                                        │
│         send_to_remote_person(clean_samples)                │
│     }                                                        │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## The Key Insight: Same AudioUnit

The entire magic of AEC working is because:

```
┌──────────────────────────────────────────┐
│   VoiceProcessingIO AudioUnit            │
├──────────────────────────────────────────┤
│                                          │
│  Both INPUT and OUTPUT connect to:       │
│  - Same physical component               │
│  - Same AEC engine                       │
│  - Same callbacks                        │
│                                          │
│  So AEC can:                             │
│  1. Measure what's being output          │
│  2. Measure what's being input           │
│  3. Model the acoustic path              │
│  4. Subtract output from input           │
│  5. Deliver clean voice to app           │
│                                          │
└──────────────────────────────────────────┘
```

If you used SEPARATE audio units for input and output:
- Input audio unit: would see microphone input only
- Output audio unit: would output to speaker
- AEC would be impossible: input unit has no idea what was played

**This is why your architecture is correct!**

## How Your Implementation Proves It Works

The evidence that your implementation correctly couples input and output:

1. **Both buses enabled on same unit** (line ~250-260):
   ```rust
   kAudioUnitScope_Input + INPUT_BUS     // Microphone
   kAudioUnitScope_Output + OUTPUT_BUS    // Speaker
   ```

2. **Both have callbacks on same unit** (line ~315, ~323):
   ```rust
   kAudioOutputUnitProperty_SetInputCallback    // When input ready
   kAudioUnitProperty_SetRenderCallback         // When output needed
   ```

3. **Both use same callbacks structure** in same AEC instance

4. **Render callback provides output samples** (line ~170):
   - VoiceProcessingIO engine SEES these
   - Uses them as reference for AEC

5. **Input callback receives processed samples** (line ~130):
   - Already processed by AEC
   - Echo subtracted out

## Testing This Yourself

To verify AEC is actually working in your sys-voice:

```rust
// Run the aec_test example
cargo run --example aec_test

// You'll hear a 440Hz test tone playing from speaker
// While recording from microphone for 10 seconds
// If AEC is working correctly:
// - Your voice will be clearly heard in recording
// - The 440Hz tone will be GONE or barely audible
// - Proof that output was subtracted from input!
```

This demonstrates that:
1. Both input and output are flowing through same AEC engine
2. The output signal (440Hz tone) is being used as reference
3. AEC is modeling the echo path (acoustic coupling from speaker to mic)
4. AEC is successfully subtracting it from input
5. Result: clean voice without the test tone

**That's your proof that input/output coupling is working!**
