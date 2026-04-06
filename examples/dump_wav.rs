use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: dump_wav <file.wav> [array_name] [size]");
        std::process::exit(1);
    }

    let path = &args[1];
    let array_name = args.get(2).map(|s| s.as_str()).unwrap_or("REFERENCE");
    let target_size: Option<usize> = args.get(3).and_then(|s| s.parse().ok());

    let mut reader = hound::WavReader::open(path).unwrap();
    let spec = reader.spec();
    assert_eq!(spec.channels, 1, "Only mono supported");
    assert_eq!(spec.bits_per_sample, 16, "Only 16-bit PCM supported");

    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.unwrap() as f32 / 32768.0)
        .collect();

    let size = target_size.unwrap_or(samples.len());
    let mut floats = vec![0f32; size];
    let copy_len = samples.len().min(size);
    floats[..copy_len].copy_from_slice(&samples[..copy_len]);

    println!("// {} — {}Hz, {} samples", path, spec.sample_rate, size);
    println!("pub const {}: [f32; {}] = [", array_name, size);
    for (i, v) in floats.iter().enumerate() {
        let end = if i < size - 1 { "," } else { "" };
        println!("    {:.8e}{}", v, end);
    }
    println!("];");
}
