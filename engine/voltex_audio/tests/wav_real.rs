//! WAV variants that real tools write. 8/32-bit PCM come from CPython's
//! `wave` module; the LIST-chunk and WAVE_FORMAT_EXTENSIBLE files are built
//! by hand from the RIFF/WAVE layout (Audacity and most DAWs write 24-bit and
//! multichannel audio as EXTENSIBLE). Sample ramps are known integers, so the
//! expected floats are exact.

use voltex_audio::parse_wav;

const N: usize = 16;

fn samples(file: &[u8]) -> Vec<f32> {
    let clip = parse_wav(file).expect("decodes");
    assert_eq!(clip.sample_rate, 8000);
    assert_eq!(clip.channels, 1);
    clip.samples.clone()
}

fn assert_close(got: &[f32], want: &[f32]) {
    assert_eq!(got.len(), want.len());
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert!((g - w).abs() < 1e-6, "sample {i}: got {g} want {w}");
    }
}

#[test]
fn pcm_8bit_is_unsigned_centred_on_128() {
    let want: Vec<f32> = (0..N).map(|i| ((i * 17) as u8 as f32 - 128.0) / 128.0).collect();
    assert_close(&samples(include_bytes!("fixtures/wav_pcm8.wav")), &want);
}

#[test]
fn pcm_32bit_integer_decodes() {
    let want: Vec<f32> = (0..N).map(|i| (i as f32 - 8.0) * 268_435_456.0 / 2_147_483_648.0).collect();
    assert_close(&samples(include_bytes!("fixtures/wav_pcm32.wav")), &want);
}

#[test]
fn odd_sized_chunk_before_data_is_skipped_with_padding() {
    let want: Vec<f32> = (0..N).map(|i| (i as f32 - 8.0) * 4096.0 / 32768.0).collect();
    assert_close(&samples(include_bytes!("fixtures/wav_list_odd_chunk.wav")), &want);
}

#[test]
fn extensible_pcm_24bit_decodes() {
    let want: Vec<f32> = (0..N).map(|i| (i as f32 - 8.0) * 524_288.0 / 8_388_608.0).collect();
    assert_close(&samples(include_bytes!("fixtures/wav_ext_pcm24.wav")), &want);
}

#[test]
fn extensible_float_decodes() {
    let want: Vec<f32> = (0..N).map(|i| (i as f32 - 8.0) / 8.0).collect();
    assert_close(&samples(include_bytes!("fixtures/wav_ext_float.wav")), &want);
}
