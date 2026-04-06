use libm::sqrtf;
pub fn cosine_similarity<const S: usize>(a: &[f32; S], b: &[f32; S]) -> f32 {
    let norm_a = sqrtf(a.iter().map(|v| v * v).sum::<f32>());
    let norm_b = sqrtf(b.iter().map(|v| v * v).sum::<f32>());
    a.iter()
        .zip(b)
        .map(|(a, b)| a * b)
        .map(|v| v / norm_a / norm_b)
        .sum::<f32>()
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
