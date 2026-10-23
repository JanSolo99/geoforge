//! Ordinary Least Squares (Spatial Statistics › Modeling Spatial Relationships) and the
//! distributions its tests need.
//!
//! Matched to ArcGIS Pro 3.7 (`testdata/arcgis-reference/spatstat_ols.json`, probe in
//! `scripts/research/spatstat/weights/probe_ols.py`): coefficients by least squares, standard
//! errors from `sigma^2 (X'X)^-1` with `sigma^2 = RSS / (n - k)`, robust standard errors White's
//! HC0, the Wald statistic from them, Koenker's Breusch-Pagan statistic `n R^2` of the squared
//! residuals on the explanatory variables, Jarque-Bera from the residuals' population moments,
//! `AIC = -2 lnL + 2 (k + 1)` and `AICc = AIC + 2 (k+1)(k+2) / (n - k - 2)` (`k` counts the
//! intercept), standardized coefficients `b sd(x) / sd(y)` and standardized residuals
//! `e / sigma`. ArcGIS's p-values come from approximations of the t, F and chi-squared tails;
//! ours are exact (a few 1e-6 apart at most).

use unigis_core::{Error, Feature, FeatureClass, Field, FieldType, GeometryKind, Result, Value};

use crate::{TableOutput, ToolOutput};

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

