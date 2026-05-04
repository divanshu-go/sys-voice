use crate::backends::PlaybackCommand;
use crate::{AecConfig, AecError, DuckingLevel};
use flume::{Receiver, Sender};
use objc2_audio_toolbox::{
    AURenderCallbackStruct, AudioComponentDescription, AudioComponentFindNext,
    AudioComponentInstanceDispose, AudioComponentInstanceNew, AudioOutputUnitStart,
    AudioOutputUnitStop, AudioUnit, AudioUnitGetProperty, AudioUnitInitialize, AudioUnitRender,
    AudioUnitSetProperty, AudioUnitUninitialize,
    AUVoiceIOOtherAudioDuckingConfiguration, AUVoiceIOOtherAudioDuckingLevel,
    kAUVoiceIOProperty_OtherAudioDuckingConfiguration,
    kAUVoiceIOProperty_VoiceProcessingEnableAGC,
    kAudioOutputUnitProperty_EnableIO, kAudioOutputUnitProperty_SetInputCallback,
    kAudioUnitManufacturer_Apple, kAudioUnitProperty_MaximumFramesPerSlice,
    kAudioUnitProperty_SetRenderCallback, kAudioUnitProperty_StreamFormat, kAudioUnitScope_Global,
    kAudioUnitScope_Input, kAudioUnitScope_Output, kAudioUnitSubType_VoiceProcessingIO,
    kAudioUnitType_Output,
};
use objc2_audio_toolbox::AudioUnitRenderActionFlags;
use objc2_core_audio_types::{
    AudioBuffer, AudioBufferList, AudioStreamBasicDescription, kAudioFormatFlagIsFloat,
    kAudioFormatFlagIsNonInterleaved, kAudioFormatFlagIsPacked, kAudioFormatLinearPCM,
};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::mem::{self, MaybeUninit};
use std::ptr::{self, NonNull};
use std::sync::{Arc, Mutex};

const INPUT_BUS: u32 = 1;
const OUTPUT_BUS: u32 = 0;

fn to_ducking_level(level: DuckingLevel) -> AUVoiceIOOtherAudioDuckingLevel {
    match level {
        DuckingLevel::Default => AUVoiceIOOtherAudioDuckingLevel::Default,
        DuckingLevel::Min => AUVoiceIOOtherAudioDuckingLevel::Min,
        DuckingLevel::Mid => AUVoiceIOOtherAudioDuckingLevel::Mid,
        DuckingLevel::Max => AUVoiceIOOtherAudioDuckingLevel::Max,
    }
}

#[derive(Copy, Clone)]
struct AudioUnitHandle(AudioUnit);

unsafe impl Send for AudioUnitHandle {}

/// Shared buffer for playback samples.
struct PlaybackBuffer {
    samples: VecDeque<f32>,
}

struct InputCallbackState {
    audio_unit: AudioUnitHandle,
    capture_tx: Sender<Vec<f32>>,
}

struct RenderCallbackState {
    playback_buffer: Arc<Mutex<PlaybackBuffer>>,
}

fn os_status_to_result(status: i32, context: &str) -> Result<(), AecError> {
    if status == 0 {
        Ok(())
    } else {
        Err(AecError::BackendError(format!("{context}: OSStatus {status}")))
    }
}

unsafe fn set_property<T>(
    audio_unit: AudioUnit,
    property_id: u32,
    scope: u32,
    element: u32,
    value: &T,
    context: &str,
) -> Result<(), AecError> {
    let status = AudioUnitSetProperty(
        audio_unit,
        property_id,
        scope,
        element,
        value as *const T as *const c_void,
        mem::size_of::<T>() as u32,
    );
    os_status_to_result(status, context)
}

unsafe fn get_property<T: Copy>(
    audio_unit: AudioUnit,
    property_id: u32,
    scope: u32,
    element: u32,
    context: &str,
) -> Result<T, AecError> {
    let mut value = MaybeUninit::<T>::uninit();
    let mut size = mem::size_of::<T>() as u32;
    let status = AudioUnitGetProperty(
        audio_unit,
        property_id,
        scope,
        element,
        NonNull::new(value.as_mut_ptr() as *mut c_void)
            .expect("MaybeUninit pointer is never null"),
        NonNull::new(&mut size).expect("stack pointer is never null"),
    );
    os_status_to_result(status, context)?;
    Ok(value.assume_init())
}

