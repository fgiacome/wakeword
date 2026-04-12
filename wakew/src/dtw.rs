use libm::sqrtf;
use embassy_futures::yield_now;

const MAX_DELAY: usize = 17;

fn cosine_similarity<const S: usize>(a: &[f32; S], b: &[f32; S]) -> f32 {
    let norm_a = sqrtf(a.iter().map(|v| v * v).sum::<f32>()) + 1e-5;
    let norm_b = sqrtf(b.iter().map(|v| v * v).sum::<f32>()) + 1e-5;
    a.iter().enumerate()
        .map(|(i, v)| *v * b[i])
        .map(|v| v / norm_a / norm_b)
        .sum::<f32>()
}


fn distance<const F: usize>(a: &[f32; F], b: &[f32; F]) -> f32 {
    (1.0 - cosine_similarity(a, b))/2.
}

fn in_band(i: usize, j: usize, n: usize, m: usize, max_delay: usize) -> bool {
    // Check if (i, j) is within max_delay steps of the diagonal from (0,0) to
    // (N-1,M-1).  j_diag = i * (M-1) / (N-1); we compare using
    // cross-multiplication to avoid floats.
    let lhs = j * (n - 1);
    let rhs = i * (m - 1);
    lhs.abs_diff(rhs) <= max_delay * (n - 1)
}

pub async fn dtw<const N: usize, const M: usize, const F: usize>(
    a: &[[f32; F]; N],
    b: &[[f32; F]; M],
) -> f32 {
    let mut prev = [f32::INFINITY; M];
    let mut curr = [f32::INFINITY; M];

    prev[0] = distance(&a[0], &b[0]);
    for j in 1..M {
        if in_band(0, j, N, M, MAX_DELAY) {
            prev[j] = prev[j-1] + distance(&a[0], &b[j]);
        }
    }

    yield_now().await;

    for i in 1..N {
        if in_band(i, 0, N, M, MAX_DELAY) {
            curr[0] = prev[0] + distance(&a[i], &b[0]);
        }
        for j in 1..M {
            if in_band(i, j, N, M, MAX_DELAY) {
                let dist = distance(&a[i], &b[j]);
                curr[j] = dist + prev[j].min(curr[j-1]).min(prev[j-1]);
            }
        }
        core::mem::swap(&mut prev, &mut curr);
        curr = [f32::INFINITY; M];
        yield_now().await;
    }

    prev[M-1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_direction() {
        let a: [f32; 2] = [0.1, 0.1];
        let b: [f32; 2] = [0.5, 0.5];
        assert!((cosine_similarity(&a, &b) - 1.).abs() < 1e-3f32);
    }
}