/// `ln Gamma(x)` (Lanczos, g = 7).
fn ln_gamma(x: f64) -> f64 {
    const C: [f64; 9] = [
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
        let pi = std::f64::consts::PI;
        return (pi / (pi * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = C[0];
    let t = x + 7.5;
    for (i, c) in C.iter().enumerate().skip(1) {
        a += c / (x + i as f64);
    }
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// The continued fraction of the regularized incomplete beta function (Lentz).
fn beta_cf(a: f64, b: f64, x: f64) -> f64 {
    let tiny = 1e-300;
    let (qab, qap, qam) = (a + b, a + 1.0, a - 1.0);
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < tiny {
        d = tiny;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..1000 {
        let m = m as f64;
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
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
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < tiny {
            d = tiny;
        }
        c = 1.0 + aa / c;
        if c.abs() < tiny {
            c = tiny;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-16 {
            break;
        }
    }
    h
}

/// The regularized incomplete beta function `I_x(a, b)`.
fn inc_beta(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let lbt = ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln();
    if x < (a + 1.0) / (a + b + 2.0) {
        lbt.exp() * beta_cf(a, b, x) / a
    } else {
        1.0 - lbt.exp() * beta_cf(b, a, 1.0 - x) / b
    }
}

/// The upper regularized incomplete gamma function `Q(a, x)`.
fn gamma_q(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 1.0;
    }
    let lg = ln_gamma(a);
    if x < a + 1.0 {
        let (mut ap, mut sum) = (a, 1.0 / a);
        let mut del = sum;
        for _ in 0..10000 {
            ap += 1.0;
            del *= x / ap;
            sum += del;
            if del.abs() < sum.abs() * 1e-17 {
                break;
            }
        }
        1.0 - sum * (-x + a * x.ln() - lg).exp()
    } else {
        let tiny = 1e-300;
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / tiny;
        let mut d = 1.0 / b;
        let mut h = d;
        for i in 1..10000 {
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
            let del = d * c;
            h *= del;
            if (del - 1.0).abs() < 1e-16 {
                break;
            }
        }
        (-x + a * x.ln() - lg).exp() * h
    }
}

/// Two-sided p-value of Student's t with `df` degrees of freedom.
pub fn t_two_sided(t: f64, df: f64) -> f64 {
    inc_beta(df / 2.0, 0.5, df / (df + t * t))
}

/// Upper tail of the F distribution.
pub fn f_upper(f: f64, d1: f64, d2: f64) -> f64 {
    if f <= 0.0 {
        return 1.0;
    }
    inc_beta(d2 / 2.0, d1 / 2.0, d2 / (d2 + d1 * f))
}

/// Upper tail of the chi-squared distribution.
pub fn chi2_upper(x: f64, df: f64) -> f64 {
    gamma_q(df / 2.0, x / 2.0)
}

/// Solves the symmetric positive system by Gauss-Jordan elimination, returning the inverse.
fn invert(m: &[Vec<f64>]) -> Result<Vec<Vec<f64>>> {
    let n = m.len();
    let mut a: Vec<Vec<f64>> = m
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut r = r.clone();
            r.extend((0..n).map(|j| if i == j { 1.0 } else { 0.0 }));
            r
        })
        .collect();
    for c in 0..n {
        let p = (c..n)
            .max_by(|x, y| a[*x][c].abs().total_cmp(&a[*y][c].abs()))
            .unwrap();
        if a[p][c].abs() < 1e-12 * a[p][c].abs().max(1.0) || a[p][c] == 0.0 {
            return Err(err(
                "ERROR 000880: the explanatory variables are perfectly multicollinear",
            ));
        }
        a.swap(c, p);
        let piv = a[c][c];
        for v in a[c].iter_mut() {
            *v /= piv;
        }
        for r in 0..n {
            if r != c {
                let f = a[r][c];
                if f != 0.0 {
                    let pivot_row = a[c].clone();
                    for (x, p) in a[r].iter_mut().zip(&pivot_row) {
                        *x -= f * p;
                    }
                }
            }
        }
    }
    Ok(a.into_iter().map(|r| r[n..].to_vec()).collect())
}

/// One least-squares fit: coefficients and residuals.
struct Fit {
    b: Vec<f64>,
    e: Vec<f64>,
    xtx_inv: Vec<Vec<f64>>,
}

fn fit(x: &[Vec<f64>], y: &[f64]) -> Result<Fit> {
    let k = x[0].len();
    let mut xtx = vec![vec![0.0; k]; k];
    let mut xty = vec![0.0; k];
    for (row, yi) in x.iter().zip(y) {
        for a in 0..k {
            xty[a] += row[a] * yi;
            for b in 0..k {
                xtx[a][b] += row[a] * row[b];
            }
        }
    }
    let inv = invert(&xtx)?;
    let b: Vec<f64> = (0..k)
        .map(|a| (0..k).map(|c| inv[a][c] * xty[c]).sum())
        .collect();
    let e = x
        .iter()
        .zip(y)
        .map(|(row, yi)| yi - row.iter().zip(&b).map(|(p, q)| p * q).sum::<f64>())
        .collect();
    Ok(Fit { b, e, xtx_inv: inv })
}

/// The outputs of Ordinary Least Squares.
pub struct Ols {
    pub features: ToolOutput,
    pub coefficients: TableOutput,
    pub diagnostics: TableOutput,
    pub messages: Vec<String>,
}

fn number(v: &Value) -> Option<f64> {
    Some(match v {
        Value::SmallInteger(v) => *v as f64,
        Value::Integer(v) => *v as f64,
        Value::BigInteger(v) => *v as f64,
        Value::Single(v) => *v as f64,
        Value::Double(v) => *v,
        _ => return None,
    })
}

const DEFINITIONS: [(&str, &str); 13] = [
    (
        "AIC",
        "Akaike's Information Criterion: A relative measure of performance used to compare models; the smaller AIC indicates the superior model.",
    ),
    (
        "AICc",
        "Corrected Akaike's Information Criterion: second order correction for small sample sizes.",
    ),
    (
        "R2",
        "R-Squared, Coefficient of Determination: The proportion of variation in the dependent variable that is explained by the model.",
    ),
    (
        "AdjR2",
        "Adjusted R-Squared: R-Squared adjusted for model complexity (number of variables) as it relates to the data.",
    ),
    (
        "F-Stat",
        "Joint F-Statistic Value: Used to assess overall model significance.",
    ),
    (
        "F-Prob",
        "Joint F-Statistic Probability (p-value): The probability that none of the explanatory variables have an effect on the dependent variable.",
    ),
    (
        "Wald",
        "Wald Statistic: Used to assess overall robust model significance.",
    ),
    (
        "Wald-Prob",
        "Wald Statistic Probability (p-value): The computed probability, using robust standard errors, that none of the explanatory variables have an effect on the dependent variable.",
    ),
    (
        "K(BP)",
        "Koenker's studentized Breusch-Pagan Statistic: Used to test the reliability of standard error values when heteroskedasticity (non-constant variance) is present.",
    ),
    (
        "K(BP)-Prob",
        "Koenker (BP) Statistic Probability (p-value): The probability that heteroskedasticity (non-constant variance) has not made standard errors unreliable.",
    ),
    (
        "JB",
        "Jarque-Bera Statistic: Used to determine whether the residuals deviate from a normal distribution.",
    ),
    (
        "JB-Prob",
        "Jarque-Bera Probability (p-value): The probability that the residuals are normally distributed.",
    ),
    (
        "Sigma2",
        "Sigma-Squared: OLS estimate of the variance of the error term (residuals).",
    ),
];

/// Ordinary Least Squares of `dependent` on `explanatory` (with an intercept). `dbase` adds
/// ArcGIS's empty `Field1` to the two tables, as dBASE tables have it.
pub fn ordinary_least_squares(
    input: &dyn FeatureClass,
    unique_id: &str,
    dependent: &str,
    explanatory: &[String],
    dbase: bool,
) -> Result<Ols> {
    let find = |name: &str| -> Result<usize> {
        input
            .fields()
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                err(format!(
                    "ERROR 000728: Field {name} does not exist within table"
                ))
            })
    };
    if explanatory.is_empty() {
        return Err(err(
            "ERROR 000735: Explanatory Variables: value is required",
        ));
    }
    let iu = find(unique_id)?;
    let iy = find(dependent)?;
    let ix: Vec<usize> = explanatory.iter().map(|n| find(n)).collect::<Result<_>>()?;
    for i in std::iter::once(iy).chain(ix.iter().copied()) {
        let t = input.fields()[i].field_type;
        if !matches!(
            t,
            FieldType::SmallInteger
                | FieldType::Integer
                | FieldType::BigInteger
                | FieldType::Single
                | FieldType::Double
        ) {
            return Err(err(format!(
                "ERROR 000989: {} must be numeric",
                input.fields()[i].name
            )));
        }
    }
    let mut feats = Vec::new();
    let mut x: Vec<Vec<f64>> = Vec::new();
    let mut y: Vec<f64> = Vec::new();
    for f in input.features() {
        let f = f?;
        if f.geometry.is_none() {
            continue;
        }
        let yv = f.attributes.get(iy).and_then(number);
        let xv: Option<Vec<f64>> = ix
            .iter()
            .map(|i| f.attributes.get(*i).and_then(number))
            .collect();
        let (Some(yv), Some(xv)) = (yv, xv) else {
            continue;
        };
        let mut row = vec![1.0];
        row.extend(xv);
        x.push(row);
        y.push(yv);
        feats.push(f);
    }
    let n = y.len();
    let k = explanatory.len() + 1;
    if n <= k + 2 {
        return Err(err(format!(
            "ERROR 000874: {n} observations are too few for {k} coefficients"
        )));
    }
    let nf = n as f64;
    let kf = k as f64;
    let ft = fit(&x, &y)?;
    let rss: f64 = ft.e.iter().map(|v| v * v).sum();
    let sigma2 = rss / (nf - kf);
    let se: Vec<f64> = (0..k).map(|a| (ft.xtx_inv[a][a] * sigma2).sqrt()).collect();
    // White's HC0: (X'X)^-1 X' diag(e^2) X (X'X)^-1.
    let mut meat = vec![vec![0.0; k]; k];
    for (row, e) in x.iter().zip(&ft.e) {
        for a in 0..k {
            for b in 0..k {
                meat[a][b] += row[a] * row[b] * e * e;
            }
        }
    }
    let mul = |p: &[Vec<f64>], q: &[Vec<f64>]| -> Vec<Vec<f64>> {
        (0..k)
            .map(|a| {
                (0..k)
                    .map(|b| (0..k).map(|c| p[a][c] * q[c][b]).sum())
                    .collect()
            })
            .collect()
    };
    let robust = mul(&mul(&ft.xtx_inv, &meat), &ft.xtx_inv);
    let rse: Vec<f64> = (0..k).map(|a| robust[a][a].sqrt()).collect();
    let df = nf - kf;
    let ymean = y.iter().sum::<f64>() / nf;
    let tss: f64 = y.iter().map(|v| (v - ymean).powi(2)).sum();
    let r2 = 1.0 - rss / tss;
    let adj = 1.0 - (1.0 - r2) * (nf - 1.0) / (nf - kf);
    let fstat = (r2 / (kf - 1.0)) / ((1.0 - r2) / (nf - kf));
    // Wald: b' V^-1 b over the explanatory coefficients.
    let sub: Vec<Vec<f64>> = (1..k)
        .map(|a| (1..k).map(|b| robust[a][b]).collect())
        .collect();
    let sub_inv = invert(&sub)?;
    let bb = &ft.b[1..];
    let wald: f64 = (0..k - 1)
        .map(|a| {
            (0..k - 1)
                .map(|b| bb[a] * sub_inv[a][b] * bb[b])
                .sum::<f64>()
        })
        .sum();
    // Koenker: n R^2 of e^2 on X.
    let e2: Vec<f64> = ft.e.iter().map(|v| v * v).collect();
    let aux = fit(&x, &e2)?;
    let m2 = e2.iter().sum::<f64>() / nf;
    let tss2: f64 = e2.iter().map(|v| (v - m2).powi(2)).sum();
    let rss2: f64 = aux.e.iter().map(|v| v * v).sum();
    let kbp = nf * (1.0 - rss2 / tss2);
    // Jarque-Bera.
    let mom = |p: i32| ft.e.iter().map(|v| v.powi(p)).sum::<f64>() / nf;
    let (s, kk) = (mom(3) / mom(2).powf(1.5), mom(4) / mom(2).powi(2));
    let jb = nf / 6.0 * (s * s + (kk - 3.0).powi(2) / 4.0);
    let ll = -nf / 2.0 * ((2.0 * std::f64::consts::PI).ln() + (rss / nf).ln() + 1.0);
    let aic = -2.0 * ll + 2.0 * (kf + 1.0);
    let aicc = aic + 2.0 * (kf + 1.0) * (kf + 2.0) / (nf - kf - 2.0);
    // Standardized coefficients (population standard deviations).
    let sd = |v: &mut dyn Iterator<Item = f64>| {
        let v: Vec<f64> = v.collect();
        let m = v.iter().sum::<f64>() / v.len() as f64;
        (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64).sqrt()
    };
    let sdy = sd(&mut y.iter().copied());
    let std_coef: Vec<f64> = (0..k)
        .map(|a| {
            if a == 0 {
                0.0 * ft.b[0].signum()
            } else {
                ft.b[a] * sd(&mut x.iter().map(|r| r[a])) / sdy
            }
        })
        .collect();
    // VIF for the messages.
    let vif: Vec<f64> = (1..k)
        .map(|a| {
            if k == 2 {
                return 1.0;
            }
            let xa: Vec<f64> = x.iter().map(|r| r[a]).collect();
            let others: Vec<Vec<f64>> = x
                .iter()
                .map(|r| (0..k).filter(|c| *c != a).map(|c| r[c]).collect())
                .collect();
            fit(&others, &xa).map_or(f64::NAN, |f| {
                let m = xa.iter().sum::<f64>() / nf;
                let t: f64 = xa.iter().map(|v| (v - m).powi(2)).sum();
                let r: f64 = f.e.iter().map(|v| v * v).sum();
                t / r
            })
        })
        .collect();
    // Tables.
    let field1 = || {
        let mut f = Field::new("Field1", FieldType::Integer);
        f.precision = 10;
        f
    };
    let text = |name: &str| {
        let mut f = Field::new(name, FieldType::String);
        f.length = 254;
        f
    };
    let mut cf = Vec::new();
    if dbase {
        cf.push(field1());
    }
    cf.push(text("Variable"));
    for name in [
        "Coef",
        "StdError",
        "t_Stat",
        "Prob",
        "Robust_SE",
        "Robust_t",
        "Robust_Pr",
        "StdCoef",
    ] {
        cf.push(Field::new(name, FieldType::Double));
    }
    let names: Vec<String> = std::iter::once("Intercept".to_string())
        .chain(ix.iter().map(|i| input.fields()[*i].name.clone()))
        .collect();
    let mut crows = Vec::new();
    for a in 0..k {
        let t = ft.b[a] / se[a];
        let rt = ft.b[a] / rse[a];
        let mut row = Vec::new();
        if dbase {
            row.push(Value::Integer(0));
        }
        row.push(Value::String(names[a].clone()));
        for v in [
            ft.b[a],
            se[a],
            t,
            t_two_sided(t, df),
            rse[a],
            rt,
            t_two_sided(rt, df),
            std_coef[a],
        ] {
            row.push(Value::Double(v));
        }
        crows.push(row);
    }
    let mut df_fields = Vec::new();
    if dbase {
        df_fields.push(field1());
    }
    df_fields.push(text("Diag_Name"));
    df_fields.push(Field::new("Diag_Value", FieldType::Double));
    df_fields.push(text("Definition"));
    let values = [
        aic,
        aicc,
        r2,
        adj,
        fstat,
        f_upper(fstat, kf - 1.0, nf - kf),
        wald,
        chi2_upper(wald, kf - 1.0),
        kbp,
        chi2_upper(kbp, kf - 1.0),
        jb,
        chi2_upper(jb, 2.0),
        sigma2,
    ];
    let drows = DEFINITIONS
        .iter()
        .zip(values)
        .map(|((name, def), v)| {
            let mut row = Vec::new();
            if dbase {
                row.push(Value::Integer(0));
            }
            row.push(Value::String(name.to_string()));
            row.push(Value::Double(v));
            row.push(Value::String(def.to_string()));
            row
        })
        .collect();
    // Features: the ID, dependent and explanatory fields, Estimated, Residual, StdResid.
    let copy = |i: usize| {
        let src = &input.fields()[i];
        let mut f = Field::new(src.name.clone(), src.field_type);
        f.length = src.length;
        f.precision = if src.field_type == FieldType::Integer {
            10
        } else {
            src.precision
        };
        f.scale = src.scale;
        f
    };
    let mut fields: Vec<Field> = vec![copy(iu), copy(iy)];
    fields.extend(ix.iter().map(|i| copy(*i)));
    for name in ["Estimated", "Residual", "StdResid"] {
        fields.push(Field::new(name, FieldType::Double));
    }
    let sigma = sigma2.sqrt();
    let features = feats
        .into_iter()
        .enumerate()
        .map(|(r, f)| {
            let mut attributes = vec![f.attributes[iu].clone(), f.attributes[iy].clone()];
            attributes.extend(ix.iter().map(|i| f.attributes[*i].clone()));
            let e = ft.e[r];
            attributes.push(Value::Double(y[r] - e));
            attributes.push(Value::Double(e));
            attributes.push(Value::Double(e / sigma));
            Feature {
                oid: r as i64,
                geometry: f.geometry,
                attributes,
            }
        })
        .collect();
    let mut messages = vec![
        "Summary of OLS Results".to_string(),
        format!(
            "{:>9} {:>14} {:>9} {:>11} {:>14} {:>9} {:>9} {:>12} {:>8}",
            "Variable",
            "Coefficient(a)",
            "StdError",
            "t-Statistic",
            "Probability(b)",
            "Robust_SE",
            "Robust_t",
            "Robust_Pr(b)",
            "VIF(c)"
        ),
    ];
    for a in 0..k {
        let t = ft.b[a] / se[a];
        let rt = ft.b[a] / rse[a];
        // ArcGIS stars p-values under 0.05 (its note says 0.01).
        let star = |p: f64| if p < 0.05 { "*" } else { " " };
        let (p, rp) = (t_two_sided(t, df), t_two_sided(rt, df));
        messages.push(format!(
            "{:>9} {:>14.6} {:>9.6} {:>11.6} {:>13.6}{} {:>9.6} {:>9.6} {:>11.6}{} {:>8}",
            names[a],
            ft.b[a],
            se[a],
            t,
            p,
            star(p),
            rse[a],
            rt,
            rp,
            star(rp),
            if a == 0 {
                "--------".to_string()
            } else {
                format!("{:.6}", vif[a - 1])
            }
        ));
    }
    messages.push(format!(
        "Number of Observations {n}; AICc {aicc:.6}; R-Squared {r2:.6}; Adjusted R-Squared {adj:.6}"
    ));
    messages.push(
        "WARNING 000851: Use the Spatial Autocorrelation (Moran's I) Tool to ensure residuals are not spatially autocorrelated."
            .into(),
    );
    Ok(Ols {
        features: ToolOutput {
            kind: input.geometry_kind().unwrap_or(GeometryKind::Point),
            has_z: false,
            has_m: false,
            fields,
            spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
            features,
            messages: Vec::new(),
        },
        coefficients: TableOutput {
            fields: cf,
            rows: crows,
        },
        diagnostics: TableOutput {
            fields: df_fields,
            rows: drows,
        },
        messages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distributions_match_known_values() {
        // scipy: 2*t.sf(2.79162526647, 56), f.sf(63.10598061, 1, 58), chi2.sf(5.15432502377, 3)
        assert!((t_two_sided(2.791_625_266_47, 56.0) - 0.007_160_168_582_324_414).abs() < 1e-14);
        assert!((f_upper(63.105_980_61, 1.0, 58.0) - 7.604_804_016_100_461e-11).abs() < 1e-20);
        assert!((chi2_upper(5.154_325_023_77, 3.0) - 0.160_839_272_006_083_92).abs() < 1e-13);
        assert!((chi2_upper(0.300_365_202_53, 2.0) - 0.860_550_824_408_372_4).abs() < 1e-13);
    }
}
