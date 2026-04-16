//! The workflow to calculate MFCCs for a `FRAME_SIZE`-sample frame is the
//! following:
//! 
//! 1. `FFT_SIZE`-point FFT of `FRAME_SIZE` samples
//! 1. First `FFT_RETURN_SIZE` FFT coefficients are kept
//! 1. `FFT_RETURN_SIZE` FFT coefficients are passed through the Mel filterbank,
//! producing `NUM_MEL_FILTERS` coefficients as a result.
//! 1. DCT is applied to `NUM_MEL_FILTERS` and the first `NUM_MFCC` coefficients
//! are kept; these are MFCCs for the current frame.

use embassy_futures::yield_now;
use libm::{cosf, expf, logf};
use microfft::{Complex32, real::rfft_512};

/// Expected audio sample rate
pub const SAMPLE_RATE: f32 = 16_000.;
/// 25ms of audio @ 16kHz
/// 
/// We calculate MFCCs for 400 samples
pub const FRAME_SIZE: usize = 400;
/// Number of MFCCs for each `FRAME_SIZE` samples
pub const NUM_MFCC: usize = 12;
/// Feature vector size per frame: base MFCCs + delta
pub const FEATURE_SIZE: usize = NUM_MFCC * 2;
/// How many samples to shift for each MFCC calculation
pub const SHIFT_WIDTH: usize = 200;

/// Number of kept FFT coeffiecients
const FFT_RETURN_SIZE: usize = 256;
/// Point size of FFT
const FFT_SIZE: usize = FFT_RETURN_SIZE * 2;
/// Number of Mel filters
///
/// Input is `FFT_RETURN_SIZE` FFT coefficients and output is one scalar for
/// each filter
const NUM_MEL_FILTERS: usize = 26;
/// Half-window size for delta computation (uses frames i-N..=i+N)
const DELTA_N: usize = 2;
/// Denominator for delta computation: 2 * sum(n^2 for n in 1..=DELTA_N)
const DELTA_DENOM: f32 = 2.0 * (1 + 4) as f32; // N=2: 2*(1+4)=10

/// Hz to Mel
fn to_mel(f: f32) -> f32 {
    1125. * logf(1. + f / 700.)
}

/// Mel to Hz
fn to_freq(m: f32) -> f32 {
    700. * (expf(m / 1125.) - 1.)
}

