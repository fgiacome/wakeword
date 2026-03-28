use core::f32::consts::PI;
use libm::sinf;
use wakeword::mfcc::FRAME_SIZE;
use wakeword::mfcc::Mfcc;
use wakeword::mfcc::NUM_MFCC;
use wakeword::mfcc::SAMPLE_RATE;
use wakeword::similarities::cosine_similarity;

const SHIFT_WIDTH: usize = 100;
const NUM_FRAMES: usize = (SAMPLE_RATE as usize - FRAME_SIZE) / SHIFT_WIDTH + 1;

fn build_1sec_array() -> [f32; SAMPLE_RATE as usize] {
    let mut array = [0.; SAMPLE_RATE as usize];
    (0..SAMPLE_RATE as usize / 3)
        // 3kHz
        .for_each(|i| array[i] = sinf(i as f32 * PI * 2. * 3000. / SAMPLE_RATE as f32));
    (SAMPLE_RATE as usize / 3..2 * SAMPLE_RATE as usize / 3)
        // 6kHz
        .for_each(|i| array[i] = sinf(i as f32 * PI * 2. * 6000. / SAMPLE_RATE as f32));
    (2 * SAMPLE_RATE as usize / 3..SAMPLE_RATE as usize)
        // 0.5kHz
        .for_each(|i| array[i] = sinf(i as f32 * PI * 2. * 500. / SAMPLE_RATE as f32));
    array
}

#[test]
fn test_mfcc_similarity() {
    let array = build_1sec_array();
    let mfcc = Mfcc::new();
    let mut mfcc_matrix = [[0f32; NUM_MFCC]; NUM_FRAMES];
    for i in 0..NUM_FRAMES {
        let mfcc_frame = mfcc.mfcc(
            array[SHIFT_WIDTH * i..(SHIFT_WIDTH * i + FRAME_SIZE)]
                .try_into()
                .unwrap(),
        );
        mfcc_frame
            .iter()
            .enumerate()
            .for_each(|(u, v)| mfcc_matrix[i][u] = *v);
    }
    let third = SAMPLE_RATE as usize / 3 / SHIFT_WIDTH;
    println!("{:?}", mfcc_matrix[0]);
    println!("{:?}", mfcc_matrix[8]);
    assert!((cosine_similarity(&mfcc_matrix[2], &mfcc_matrix[9]) - 1.).abs() < 1e-2);
    println!("{:?}", mfcc_matrix[2 + third]);
    println!("{:?}", mfcc_matrix[9 + third]);
    assert!(
        (cosine_similarity(&mfcc_matrix[2 + third], &mfcc_matrix[9 + third]) - 1.).abs() < 1e-2
    );
    println!("{:?}", mfcc_matrix[2 + 2 * third]);
    println!("{:?}", mfcc_matrix[9 + 2 * third]);
    assert!(
        (cosine_similarity(&mfcc_matrix[2 + 2 * third], &mfcc_matrix[9 + 2 * third]) - 1.).abs()
            < 1e-2
    );
}
