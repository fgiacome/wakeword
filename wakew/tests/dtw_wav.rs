use wakew::mfcc::{ FRAME_SIZE, Mfcc, NUM_MFCC, SAMPLE_RATE, SHIFT_WIDTH};
use wakew::dtw::dtw;

const SIZE: usize = 24000;
const NUM_FRAMES: usize = (SIZE - FRAME_SIZE + SHIFT_WIDTH - 1) / SHIFT_WIDTH;

fn load_wav(path: &str) -> [f32; SIZE] {
    let mut reader = hound::WavReader::open(path).unwrap();
    let spec = reader.spec();
    assert_eq!(spec.sample_rate, SAMPLE_RATE as u32);

    let mut array = [0f32; SIZE];
    for (i, sample) in reader.samples::<i16>().enumerate().take(SIZE) {
        array[i] = sample.unwrap() as f32 / 32768.0;
    }
    array
}

#[test]
fn dtw_wav() {
    let mfcc = Mfcc::new();
    let reference = load_wav("../assets/reference.wav");
    let sample_detect = load_wav("../assets/sample_detect.wav");
    let sample_detect_b = load_wav("../assets/sample_detect_b.wav");
    let sample_none = load_wav("../assets/sample_none.wav");

    let reference_mfcc: [[f32; _]; NUM_FRAMES] = mfcc.seq_mfcc(&reference);
    let detect_mfcc: [[f32; _]; NUM_FRAMES]= mfcc.seq_mfcc(&sample_detect);
    let detect_b_mfcc: [[f32; _]; NUM_FRAMES] = mfcc.seq_mfcc(&sample_detect_b);
    let none_mfcc: [[f32; _]; NUM_FRAMES] = mfcc.seq_mfcc(&sample_none);

    let score_detect = dtw(&reference_mfcc, &detect_mfcc);
    let score_detect_b = dtw(&reference_mfcc, &detect_b_mfcc);
    let score_none = dtw(&reference_mfcc, &none_mfcc);
    let score_reference = dtw(&reference_mfcc, &reference_mfcc);

    println!("DTW score (detect): {}", score_detect);
    println!("DTW score (detect_b): {}", score_detect_b);
    println!("DTW score (none):   {}", score_none);
    println!("DTW score (reference):   {}", score_reference);

    assert!(score_detect < score_none);
    assert!(score_detect_b < score_none);
    assert!(score_reference < 1e-3);
}
