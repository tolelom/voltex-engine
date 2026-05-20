#![allow(non_snake_case, non_camel_case_types, dead_code)]
//! WASAPI FFI bindings for Windows audio output.
//!
//! This module is only compiled on Windows (`#[cfg(target_os = "windows")]`).

use std::ffi::c_void;

// ---------------------------------------------------------------------------
// Basic Windows types
// ---------------------------------------------------------------------------

pub type HRESULT = i32;
pub type ULONG = u32;
pub type DWORD = u32;
pub type WORD = u16;
pub type BOOL = i32;
pub type UINT = u32;
pub type UINT32 = u32;
pub type BYTE = u8;
pub type REFERENCE_TIME = i64;
pub type HANDLE = *mut c_void;
pub type LPVOID = *mut c_void;
pub type LPCWSTR = *const u16;

pub const S_OK: HRESULT = 0;
pub const AUDCLNT_SHAREMODE_SHARED: u32 = 0;
pub const AUDCLNT_STREAMFLAGS_RATEADJUST: u32 = 0x00100000;
pub const CLSCTX_ALL: DWORD = 0x17;
pub const COINIT_APARTMENTTHREADED: DWORD = 0x2;
pub const DEVICE_STATE_ACTIVE: DWORD = 0x1;
pub const eRender: u32 = 0;
pub const eConsole: u32 = 0;
pub const WAVE_FORMAT_PCM: WORD = 1;
pub const WAVE_FORMAT_IEEE_FLOAT: WORD = 3;
pub const WAVE_FORMAT_EXTENSIBLE: WORD = 0xFFFE;

// ---------------------------------------------------------------------------
// GUID
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GUID {
    pub Data1: u32,
    pub Data2: u16,
    pub Data3: u16,
    pub Data4: [u8; 8],
}

/// CLSID_MMDeviceEnumerator: {BCDE0395-E52F-467C-8E3D-C4579291692E}
pub const CLSID_MMDeviceEnumerator: GUID = GUID {
    Data1: 0xBCDE0395,
    Data2: 0xE52F,
    Data3: 0x467C,
    Data4: [0x8E, 0x3D, 0xC4, 0x57, 0x92, 0x91, 0x69, 0x2E],
};

/// IID_IMMDeviceEnumerator: {A95664D2-9614-4F35-A746-DE8DB63617E6}
pub const IID_IMMDeviceEnumerator: GUID = GUID {
    Data1: 0xA95664D2,
    Data2: 0x9614,
    Data3: 0x4F35,
    Data4: [0xA7, 0x46, 0xDE, 0x8D, 0xB6, 0x36, 0x17, 0xE6],
};

/// IID_IAudioClient: {1CB9AD4C-DBFA-4c32-B178-C2F568A703B2}
pub const IID_IAudioClient: GUID = GUID {
    Data1: 0x1CB9AD4C,
    Data2: 0xDBFA,
    Data3: 0x4C32,
    Data4: [0xB1, 0x78, 0xC2, 0xF5, 0x68, 0xA7, 0x03, 0xB2],
};

/// IID_IAudioRenderClient: {F294ACFC-3146-4483-A7BF-ADDCA7C260E2}
pub const IID_IAudioRenderClient: GUID = GUID {
    Data1: 0xF294ACFC,
    Data2: 0x3146,
    Data3: 0x4483,
    Data4: [0xA7, 0xBF, 0xAD, 0xDC, 0xA7, 0xC2, 0x60, 0xE2],
};

// ---------------------------------------------------------------------------
// WAVEFORMATEX
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct WAVEFORMATEX {
    pub wFormatTag: WORD,
    pub nChannels: WORD,
    pub nSamplesPerSec: DWORD,
    pub nAvgBytesPerSec: DWORD,
    pub nBlockAlign: WORD,
    pub wBitsPerSample: WORD,
    pub cbSize: WORD,
}

