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

fn time_aware_distance<const F: usize>(a: &[f32; F], b: &[f32; F], i: usize, j: usize, max_delay: usize) -> f32 {
    if j.abs_diff(i) > max_delay {
        f32::INFINITY
    }
    else {
        distance(a, b)
    }
}

pub async fn dtw<const N: usize, const M: usize, const F: usize>(
    a: &[[f32; F]; N],
    b: &[[f32; F]; M],
) -> f32 {
    let mut prev = [f32::INFINITY; M];
    let mut curr = [f32::INFINITY; M];

    prev[0] = distance(&a[0], &b[0]);
    for j in 1..M {
        prev[j] = prev[j-1] + time_aware_distance(&a[0], &b[j], 0, j, MAX_DELAY);
    }

    yield_now().await;

    for i in 1..N {
        curr[0] = prev[0] + time_aware_distance(&a[i], &b[0], i, 0, MAX_DELAY);
        for j in 1..M {
            let dist = time_aware_distance(&a[i], &b[j], i, j, MAX_DELAY);
            curr[j] = dist + prev[j].min(curr[j-1]).min(prev[j-1]);
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