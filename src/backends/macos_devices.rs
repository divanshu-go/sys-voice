use std::ptr;
use std::mem;
use std::ptr::NonNull;
use std::ffi::CStr;

use objc2_core_audio::AudioObjectPropertyAddress;
use objc2_core_audio::AudioObjectGetPropertyDataSize;
use objc2_core_audio::AudioObjectGetPropertyData;
use objc2_core_audio::kAudioHardwarePropertyDevices;
use objc2_core_audio::kAudioObjectSystemObject;
use objc2_core_audio::kAudioObjectPropertyScopeGlobal;
use objc2_core_audio::kAudioObjectPropertyElementMain;
use objc2_core_audio::kAudioDevicePropertyDeviceNameCFString;
use objc2_core_audio::kAudioDevicePropertyStreams;
use objc2_core_audio::kAudioObjectPropertyScopeInput;
use objc2_core_audio::kAudioObjectPropertyScopeOutput;
use objc2_core_audio::kAudioDevicePropertyNominalSampleRate;

use objc2_core_foundation::CFString;

use crate::AudioDeviceInfo;
use crate::AecError;

pub fn list_audio_devices() -> Result<Vec<AudioDeviceInfo>, AecError> {
    unsafe {
        let property_address = AudioObjectPropertyAddress {
            mSelector: kAudioHardwarePropertyDevices,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain,
        };

        let mut data_size: u32 = 0;
        let status = AudioObjectGetPropertyDataSize(
            kAudioObjectSystemObject as u32,
            NonNull::from(&property_address),
            0,
            ptr::null(),
            NonNull::new(&mut data_size).expect("non-null"),
        );
        if status != 0 {
            return Err(AecError::BackendError(format!("AudioObjectGetPropertyDataSize failed: {}", status)));
        }

        let count = (data_size as usize) / mem::size_of::<u32>();
        let mut ids: Vec<u32> = vec![0u32; count];
        let mut data_size_mut = data_size;
        let status = AudioObjectGetPropertyData(
            kAudioObjectSystemObject as u32,
            NonNull::from(&property_address),
            0,
            ptr::null(),
            NonNull::new(&mut data_size_mut).expect("non-null"),
            NonNull::new(ids.as_mut_ptr() as *mut _).expect("non-null").cast(),
        );
        if status != 0 {
            return Err(AecError::BackendError(format!("AudioObjectGetPropertyData failed: {}", status)));
        }

        let mut out = Vec::with_capacity(count);
        for dev in ids {
            // name
            let name_address = AudioObjectPropertyAddress {
                mSelector: kAudioDevicePropertyDeviceNameCFString,
                mScope: kAudioObjectPropertyScopeGlobal,
                mElement: kAudioObjectPropertyElementMain,
            };
            let mut cf_ptr: *const CFString = ptr::null();
            let mut name_size = mem::size_of::<*const CFString>() as u32;
            let cf_ptr_ptr: *mut *const CFString = &mut cf_ptr;
            let _ = AudioObjectGetPropertyData(
                dev as u32,
                NonNull::from(&name_address),
                0,
                ptr::null(),
                NonNull::new(&mut name_size).expect("non-null"),
                NonNull::new(cf_ptr_ptr as *mut _).expect("non-null").cast(),
            );
            let name = if !cf_ptr.is_null() {
                let cf = &*cf_ptr;
                let mut buf = vec![0i8; 512];
                let ok = cf.c_string(buf.as_mut_ptr(), buf.len() as isize, 0x08000100);
                if ok {
                    CStr::from_ptr(buf.as_ptr() as *const i8)
                        .to_string_lossy()
                        .into_owned()
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            // is_input
            let streams_in_address = AudioObjectPropertyAddress {
                mSelector: kAudioDevicePropertyStreams,
                mScope: kAudioObjectPropertyScopeInput,
                mElement: kAudioObjectPropertyElementMain,
            };
            let mut in_size: u32 = 0;
            let in_support = AudioObjectGetPropertyDataSize(dev as u32, NonNull::from(&streams_in_address), 0, ptr::null(), NonNull::new(&mut in_size).expect("non-null"));
            let is_input = in_support == 0 && in_size > 0;

            // is_output
            let streams_out_address = AudioObjectPropertyAddress {
                mSelector: kAudioDevicePropertyStreams,
                mScope: kAudioObjectPropertyScopeOutput,
                mElement: kAudioObjectPropertyElementMain,
            };
            let mut out_size: u32 = 0;
            let out_support = AudioObjectGetPropertyDataSize(dev as u32, NonNull::from(&streams_out_address), 0, ptr::null(), NonNull::new(&mut out_size).expect("non-null"));
            let is_output = out_support == 0 && out_size > 0;

            // sample rate
            let rate_address = AudioObjectPropertyAddress {
                mSelector: kAudioDevicePropertyNominalSampleRate,
                mScope: kAudioObjectPropertyScopeGlobal,
                mElement: kAudioObjectPropertyElementMain,
            };
            let mut sample_rate: f64 = 0.0;
            let mut rate_size = mem::size_of::<f64>() as u32;
            let _ = AudioObjectGetPropertyData(dev as u32, NonNull::from(&rate_address), 0, ptr::null(), NonNull::new(&mut rate_size).expect("non-null"), NonNull::new(&mut sample_rate as *mut f64 as *mut _).expect("non-null").cast());

            out.push(AudioDeviceInfo {
                id: dev,
                name,
                is_input,
                is_output,
                sample_rate,
                channel_count: 0,
            });
        }

        Ok(out)
    }
}
