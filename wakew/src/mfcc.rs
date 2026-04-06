use libm::{cosf, expf, logf};
use microfft::{Complex32, real::rfft_512};

pub const SAMPLE_RATE: f32 = 16_000.;
// 25ms of audio @ 16kHz
pub const FRAME_SIZE: usize = 400;
pub const NUM_MFCC: usize = 12;

const FFT_RETURN_SIZE: usize = 256;
const FFT_SIZE: usize = FFT_RETURN_SIZE * 2;
const NUM_MEL_FILTERS: usize = 26;

fn to_mel(f: f32) -> f32 {
    1125. * logf(1. + f / 700.)
}

fn to_freq(m: f32) -> f32 {
    700. * (expf(m / 1125.) - 1.)
}

fn get_mel_filters() -> [[f32; FFT_RETURN_SIZE]; NUM_MEL_FILTERS] {
    let low_freq = to_mel(300.);
    let high_freq = to_mel(8000.);
    let step = (high_freq - low_freq) / (NUM_MEL_FILTERS + 1) as f32;
    let freq_points: [f32; NUM_MEL_FILTERS + 2] =
        core::array::from_fn(|i| to_freq(i as f32 * step + low_freq));
    let bins: [usize; NUM_MEL_FILTERS + 2] =
        core::array::from_fn(|i| ((FFT_SIZE + 1) as f32 * freq_points[i] / SAMPLE_RATE) as usize);
    let mut filters = [[0f32; FFT_RETURN_SIZE]; NUM_MEL_FILTERS];
    for i in 1..=NUM_MEL_FILTERS {
        let low = bins[i - 1];
        let mid = bins[i];
        let high = bins[i + 1];
        for j in low..mid {
            filters[i - 1][j] = (j - low) as f32 / (mid - low) as f32;
        }
        for j in mid..high {
            filters[i - 1][j] = (high - j) as f32 / (high - mid) as f32;
        }
    }
    filters
}

fn get_dct_matrix() -> [[f32; NUM_MEL_FILTERS]; NUM_MFCC] {
    let mut dct_matrix = [[0f32; NUM_MEL_FILTERS]; NUM_MFCC];
    for i in 0..NUM_MFCC {
        for j in 0..NUM_MEL_FILTERS {
            dct_matrix[i][j] =
                cosf(core::f32::consts::PI * (j as f32 + 0.5) * i as f32 / NUM_MFCC as f32);
        }
    }
    dct_matrix
}

pub struct Mfcc {
    mel_filters: [[f32; FFT_RETURN_SIZE]; NUM_MEL_FILTERS],
    dct_matrix: [[f32; NUM_MEL_FILTERS]; NUM_MFCC],
}

impl Mfcc {
    pub fn new() -> Self {
        let mel_filters = get_mel_filters();
        let dct_matrix = get_dct_matrix();
        Self {
            mel_filters,
            dct_matrix,
        }
    }
    pub fn mfcc(&self, frame: &[f32; FRAME_SIZE]) -> [f32; NUM_MFCC] {
        let post_fft = fft(frame);
        let post_periodogram = periodogram(&post_fft);
        let post_log_mel_energies = log_mel_energies(&post_periodogram, &self.mel_filters);
        dct(&post_log_mel_energies, &self.dct_matrix)
    }
}

fn fft(frame: &[f32; FRAME_SIZE]) -> [Complex32; FFT_RETURN_SIZE] {
    let mut buffer: [f32; 512] = [0.; 512];
    buffer[..FRAME_SIZE].copy_from_slice(frame);
    // 512-point real FFT returns 256 complex coefficients
    *rfft_512(&mut buffer)
}

fn periodogram(frame_fft: &[Complex32; FFT_RETURN_SIZE]) -> [f32; FFT_RETURN_SIZE] {
    core::array::from_fn(|i| frame_fft[i].norm_sqr() / FRAME_SIZE as f32)
}

fn log_mel_energies(
    periodogram: &[f32; FFT_RETURN_SIZE],
    mel_filters: &[[f32; FFT_RETURN_SIZE]; NUM_MEL_FILTERS],
) -> [f32; NUM_MEL_FILTERS] {
    core::array::from_fn(|i| {
        logf(
            (0..FFT_RETURN_SIZE)
                .map(|j| mel_filters[i][j] * periodogram[j])
                .sum::<f32>()
                + 1e-5f32,
        )
    })
}

fn dct(
    log_mel_energies: &[f32; NUM_MEL_FILTERS],
    dct_matrix: &[[f32; NUM_MEL_FILTERS]; NUM_MFCC],
) -> [f32; NUM_MFCC] {
    core::array::from_fn(|i| {
        (0..NUM_MEL_FILTERS)
            .map(|j| dct_matrix[i][j] * log_mel_energies[j])
            .sum()
    })
}
