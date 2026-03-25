use voltex_audio::{AudioClip, AudioSystem};

fn generate_sine_clip(freq: f32, duration: f32, sample_rate: u32) -> AudioClip {
    let num_samples = (sample_rate as f32 * duration) as usize;
    let mut samples = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        samples.push((t * freq * 2.0 * std::f32::consts::PI).sin() * 0.3);
    }
    AudioClip::new(samples, sample_rate, 1)
}

fn main() {
    println!("=== Voltex Audio Demo ===");
    println!("Generating 440Hz sine wave (2 seconds)...");

    let clip = generate_sine_clip(440.0, 2.0, 44100);
    let clip2 = generate_sine_clip(660.0, 1.5, 44100);

    println!("Initializing audio system...");
    let audio = match AudioSystem::new(vec![clip, clip2]) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Failed to init audio: {}", e);
            return;
        }
    };

    println!("Playing 440Hz tone...");
    audio.play(0, 0.5, false);
    std::thread::sleep(std::time::Duration::from_secs(1));

    println!("Playing 660Hz tone on top...");
    audio.play(1, 0.3, false);
    std::thread::sleep(std::time::Duration::from_secs(2));

    println!("Done!");
}