fn get_mel_filters() -> [[f32; FFT_RETURN_SIZE]; NUM_MEL_FILTERS] {
    // Memory: 26*256*4 = 26.6KB
    let mut filters = [[0f32; FFT_RETURN_SIZE]; NUM_MEL_FILTERS];
    let low_freq = to_mel(300.);
    let high_freq = to_mel(8000.);
    let step = (high_freq - low_freq) / (NUM_MEL_FILTERS + 1) as f32;
    let freq_points: [f32; NUM_MEL_FILTERS + 2] =
        core::array::from_fn(|i| to_freq(i as f32 * step + low_freq));
    let bins: [usize; NUM_MEL_FILTERS + 2] =
        core::array::from_fn(|i| ((FFT_SIZE + 1) as f32 * freq_points[i] / SAMPLE_RATE) as usize);
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

fn get_hamming_window() -> [f32; FRAME_SIZE] {
    // Memory: 400*4 = 1.6KB
    core::array::from_fn(|n| {
        0.54 - 0.46 * cosf(2.0 * core::f32::consts::PI * n as f32 / (FRAME_SIZE - 1) as f32)
    })
}

fn get_dct_matrix() -> [[f32; NUM_MEL_FILTERS]; NUM_MFCC] {
    // Memory: 12*26*4 = 1.2KB
    // Coefficients 1..=NUM_MFCC (C0 is omitted: it is proportional to log frame
    // energy and carries no phonetic information beyond what cosine similarity
    // already discards).
    core::array::from_fn(|i| {
        core::array::from_fn(|j| {
            cosf(core::f32::consts::PI * (j as f32 + 0.5) * (i + 1) as f32 / NUM_MEL_FILTERS as f32)
        })
    })
}

pub struct Mfcc {
    mel_filters: [[f32; FFT_RETURN_SIZE]; NUM_MEL_FILTERS],
    dct_matrix: [[f32; NUM_MEL_FILTERS]; NUM_MFCC],
    hamming_window: [f32; FRAME_SIZE],
}

impl Mfcc {
    pub fn new() -> Self {
        let mel_filters = get_mel_filters();
        let dct_matrix = get_dct_matrix();
        let hamming_window = get_hamming_window();
        Self {
            mel_filters,
            dct_matrix,
            hamming_window,
        }
    }
    pub fn mfcc(&self, frame: &[f32; FRAME_SIZE]) -> [f32; NUM_MFCC] {
        let dc_free = remove_dc(frame);
        let windowed = apply_hamming(&dc_free, &self.hamming_window);
        let post_fft = fft(&windowed);
        let post_periodogram = periodogram(&post_fft);
        let post_log_mel_energies = log_mel_energies(&post_periodogram, &self.mel_filters);
        dct(&post_log_mel_energies, &self.dct_matrix)
    }

    pub fn seq_mfcc<const S: usize, const N: usize>(&self, seq: &[f32; S]) -> [[f32; FEATURE_SIZE]; N] {
        assert_eq!((S - FRAME_SIZE + SHIFT_WIDTH - 1) / SHIFT_WIDTH, N);
        let base: [[f32; NUM_MFCC]; N] = core::array::from_fn(|i| {
            let mut frame = [0f32; FRAME_SIZE];
            let chunk = &seq[i * SHIFT_WIDTH..];
            let len = chunk.len().min(FRAME_SIZE);
            frame[..len].copy_from_slice(&chunk[..len]);
            self.mfcc(&frame)
        });
        core::array::from_fn(|i| {
            let mut feat = [0f32; FEATURE_SIZE];
            feat[..NUM_MFCC].copy_from_slice(&base[i]);
            feat[NUM_MFCC..].copy_from_slice(&compute_delta_frame(&base, i));
            feat
        })
    }
}

fn remove_dc(frame: &[f32; FRAME_SIZE]) -> [f32; FRAME_SIZE] {
    let mean = frame.iter().sum::<f32>() / FRAME_SIZE as f32;
    core::array::from_fn(|i| frame[i] - mean)
}

fn apply_hamming(frame: &[f32; FRAME_SIZE], window: &[f32; FRAME_SIZE]) -> [f32; FRAME_SIZE] {
    core::array::from_fn(|i| frame[i] * window[i])
}

fn fft(frame: &[f32; FRAME_SIZE]) -> [Complex32; FFT_RETURN_SIZE] {
    let mut buffer: [f32; 512] = [0f32; 512];
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

/// Expand a sequence of base MFCCs into [base | delta] feature vectors, writing
/// into a caller-provided buffer to avoid a large internal allocation.
pub async fn window_to_features_into<const N: usize>(
    base: &[[f32; NUM_MFCC]; N],
    out: &mut [[f32; FEATURE_SIZE]; N],
) {
    for i in 0..N {
        out[i][..NUM_MFCC].copy_from_slice(&base[i]);
        out[i][NUM_MFCC..].copy_from_slice(&compute_delta_frame(base, i));
        yield_now().await;
    }
}

fn compute_delta_frame<const N: usize>(frames: &[[f32; NUM_MFCC]; N], i: usize) -> [f32; NUM_MFCC] {
    core::array::from_fn(|k| {
        (1..=DELTA_N).map(|n| {
            let prev = frames[i.saturating_sub(n)][k];
            let next = frames[(i + n).min(N - 1)][k];
            n as f32 * (next - prev)
        })
        .sum::<f32>() / DELTA_DENOM
    })
}