unsafe fn build_stream_format(sample_rate: f64) -> AudioStreamBasicDescription {
    AudioStreamBasicDescription {
        mSampleRate: sample_rate,
        mFormatID: kAudioFormatLinearPCM,
        mFormatFlags: kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked | kAudioFormatFlagIsNonInterleaved,
        mBytesPerPacket: mem::size_of::<f32>() as u32,
        mFramesPerPacket: 1,
        mBytesPerFrame: mem::size_of::<f32>() as u32,
        mChannelsPerFrame: 1,
        mBitsPerChannel: (mem::size_of::<f32>() * 8) as u32,
        mReserved: 0,
    }
}

unsafe extern "C-unwind" fn input_callback(
    in_ref_con: NonNull<c_void>,
    io_action_flags: NonNull<AudioUnitRenderActionFlags>,
    in_time_stamp: NonNull<objc2_core_audio_types::AudioTimeStamp>,
    in_bus_number: u32,
    in_number_frames: u32,
    _io_data: *mut AudioBufferList,
) -> i32 {
    let state = &*(in_ref_con.as_ptr() as *const InputCallbackState);
    let sample_count = in_number_frames as usize;
    let mut samples = vec![0.0f32; sample_count];
    let audio_buffer = AudioBuffer {
        mNumberChannels: 1,
        mDataByteSize: (sample_count * mem::size_of::<f32>()) as u32,
        mData: samples.as_mut_ptr() as *mut c_void,
    };
    let mut audio_buffer_list = AudioBufferList {
        mNumberBuffers: 1,
        mBuffers: [audio_buffer],
    };

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

    let _ = state.capture_tx.try_send(samples);
    0
}

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

    if let Ok(mut buffer) = state.playback_buffer.try_lock() {
        for sample in output_samples.iter_mut() {
            *sample = buffer.samples.pop_front().unwrap_or(0.0);
        }
    } else {
        output_samples.fill(0.0);
    }

    0
}

