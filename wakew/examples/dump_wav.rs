use std::env;
use std::io::{Write, stderr};
use wakew::mfcc::FEATURE_SIZE;
use wakew::mfcc::FRAME_SIZE;
use wakew::mfcc::Mfcc;
use wakew::mfcc::SHIFT_WIDTH;

const SIZE: usize = 15000;
const NUM_FRAMES: usize = (SIZE - FRAME_SIZE + SHIFT_WIDTH - 1) / SHIFT_WIDTH;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: dump_wav <file.wav> [array_name]");
        std::process::exit(1);
    }

    let path = &args[1];
    let array_name = args.get(2).map(|s| s.as_str()).unwrap_or("REFERENCE");

    let mut reader = hound::WavReader::open(path).unwrap();
    let spec = reader.spec();
    assert_eq!(spec.channels, 1, "Only mono supported");
    assert_eq!(spec.bits_per_sample, 16, "Only 16-bit PCM supported");

    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.unwrap() as f32 / 32768.0)
        .collect();

    // Allocate buffer and pad / truncate if necessary
    let mut floats = [0f32; SIZE];
    let copy_len = samples.len().min(SIZE);
    if SIZE < samples.len() {
        let _ = writeln!(stderr(), "Truncating {} samples", samples.len() - SIZE);
    }
    if SIZE >= samples.len() {
        let _ = writeln!(stderr(), "Padding {} samples", SIZE - samples.len());
    }
    floats[..copy_len].copy_from_slice(&samples[..copy_len]);

    // Calculate MFCCs
    let mfcc = Mfcc::new();
    let mfcc_result: [[f32; _]; NUM_FRAMES] = mfcc.seq_mfcc(&floats);

    // Print rust tensor
    println!("// {} — {}Hz, {} samples", path, spec.sample_rate, SIZE);
    println!("pub const {}: [[f32; {}]; {}] = [", array_name, FEATURE_SIZE, NUM_FRAMES);
    for v in mfcc_result.iter() {
        print!("    [ ");
        for c in v.iter() {
            print!("{:.8e}, ", {c})
        }
        println!("],");
    }
    println!("];");
}
