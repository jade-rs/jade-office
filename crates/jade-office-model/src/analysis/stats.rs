//! The distributions and sums the Analysis ToolPak's tables are made of:
//! Student's t, F, the normal and chi-squared, by the incomplete beta and
//! gamma functions, and the sample statistics Excel's worksheet functions
//! give.

use std::f64::consts::PI;

/// ln Γ(x), by Lanczos' approximation.
pub fn ln_gamma(x: f64) -> f64 {
    const G: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        return (PI / (PI * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let t = x + 7.5;
    let sum = G[1..]
        .iter()
        .enumerate()
        .fold(G[0], |sum, (i, g)| sum + g / (x + i as f64 + 1.0));
    0.5 * (2.0 * PI).ln() + (x + 0.5) * t.ln() - t + sum.ln()
}

/// The regularised incomplete beta function I_x(a, b).
pub fn beta_regularised(x: f64, a: f64, b: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let front =
        (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln()).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        front * beta_fraction(x, a, b) / a
    } else {
        1.0 - front * beta_fraction(1.0 - x, b, a) / b
    }
}

/// Lentz's continued fraction for the incomplete beta.
fn beta_fraction(x: f64, a: f64, b: f64) -> f64 {
    let tiny = 1e-300;
    let mut c = 1.0;
    let mut d = 1.0 - (a + b) * x / (a + 1.0);
    if d.abs() < tiny {
        d = tiny;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..400 {
        let m = m as f64;
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((a + m2 - 1.0) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < tiny {
            d = tiny;
        }
        c = 1.0 + aa / c;
        if c.abs() < tiny {
            c = tiny;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (a + b + m) * x / ((a + m2) * (a + m2 + 1.0));
        d = 1.0 + aa * d;
        if d.abs() < tiny {
            d = tiny;
        }
        c = 1.0 + aa / c;
        if c.abs() < tiny {
            c = tiny;
        }
        d = 1.0 / d;
        let delta = d * c;
        h *= delta;
        if (delta - 1.0).abs() < 1e-15 {
            break;
        }
    }
    h
}

/// The regularised lower incomplete gamma function P(a, x).
pub fn gamma_regularised(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x < a + 1.0 {
        let mut sum = 1.0 / a;
        let mut term = sum;
        let mut n = a;
        for _ in 0..1000 {
            n += 1.0;
            term *= x / n;
            sum += term;
            if term.abs() < sum.abs() * 1e-16 {
                break;
            }
        }
        sum * (-x + a * x.ln() - ln_gamma(a)).exp()
    } else {
        let tiny = 1e-300;
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / tiny;
        let mut d = 1.0 / b;
        let mut h = d;
        for i in 1..1000 {
            let an = -(i as f64) * (i as f64 - a);
            b += 2.0;
            d = an * d + b;
            if d.abs() < tiny {
                d = tiny;
            }
            c = b + an / c;
            if c.abs() < tiny {
                c = tiny;
            }
            d = 1.0 / d;
            let delta = d * c;
            h *= delta;
            if (delta - 1.0).abs() < 1e-16 {
                break;
            }
        }
        1.0 - (-x + a * x.ln() - ln_gamma(a)).exp() * h
    }
}

/// erf(x), to double precision through the incomplete gamma.
pub fn erf(x: f64) -> f64 {
    let value = gamma_regularised(0.5, x * x);
    if x < 0.0 {
        -value
    } else {
        value
    }
}

/// NORM.S.DIST(z, TRUE).
pub fn normal_cdf(z: f64) -> f64 {
    if z < -8.0 {
        // Kept accurate in the far tail.
        return 0.5 * (1.0 - gamma_regularised(0.5, z * z / 2.0)).max(0.0);
    }
    0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2))
}

/// NORM.S.INV(p).
pub fn normal_inverse(p: f64) -> f64 {
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }
    // Acklam's rational approximation, polished by Newton steps.
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    let low = 0.02425;
    let mut x = if p < low {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= 1.0 - low {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    };
    for _ in 0..3 {
        let density = (-x * x / 2.0).exp() / (2.0 * PI).sqrt();
        if density <= 0.0 {
            break;
        }
        x -= (normal_cdf(x) - p) / density;
    }
    x
}

/// T.DIST(t, df, TRUE).
pub fn t_cdf(t: f64, df: f64) -> f64 {
    let x = df / (df + t * t);
    let tail = 0.5 * beta_regularised(x, df / 2.0, 0.5);
    if t > 0.0 {
        1.0 - tail
    } else {
        tail
    }
}

/// T.INV(p, df).
pub fn t_inverse(p: f64, df: f64) -> f64 {
    invert(|t| t_cdf(t, df), p, -1e3, 1e3)
}

/// F.DIST(f, d1, d2, TRUE).
pub fn f_cdf(f: f64, d1: f64, d2: f64) -> f64 {
    if f <= 0.0 {
        return 0.0;
    }
    beta_regularised(d1 * f / (d1 * f + d2), d1 / 2.0, d2 / 2.0)
}

/// F.INV(p, d1, d2).
pub fn f_inverse(p: f64, d1: f64, d2: f64) -> f64 {
    invert(|f| f_cdf(f, d1, d2), p, 0.0, 1e6)
}

/// CHISQ.DIST(x, df, TRUE).
pub fn chi_squared_cdf(x: f64, df: f64) -> f64 {
    gamma_regularised(df / 2.0, x / 2.0)
}

/// The `x` an increasing `cdf` takes to `p`, by bisection then a few
/// secant steps.
fn invert(cdf: impl Fn(f64) -> f64, p: f64, mut low: f64, mut high: f64) -> f64 {
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    // Widen until the bracket holds `p`.
    while cdf(high) < p && high < 1e12 {
        high *= 2.0;
    }
    for _ in 0..200 {
        let mid = 0.5 * (low + high);
        if cdf(mid) < p {
            low = mid;
        } else {
            high = mid;
        }
        if (high - low).abs() <= 1e-15 * (1.0 + mid.abs()) {
            break;
        }
    }
    0.5 * (low + high)
}

pub fn sum(values: &[f64]) -> f64 {
    values.iter().sum()
}

pub fn mean(values: &[f64]) -> f64 {
    sum(values) / values.len() as f64
}

/// The sum of squared deviations from the mean (DEVSQ).
pub fn devsq(values: &[f64]) -> f64 {
    let m = mean(values);
    values.iter().map(|v| (v - m).powi(2)).sum()
}

/// VAR.S.
pub fn variance(values: &[f64]) -> f64 {
    devsq(values) / (values.len() as f64 - 1.0)
}

/// STDEV.S.
pub fn deviation(values: &[f64]) -> f64 {
    variance(values).sqrt()
}

/// MEDIAN.
pub fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    if n == 0 {
        return f64::NAN;
    }
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        0.5 * (sorted[n / 2 - 1] + sorted[n / 2])
    }
}

