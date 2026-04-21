use std::env;
use std::io::{Write, stderr};
use wakew::mfcc::FEATURE_SIZE;
use wakew::mfcc::FRAME_SIZE;
use wakew::mfcc::Mfcc;
use wakew::mfcc::SHIFT_WIDTH;

const SIZE: usize = 18000;
const NUM_FRAMES: usize = (SIZE - FRAME_SIZE + SHIFT_WIDTH - 1) / SHIFT_WIDTH + 1;

fn load(path: &str, mfcc: &Mfcc) -> [[f32; FEATURE_SIZE]; NUM_FRAMES] {
    let mut reader = hound::WavReader::open(path).unwrap_or_else(|e| {
        eprintln!("Error opening {}: {}", path, e);
        std::process::exit(1);
    });
    let spec = reader.spec();
    assert_eq!(spec.channels, 1, "{}: only mono supported", path);
    assert_eq!(
        spec.bits_per_sample, 16,
        "{}: only 16-bit PCM supported",
        path
    );

    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.unwrap() as f32 / 32768.0)
        .collect();

    let mut floats = [0f32; SIZE];
    let copy_len = samples.len().min(SIZE);
    if SIZE < samples.len() {
        let _ = writeln!(
            stderr(),
            "{}: truncating {} samples",
            path,
            samples.len() - SIZE
        );
    } else if copy_len < SIZE {
        let _ = writeln!(stderr(), "{}: padding {} samples", path, SIZE - copy_len);
    }
    floats[..copy_len].copy_from_slice(&samples[..copy_len]);
    mfcc.seq_mfcc(&floats)
}

fn print_array(name: &str, path: &str, data: &[[f32; FEATURE_SIZE]; NUM_FRAMES]) {
    println!("// {} — {} samples", path, SIZE);
    println!(
        "pub const {}: [[f32; {}]; {}] = [",
        name, FEATURE_SIZE, NUM_FRAMES
    );
    for v in data.iter() {
        print!("    [ ");
        for c in v.iter() {
            print!("{:.8e}, ", c);
        }
        println!("],");
    }
    println!("];");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: dump_wav <file1.wav> [file2.wav ...]");
        std::process::exit(1);
    }

    let paths = &args[1..];
    let mfcc = Mfcc::new();

    let names: Vec<String> = if paths.len() == 1 {
        vec!["REFERENCE".to_string()]
    } else {
        (1..=paths.len())
            .map(|i| format!("REFERENCE_{}", i))
            .collect()
    };

    for (path, name) in paths.iter().zip(names.iter()) {
        let data = load(path, &mfcc);
        print_array(name, path, &data);
    }

    if paths.len() > 1 {
        println!(
            "pub const REFERENCES: [&[[f32; {}]; {}]; {}] = [",
            FEATURE_SIZE,
            NUM_FRAMES,
            paths.len()
        );
        for name in &names {
            println!("    &{},", name);
        }
        println!("];");
    }
}