/// Create macOS backend. Spawns a task that owns audio resources.
/// Returns (sample_rate, buffer_size). Task stops when sender fails.
pub fn create_backend(
    public_sender: Sender<Vec<f32>>,
    playback_rx: Receiver<PlaybackCommand>,
    config: AecConfig,
) -> Result<(u32, usize), AecError> {
    let (callback_tx, callback_rx) = flume::bounded::<Vec<f32>>(32);

    let playback_buffer = Arc::new(Mutex::new(PlaybackBuffer {
        samples: VecDeque::with_capacity(48_000),
    }));

    let audio_unit_description = AudioComponentDescription {
        componentType: kAudioUnitType_Output,
        componentSubType: kAudioUnitSubType_VoiceProcessingIO,
        componentManufacturer: kAudioUnitManufacturer_Apple,
        componentFlags: 0,
        componentFlagsMask: 0,
    };

    let component = unsafe { AudioComponentFindNext(ptr::null_mut(), NonNull::from(&audio_unit_description)) };
    if component.is_null() {
        return Err(AecError::BackendError("failed to find VoiceProcessingIO component".to_string()));
    }

    let mut audio_unit = ptr::null_mut();
    let status = unsafe { AudioComponentInstanceNew(component, NonNull::from(&mut audio_unit)) };
    os_status_to_result(status, "failed to create VoiceProcessingIO")?;

    let enable_io: u32 = 1;
    unsafe {
        set_property(
            audio_unit,
            kAudioOutputUnitProperty_EnableIO,
            kAudioUnitScope_Input,
            INPUT_BUS,
            &enable_io,
            "failed to enable input IO",
        )?;
        set_property(
            audio_unit,
            kAudioOutputUnitProperty_EnableIO,
            kAudioUnitScope_Output,
            OUTPUT_BUS,
            &enable_io,
            "failed to enable output IO",
        )?;

        if config.enable_advanced_ducking || config.ducking_level != DuckingLevel::Default {
            let ducking_config = AUVoiceIOOtherAudioDuckingConfiguration {
                mEnableAdvancedDucking: if config.enable_advanced_ducking { 1 } else { 0 },
                mDuckingLevel: to_ducking_level(config.ducking_level),
            };
            set_property(
                audio_unit,
                kAUVoiceIOProperty_OtherAudioDuckingConfiguration,
                kAudioUnitScope_Global,
                OUTPUT_BUS,
                &ducking_config,
                "failed to configure VoiceProcessingIO ducking (requires supported macOS runtime)",
            )?;
        }

        if let Some(enable_agc) = config.voice_processing_enable_agc {
            let agc_enabled: u32 = if enable_agc { 1 } else { 0 };
            set_property(
                audio_unit,
                kAUVoiceIOProperty_VoiceProcessingEnableAGC,
                kAudioUnitScope_Global,
                OUTPUT_BUS,
                &agc_enabled,
                "failed to configure VoiceProcessingIO AGC",
            )?;
        }
    }

    let native_format = unsafe {
        get_property::<AudioStreamBasicDescription>(
            audio_unit,
            kAudioUnitProperty_StreamFormat,
            kAudioUnitScope_Output,
            INPUT_BUS,
            "failed to get native stream format",
        )?
    };

    let stream_format = unsafe { build_stream_format(native_format.mSampleRate) };
    unsafe {
        set_property(
            audio_unit,
            kAudioUnitProperty_StreamFormat,
            kAudioUnitScope_Output,
            INPUT_BUS,
            &stream_format,
            "failed to set input stream format",
        )?;
        set_property(
            audio_unit,
            kAudioUnitProperty_StreamFormat,
            kAudioUnitScope_Input,
            OUTPUT_BUS,
            &stream_format,
            "failed to set output stream format",
        )?;
    }

    let audio_unit_handle = AudioUnitHandle(audio_unit);

    let input_state = Box::new(InputCallbackState {
        audio_unit: audio_unit_handle,
        capture_tx: callback_tx,
    });
    let input_state_refcon = (&*input_state as *const InputCallbackState).cast::<c_void>() as *mut c_void;

    let render_state = Box::new(RenderCallbackState {
        playback_buffer: playback_buffer.clone(),
    });
    let render_state_refcon = (&*render_state as *const RenderCallbackState).cast::<c_void>() as *mut c_void;

    let input_callback = AURenderCallbackStruct {
        inputProc: Some(input_callback),
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

    let render_callback = AURenderCallbackStruct {
        inputProc: Some(render_callback),
        inputProcRefCon: render_state_refcon,
    };
    unsafe {
        set_property(
            audio_unit,
            kAudioUnitProperty_SetRenderCallback,
            kAudioUnitScope_Input,
            OUTPUT_BUS,
            &render_callback,
            "failed to set render callback",
        )?;
    }

    unsafe {
        os_status_to_result(AudioUnitInitialize(audio_unit), "failed to initialize VoiceProcessingIO")?;
        os_status_to_result(AudioOutputUnitStart(audio_unit), "failed to start VoiceProcessingIO")?;
    }

    let buffer_size: u32 = unsafe {
        get_property::<u32>(
            audio_unit,
            kAudioUnitProperty_MaximumFramesPerSlice,
            kAudioUnitScope_Global,
            OUTPUT_BUS,
            "failed to query maximum frames per slice",
        )
        .unwrap_or(512)
    };

    let buffer_for_playback = playback_buffer.clone();
    tokio::spawn(async move {
        while let Ok(command) = playback_rx.recv_async().await {
            match command {
                PlaybackCommand::OneShot(samples) => {
                    let Ok(mut buffer) = buffer_for_playback.lock() else {
                        continue;
                    };
                    buffer.samples.extend(samples);
                }
                PlaybackCommand::StartStream(chunk_rx) => {
                    let playback_buffer = buffer_for_playback.clone();
                    tokio::spawn(async move {
                        while let Ok(samples) = chunk_rx.recv_async().await {
                            let Ok(mut buffer) = playback_buffer.lock() else {
                                break;
                            };
                            buffer.samples.extend(samples);
                        }
                    });
                }
            }
        }
    });

    tokio::spawn(async move {
        let _input_state = input_state;
        let _render_state = render_state;
        let _audio_unit = audio_unit_handle;

        while let Ok(samples) = callback_rx.recv_async().await {
            if public_sender.send_async(samples).await.is_err() {
                break;
            }
        }

        unsafe {
            let _ = AudioOutputUnitStop(_audio_unit.0);
            let _ = AudioUnitUninitialize(_audio_unit.0);
            let _ = AudioComponentInstanceDispose(_audio_unit.0);
        }
    });

    Ok((native_format.mSampleRate as u32, buffer_size as usize))
}