/// MODE.SNGL: the value seen most often, the first such; none when every
/// value is seen once.
pub fn mode(values: &[f64]) -> Option<f64> {
    let mut best: Option<(f64, usize)> = None;
    for (i, v) in values.iter().enumerate() {
        let count = values.iter().filter(|w| *w == v).count();
        if count > 1 && best.is_none_or(|(_, most)| count > most) && !values[..i].contains(v) {
            best = Some((*v, count));
        }
    }
    best.map(|(v, _)| v)
}

/// KURT: excess kurtosis, as Excel works it out.
pub fn kurtosis(values: &[f64]) -> f64 {
    let n = values.len() as f64;
    if n < 4.0 {
        return f64::NAN;
    }
    let m = mean(values);
    let s = deviation(values);
    let fourth: f64 = values.iter().map(|v| ((v - m) / s).powi(4)).sum();
    n * (n + 1.0) / ((n - 1.0) * (n - 2.0) * (n - 3.0)) * fourth
        - 3.0 * (n - 1.0).powi(2) / ((n - 2.0) * (n - 3.0))
}

/// SKEW.
pub fn skewness(values: &[f64]) -> f64 {
    let n = values.len() as f64;
    if n < 3.0 {
        return f64::NAN;
    }
    let m = mean(values);
    let s = deviation(values);
    let third: f64 = values.iter().map(|v| ((v - m) / s).powi(3)).sum();
    n / ((n - 1.0) * (n - 2.0)) * third
}

/// PEARSON.
pub fn correlation(a: &[f64], b: &[f64]) -> f64 {
    let (ma, mb) = (mean(a), mean(b));
    let cross: f64 = a.iter().zip(b).map(|(x, y)| (x - ma) * (y - mb)).sum();
    cross / (devsq(a) * devsq(b)).sqrt()
}

/// COVARIANCE.P.
pub fn covariance(a: &[f64], b: &[f64]) -> f64 {
    let (ma, mb) = (mean(a), mean(b));
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - ma) * (y - mb))
        .sum::<f64>()
        / a.len() as f64
}