// ---------------------------------------------------------------------------
// COM vtable structs
// ---------------------------------------------------------------------------

/// IUnknown vtable (base for all COM interfaces)
#[repr(C)]
pub struct IUnknownVtbl {
    pub QueryInterface: unsafe extern "system" fn(
        this: *mut c_void,
        riid: *const GUID,
        ppvObject: *mut *mut c_void,
    ) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(this: *mut c_void) -> ULONG,
    pub Release: unsafe extern "system" fn(this: *mut c_void) -> ULONG,
}

/// IMMDeviceEnumerator vtable
#[repr(C)]
pub struct IMMDeviceEnumeratorVtbl {
    pub base: IUnknownVtbl,
    pub EnumAudioEndpoints: unsafe extern "system" fn(
        this: *mut c_void,
        dataFlow: u32,
        dwStateMask: DWORD,
        ppDevices: *mut *mut c_void,
    ) -> HRESULT,
    pub GetDefaultAudioEndpoint: unsafe extern "system" fn(
        this: *mut c_void,
        dataFlow: u32,
        role: u32,
        ppEndpoint: *mut *mut c_void,
    ) -> HRESULT,
    pub GetDevice: unsafe extern "system" fn(
        this: *mut c_void,
        pwstrId: LPCWSTR,
        ppDevice: *mut *mut c_void,
    ) -> HRESULT,
    pub RegisterEndpointNotificationCallback: unsafe extern "system" fn(
        this: *mut c_void,
        pClient: *mut c_void,
    ) -> HRESULT,
    pub UnregisterEndpointNotificationCallback: unsafe extern "system" fn(
        this: *mut c_void,
        pClient: *mut c_void,
    ) -> HRESULT,
}

/// IMMDevice vtable
#[repr(C)]
pub struct IMMDeviceVtbl {
    pub base: IUnknownVtbl,
    pub Activate: unsafe extern "system" fn(
        this: *mut c_void,
        iid: *const GUID,
        dwClsCtx: DWORD,
        pActivationParams: *mut c_void,
        ppInterface: *mut *mut c_void,
    ) -> HRESULT,
    pub OpenPropertyStore: unsafe extern "system" fn(
        this: *mut c_void,
        stgmAccess: DWORD,
        ppProperties: *mut *mut c_void,
    ) -> HRESULT,
    pub GetId: unsafe extern "system" fn(
        this: *mut c_void,
        ppstrId: *mut LPCWSTR,
    ) -> HRESULT,
    pub GetState: unsafe extern "system" fn(
        this: *mut c_void,
        pdwState: *mut DWORD,
    ) -> HRESULT,
}

