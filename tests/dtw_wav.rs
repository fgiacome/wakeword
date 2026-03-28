use wakeword::mfcc::{Mfcc, FRAME_SIZE, SAMPLE_RATE, NUM_MFCC};
use wakeword::dtw::dtw;

const WINDOW_SIZE: usize = 16800;
const SHIFT_WIDTH: usize = 100;
const NUM_FRAMES: usize = (WINDOW_SIZE - FRAME_SIZE) / SHIFT_WIDTH + 1;

fn load_wav(path: &str) -> [f32; WINDOW_SIZE] {
    let mut reader = hound::WavReader::open(path).unwrap();
    let spec = reader.spec();
    assert_eq!(spec.sample_rate, SAMPLE_RATE as u32);

    let mut array = [0f32; WINDOW_SIZE];
    for (i, sample) in reader.samples::<i16>().enumerate().take(WINDOW_SIZE) {
        array[i] = sample.unwrap() as f32 / 32768.0;
    }
    array
}

fn compute_mfcc_matrix(array: &[f32; WINDOW_SIZE]) -> [[f32; NUM_MFCC]; NUM_FRAMES] {
    let mfcc = Mfcc::new();
    let mut matrix = [[0f32; NUM_MFCC]; NUM_FRAMES];
    for i in 0..NUM_FRAMES {
        let frame = array[SHIFT_WIDTH*i..(SHIFT_WIDTH*i+FRAME_SIZE)].try_into().unwrap();
        let mfcc_frame = mfcc.mfcc(frame);
        mfcc_frame.iter().enumerate().for_each(|(j, v)| matrix[i][j] = *v);
    }
    matrix
}

#[test]
fn dtw_wav() {
    let reference = load_wav("assets/reference.wav");
    let sample_detect = load_wav("assets/sample_detect.wav");
    let sample_detect_b = load_wav("assets/sample_detect_b.wav");
    let sample_none = load_wav("assets/sample_none.wav");

    let reference_mfcc = compute_mfcc_matrix(&reference);
    let detect_mfcc = compute_mfcc_matrix(&sample_detect);
    let detect_b_mfcc = compute_mfcc_matrix(&sample_detect_b);
    let none_mfcc = compute_mfcc_matrix(&sample_none);

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