/// The `k`th largest (LARGE) or, with `smallest`, smallest (SMALL).
pub fn kth(values: &[f64], k: usize, smallest: bool) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    if smallest {
        sorted.get(k.wrapping_sub(1)).copied().unwrap_or(f64::NAN)
    } else {
        sorted
            .len()
            .checked_sub(k)
            .and_then(|at| sorted.get(at))
            .copied()
            .unwrap_or(f64::NAN)
    }
}

/// Solves least squares `X b = y` by the normal equations: the
/// coefficients and (XᵀX)⁻¹.
pub fn least_squares(x: &[Vec<f64>], y: &[f64]) -> Option<(Vec<f64>, Vec<Vec<f64>>)> {
    let k = x.first()?.len();
    let mut xtx = vec![vec![0.0; k]; k];
    let mut xty = vec![0.0; k];
    for (row, target) in x.iter().zip(y) {
        for i in 0..k {
            xty[i] += row[i] * target;
            for j in 0..k {
                xtx[i][j] += row[i] * row[j];
            }
        }
    }
    let inverse = invert_matrix(&xtx)?;
    let coefficients = (0..k)
        .map(|i| (0..k).map(|j| inverse[i][j] * xty[j]).sum())
        .collect();
    Some((coefficients, inverse))
}

/// A square matrix's inverse by Gauss–Jordan elimination with partial
/// pivoting; none when it is singular.
pub fn invert_matrix(matrix: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = matrix.len();
    let mut a: Vec<Vec<f64>> = matrix
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut row = row.clone();
            row.extend((0..n).map(|j| if i == j { 1.0 } else { 0.0 }));
            row
        })
        .collect();
    let scale = matrix
        .iter()
        .flatten()
        .fold(0.0f64, |most, v| most.max(v.abs()))
        .max(1e-300);
    for column in 0..n {
        let pivot =
            (column..n).max_by(|&i, &j| a[i][column].abs().total_cmp(&a[j][column].abs()))?;
        if a[pivot][column].abs() <= 1e-13 * scale {
            return None;
        }
        a.swap(column, pivot);
        let value = a[column][column];
        for v in a[column].iter_mut() {
            *v /= value;
        }
        let pivot_row = a[column].clone();
        for (i, row) in a.iter_mut().enumerate() {
            if i != column {
                let factor = row[column];
                if factor != 0.0 {
                    for (v, p) in row.iter_mut().zip(&pivot_row) {
                        *v -= factor * p;
                    }
                }
            }
        }
    }
    Some(a.into_iter().map(|row| row[n..].to_vec()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tolerance: f64) {
        assert!((a - b).abs() <= tolerance, "{a} is not {b}");
    }

    #[test]
    fn distributions_match_excel() {
        // T.DIST.2T(2.0, 10) = 0.073388; T.INV.2T(0.05, 10) = 2.228139.
        close(2.0 * (1.0 - t_cdf(2.0, 10.0)), 0.073_388_034, 1e-8);
        close(t_inverse(0.975, 10.0), 2.228_138_852, 1e-8);
        // F.INV.RT(0.05, 2, 12) = 3.885294; F.DIST.RT(4, 2, 12) = 0.046656.
        close(f_inverse(0.95, 2.0, 12.0), 3.885_293_835, 1e-7);
        close(1.0 - f_cdf(4.0, 2.0, 12.0), 0.046_656, 1e-9);
        // NORM.S.INV(0.975) = 1.959964; NORM.S.DIST(1, TRUE) = 0.841345.
        close(normal_inverse(0.975), 1.959_963_985, 1e-9);
        close(normal_cdf(1.0), 0.841_344_746, 1e-9);
        close(chi_squared_cdf(3.84, 1.0), 0.949_956_5, 1e-6);
    }

    #[test]
    fn sample_statistics_match_excel() {
        let values = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        close(mean(&values), 5.0, 1e-12);
        close(variance(&values), 32.0 / 7.0, 1e-12);
        close(median(&values), 4.5, 1e-12);
        assert_eq!(mode(&values), Some(4.0));
        // KURT and SKEW of these as Excel gives them.
        close(kurtosis(&values), 0.940_625, 1e-6);
        close(skewness(&values), 0.818_487, 1e-6);
        close(kth(&values, 2, false), 7.0, 0.0);
        close(kth(&values, 1, true), 2.0, 0.0);
    }
}