/// IAudioClient vtable
#[repr(C)]
pub struct IAudioClientVtbl {
    pub base: IUnknownVtbl,
    pub Initialize: unsafe extern "system" fn(
        this: *mut c_void,
        ShareMode: u32,
        StreamFlags: DWORD,
        hnsBufferDuration: REFERENCE_TIME,
        hnsPeriodicity: REFERENCE_TIME,
        pFormat: *const WAVEFORMATEX,
        AudioSessionGuid: *const GUID,
    ) -> HRESULT,
    pub GetBufferSize: unsafe extern "system" fn(
        this: *mut c_void,
        pNumBufferFrames: *mut UINT32,
    ) -> HRESULT,
    pub GetStreamLatency: unsafe extern "system" fn(
        this: *mut c_void,
        phnsLatency: *mut REFERENCE_TIME,
    ) -> HRESULT,
    pub GetCurrentPadding: unsafe extern "system" fn(
        this: *mut c_void,
        pNumPaddingFrames: *mut UINT32,
    ) -> HRESULT,
    pub IsFormatSupported: unsafe extern "system" fn(
        this: *mut c_void,
        ShareMode: u32,
        pFormat: *const WAVEFORMATEX,
        ppClosestMatch: *mut *mut WAVEFORMATEX,
    ) -> HRESULT,
    pub GetMixFormat: unsafe extern "system" fn(
        this: *mut c_void,
        ppDeviceFormat: *mut *mut WAVEFORMATEX,
    ) -> HRESULT,
    pub GetDevicePeriod: unsafe extern "system" fn(
        this: *mut c_void,
        phnsDefaultDevicePeriod: *mut REFERENCE_TIME,
        phnsMinimumDevicePeriod: *mut REFERENCE_TIME,
    ) -> HRESULT,
    pub Start: unsafe extern "system" fn(this: *mut c_void) -> HRESULT,
    pub Stop: unsafe extern "system" fn(this: *mut c_void) -> HRESULT,
    pub Reset: unsafe extern "system" fn(this: *mut c_void) -> HRESULT,
    pub SetEventHandle: unsafe extern "system" fn(
        this: *mut c_void,
        eventHandle: HANDLE,
    ) -> HRESULT,
    pub GetService: unsafe extern "system" fn(
        this: *mut c_void,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> HRESULT,
}

/// IAudioRenderClient vtable
#[repr(C)]
pub struct IAudioRenderClientVtbl {
    pub base: IUnknownVtbl,
    pub GetBuffer: unsafe extern "system" fn(
        this: *mut c_void,
        NumFramesRequested: UINT32,
        ppData: *mut *mut BYTE,
    ) -> HRESULT,
    pub ReleaseBuffer: unsafe extern "system" fn(
        this: *mut c_void,
        NumFramesWritten: UINT32,
        dwFlags: DWORD,
    ) -> HRESULT,
}

// ---------------------------------------------------------------------------
// extern "system" functions
// ---------------------------------------------------------------------------

#[link(name = "ole32")]
extern "system" {
    pub fn CoInitializeEx(pvReserved: LPVOID, dwCoInit: DWORD) -> HRESULT;
    pub fn CoUninitialize();
    pub fn CoCreateInstance(
        rclsid: *const GUID,
        pUnkOuter: *mut c_void,
        dwClsContext: DWORD,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> HRESULT;
    pub fn CoTaskMemFree(pv: LPVOID);
}

// ---------------------------------------------------------------------------
// WasapiDevice
// ---------------------------------------------------------------------------

pub struct WasapiDevice {
    client: *mut c_void,
    render_client: *mut c_void,
    buffer_size: u32,
    pub sample_rate: u32,
    pub channels: u16,
    bits_per_sample: u16,
    is_float: bool,
}

unsafe impl Send for WasapiDevice {}

impl WasapiDevice {
    /// Initialize WASAPI: COM, default endpoint, IAudioClient, mix format,
    /// Initialize shared mode (50 ms buffer), GetBufferSize, GetService, Start.
    pub fn new() -> crate::Result<Self> {
        unsafe {
            // 1. CoInitializeEx
            let hr = CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED);
            if hr < 0 {
                return Err(format!("CoInitializeEx failed: 0x{:08X}", hr as u32).into());
            }

            // 2. CoCreateInstance -> IMMDeviceEnumerator
            let mut enumerator: *mut c_void = std::ptr::null_mut();
            let hr = CoCreateInstance(
                &CLSID_MMDeviceEnumerator,
                std::ptr::null_mut(),
                CLSCTX_ALL,
                &IID_IMMDeviceEnumerator,
                &mut enumerator,
            );
            if hr < 0 || enumerator.is_null() {
                CoUninitialize();
                return Err(format!("CoCreateInstance(MMDeviceEnumerator) failed: 0x{:08X}", hr as u32).into());
            }

            // 3. GetDefaultAudioEndpoint -> IMMDevice
            let mut device: *mut c_void = std::ptr::null_mut();
            {
                let vtbl = *(enumerator as *mut *const IMMDeviceEnumeratorVtbl);
                let hr = ((*vtbl).GetDefaultAudioEndpoint)(enumerator, eRender, eConsole, &mut device);
                ((*vtbl).base.Release)(enumerator);
                if hr < 0 || device.is_null() {
                    CoUninitialize();
                    return Err(format!("GetDefaultAudioEndpoint failed: 0x{:08X}", hr as u32).into());
                }
            }

            // 4. IMMDevice::Activate -> IAudioClient
            let mut client: *mut c_void = std::ptr::null_mut();
            {
                let vtbl = *(device as *mut *const IMMDeviceVtbl);
                let hr = ((*vtbl).Activate)(
                    device,
                    &IID_IAudioClient,
                    CLSCTX_ALL,
                    std::ptr::null_mut(),
                    &mut client,
                );
                ((*vtbl).base.Release)(device);
                if hr < 0 || client.is_null() {
                    CoUninitialize();
                    return Err(format!("IMMDevice::Activate(IAudioClient) failed: 0x{:08X}", hr as u32).into());
                }
            }

            // 5. GetMixFormat
            let mut mix_format_ptr: *mut WAVEFORMATEX = std::ptr::null_mut();
            {
                let vtbl = *(client as *mut *const IAudioClientVtbl);
                let hr = ((*vtbl).GetMixFormat)(client, &mut mix_format_ptr);
                if hr < 0 || mix_format_ptr.is_null() {
                    ((*vtbl).base.Release)(client);
                    CoUninitialize();
                    return Err(format!("GetMixFormat failed: 0x{:08X}", hr as u32).into());
                }
            }

            let mix_format = *mix_format_ptr;
            let sample_rate = mix_format.nSamplesPerSec;
            let channels = mix_format.nChannels;
            let bits_per_sample = mix_format.wBitsPerSample;

            // Determine float vs int
            let is_float = match mix_format.wFormatTag {
                WAVE_FORMAT_IEEE_FLOAT => true,
                WAVE_FORMAT_EXTENSIBLE => bits_per_sample == 32,
                _ => false,
            };

            // 6. IAudioClient::Initialize (shared mode, 50 ms = 500_000 REFERENCE_TIME)
            const BUFFER_DURATION: REFERENCE_TIME = 500_000; // 50 ms in 100-ns units
            let hr = {
                let vtbl = *(client as *mut *const IAudioClientVtbl);
                ((*vtbl).Initialize)(
                    client,
                    AUDCLNT_SHAREMODE_SHARED,
                    0,
                    BUFFER_DURATION,
                    0,
                    mix_format_ptr,
                    std::ptr::null(),
                )
            };
            CoTaskMemFree(mix_format_ptr as LPVOID);
            if hr < 0 {
                let vtbl = *(client as *mut *const IAudioClientVtbl);
                ((*vtbl).base.Release)(client);
                CoUninitialize();
                return Err(format!("IAudioClient::Initialize failed: 0x{:08X}", hr as u32).into());
            }

            // 7. GetBufferSize
            let mut buffer_size: UINT32 = 0;
            {
                let vtbl = *(client as *mut *const IAudioClientVtbl);
                let hr = ((*vtbl).GetBufferSize)(client, &mut buffer_size);
                if hr < 0 {
                    ((*vtbl).base.Release)(client);
                    CoUninitialize();
                    return Err(format!("GetBufferSize failed: 0x{:08X}", hr as u32).into());
                }
            }

            // 8. GetService -> IAudioRenderClient
            let mut render_client: *mut c_void = std::ptr::null_mut();
            {
                let vtbl = *(client as *mut *const IAudioClientVtbl);
                let hr = ((*vtbl).GetService)(client, &IID_IAudioRenderClient, &mut render_client);
                if hr < 0 || render_client.is_null() {
                    ((*vtbl).base.Release)(client);
                    CoUninitialize();
                    return Err(format!("GetService(IAudioRenderClient) failed: 0x{:08X}", hr as u32).into());
                }
            }

            // 9. Start
            {
                let vtbl = *(client as *mut *const IAudioClientVtbl);
                let hr = ((*vtbl).Start)(client);
                if hr < 0 {
                    let rc_vtbl = *(render_client as *mut *const IAudioRenderClientVtbl);
                    ((*rc_vtbl).base.Release)(render_client);
                    ((*vtbl).base.Release)(client);
                    CoUninitialize();
                    return Err(format!("IAudioClient::Start failed: 0x{:08X}", hr as u32).into());
                }
            }

            Ok(WasapiDevice {
                client,
                render_client,
                buffer_size,
                sample_rate,
                channels,
                bits_per_sample,
                is_float,
            })
        }
    }

    /// Write f32 samples to the audio device.
    /// Returns the number of frames actually written.
    pub fn write_samples(&self, samples: &[f32]) -> crate::Result<usize> {
        unsafe {
            // GetCurrentPadding
            let mut padding: UINT32 = 0;
            {
                let vtbl = *(self.client as *mut *const IAudioClientVtbl);
                let hr = ((*vtbl).GetCurrentPadding)(self.client, &mut padding);
                if hr < 0 {
                    return Err(format!("GetCurrentPadding failed: 0x{:08X}", hr as u32).into());
                }
            }

            let available_frames = if self.buffer_size > padding {
                self.buffer_size - padding
            } else {
                return Ok(0);
            };

            let samples_per_frame = self.channels as usize;
            let input_frames = samples.len() / samples_per_frame;
            let frames_to_write = available_frames.min(input_frames as u32);

            if frames_to_write == 0 {
                return Ok(0);
            }

            // GetBuffer
            let mut data_ptr: *mut BYTE = std::ptr::null_mut();
            {
                let vtbl = *(self.render_client as *mut *const IAudioRenderClientVtbl);
                let hr = ((*vtbl).GetBuffer)(self.render_client, frames_to_write, &mut data_ptr);
                if hr < 0 || data_ptr.is_null() {
                    return Err(format!("GetBuffer failed: 0x{:08X}", hr as u32).into());
                }
            }

            // Write samples
            let total_samples = frames_to_write as usize * samples_per_frame;
            if self.is_float {
                // Write f32 directly
                let dst = std::slice::from_raw_parts_mut(data_ptr as *mut f32, total_samples);
                let src_len = total_samples.min(samples.len());
                dst[..src_len].copy_from_slice(&samples[..src_len]);
                if src_len < total_samples {
                    for s in &mut dst[src_len..] {
                        *s = 0.0;
                    }
                }
            } else {
                // Convert f32 to i16
                let dst = std::slice::from_raw_parts_mut(data_ptr as *mut i16, total_samples);
                for i in 0..total_samples {
                    let val = if i < samples.len() { samples[i] } else { 0.0 };
                    dst[i] = (val.clamp(-1.0, 1.0) * 32767.0) as i16;
                }
            }

            // ReleaseBuffer
            {
                let vtbl = *(self.render_client as *mut *const IAudioRenderClientVtbl);
                let hr = ((*vtbl).ReleaseBuffer)(self.render_client, frames_to_write, 0);
                if hr < 0 {
                    return Err(format!("ReleaseBuffer failed: 0x{:08X}", hr as u32).into());
                }
            }

            Ok(frames_to_write as usize)
        }
    }

    /// Returns the allocated buffer size in frames.
    pub fn buffer_frames(&self) -> u32 {
        self.buffer_size
    }
}

impl Drop for WasapiDevice {
    fn drop(&mut self) {
        unsafe {
            // Stop the audio stream
            let client_vtbl = *(self.client as *mut *const IAudioClientVtbl);
            ((*client_vtbl).Stop)(self.client);

            // Release IAudioRenderClient
            let rc_vtbl = *(self.render_client as *mut *const IAudioRenderClientVtbl);
            ((*rc_vtbl).base.Release)(self.render_client);

            // Release IAudioClient
            ((*client_vtbl).base.Release)(self.client);

            // Uninitialize COM
            CoUninitialize();
        }
    }
}
