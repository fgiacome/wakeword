use crate::similarities::cosine_similarity;

fn distance<const F: usize>(a: &[f32; F], b: &[f32; F]) -> f32 {
    (1.0 - cosine_similarity(a, b))/2.
}

pub fn dtw<const N: usize, const M: usize, const F: usize>(
    a: &[[f32; F]; N],
    b: &[[f32; F]; M],
) -> f32 {
    let mut prev = [f32::INFINITY; M];
    let mut curr = [f32::INFINITY; M];

    prev[0] = distance(&a[0], &b[0]);
    for j in 1..M {
        prev[j] = prev[j-1] + distance(&a[0], &b[j]);
    }

    for i in 1..N {
        curr[0] = prev[0] + distance(&a[i], &b[0]);
        for j in 1..M {
            let dist = distance(&a[i], &b[j]);
            curr[j] = dist + prev[j].min(curr[j-1]).min(prev[j-1]);
        }
        core::mem::swap(&mut prev, &mut curr);
        curr = [f32::INFINITY; M];
    }

    prev[M-1]
}
