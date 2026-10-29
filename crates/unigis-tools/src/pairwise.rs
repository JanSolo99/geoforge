//! Assign Weights By Pairwise Comparison (Analysis): AHP weights from a pairwise comparison
//! matrix.

use unigis_core::{Error, Field, FieldType, Result, Value};

use crate::TableOutput;

fn err(msg: &str) -> Error {
    Error::Unsupported(msg.into())
}

/// A variable name as the tool accepts it: a letter first, then letters, digits or `_`.
fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(char::is_alphabetic) && chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// A comparison value: a number or a fraction `a/b`, from 1/9 to 9.
fn value(text: &str) -> Option<f64> {
    let v = match text.split_once('/') {
        Some((a, b)) => a.trim().parse::<f64>().ok()? / b.trim().parse::<f64>().ok()?,
        None => text.parse::<f64>().ok()?,
    };
    (v.is_finite() && (1.0 / 9.0..=9.0).contains(&v)).then_some(v)
}

/// Assign Weights By Pairwise Comparison: the weights are the row means of the comparison
/// matrix with each column divided by its sum (Saaty's approximation, not the principal
/// eigenvector; observed against ArcGIS Pro 3.7). `matrix` is the arcpy string
/// `Variables A B 3; A C 5; B C 2`: every pair of the upper triangle exactly once, in
/// variable order, each value the importance of the first over the second (1/9..9, `a/b`
/// allowed); without it all comparisons are 1. With `add_matrix` the table also has the
/// matrix, one column per variable.
pub fn assign_weights_by_pairwise_comparison(
    variables: &[String],
    matrix: Option<&str>,
    add_matrix: bool,
) -> Result<TableOutput> {
    let n = variables.len();
    if !(2..=9).contains(&n) {
        return Err(err(
            "ERROR 004145: The number of input variables must be greater than 1 and smaller than 10.",
        ));
    }
    if !variables.iter().all(|v| valid_name(v)) {
        return Err(err(
            "ERROR 004153: One or more input variable names are invalid.",
        ));
    }
    if (1..n).any(|i| variables[..i].contains(&variables[i])) {
        return Err(err("ERROR 004146: Variable names must be unique."));
    }
    let mut m = vec![vec![1.0; n]; n];
    if let Some(text) = matrix.map(str::trim).filter(|t| !t.is_empty()) {
        let mismatch = || {
            err(
                "ERROR 004160: Input variable names do not match the variable names in the comparison matrix.",
            )
        };
        let body = text.strip_prefix("Variables").ok_or_else(mismatch)?;
        let items: Vec<Vec<&str>> = body
            .split(';')
            .map(|i| i.split_whitespace().collect())
            .collect();
        let pairs: Vec<(usize, usize)> = (0..n)
            .flat_map(|i| (i + 1..n).map(move |j| (i, j)))
            .collect();
        if items.len() != pairs.len()
            || items.iter().zip(&pairs).any(|(it, &(i, j))| {
                it.len() != 3 || it[0] != variables[i] || it[1] != variables[j]
            })
        {
            return Err(mismatch());
        }
        for (it, &(i, j)) in items.iter().zip(&pairs) {
            let v = value(it[2])
                .ok_or_else(|| err("ERROR 004155: One or more comparison values are invalid."))?;
            m[i][j] = v;
            m[j][i] = 1.0 / v;
        }
    }
    let sums: Vec<f64> = (0..n).map(|j| (0..n).map(|i| m[i][j]).sum()).collect();
    let mut fields = vec![{
        let mut f = Field::new("Variables", FieldType::String);
        f.length = 50;
        f
    }];
    // ArcGIS writes these as dBASE numbers 13 wide with 6 decimals.
    let double = |name: &str| {
        let mut f = Field::new(name, FieldType::Double);
        (f.length, f.precision, f.scale) = (13, 12, 6);
        f
    };
    if add_matrix {
        fields.extend(variables.iter().map(|v| double(v)));
    }
    fields.push(double("Weights"));
    let rows = (0..n)
        .map(|i| {
            let weight = (0..n).map(|j| m[i][j] / sums[j]).sum::<f64>() / n as f64;
            let mut row = vec![Value::String(variables[i].clone())];
            if add_matrix {
                row.extend(m[i].iter().map(|&v| Value::Double(v)));
            }
            row.push(Value::Double(weight));
            row
        })
        .collect();
    Ok(TableOutput { fields, rows })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(names: &str) -> Vec<String> {
        names.split(';').map(str::to_owned).collect()
    }

    fn weights(t: &TableOutput) -> Vec<f64> {
        t.rows
            .iter()
            .map(|r| match r.last() {
                Some(Value::Double(w)) => *w,
                _ => f64::NAN,
            })
            .collect()
    }

    #[test]
    fn weights_match_arcgis() {
        let t = assign_weights_by_pairwise_comparison(
            &vars("A;B;C"),
            Some("Variables A B 3; A C 5; B C 2"),
            true,
        )
        .unwrap();
        let w = weights(&t);
        for (a, b) in w
            .iter()
            .zip([0.6479468599033816, 0.22987117552334943, 0.12218196457326892])
        {
            assert!((a - b).abs() < 1e-15, "{w:?}");
        }
        assert_eq!(t.rows[1][1], Value::Double(1.0 / 3.0));
        let t = assign_weights_by_pairwise_comparison(
            &vars("A;B;C"),
            Some("Variables A B 1/3; A C 1; B C 1"),
            false,
        )
        .unwrap();
        assert!((weights(&t)[0] - 0.2253968253968254).abs() < 1e-15);
        assert_eq!(t.fields.len(), 2);
    }

    #[test]
    fn invalid_inputs_fail_as_in_arcgis() {
        let run = |v: &str, m: Option<&str>| {
            assign_weights_by_pairwise_comparison(&vars(v), m, true)
                .unwrap_err()
                .to_string()
        };
        assert!(run("A", None).contains("004145"));
        assert!(run("V1;V2;V3;V4;V5;V6;V7;V8;V9;V10", None).contains("004145"));
        assert!(run("1A;B;C", None).contains("004153"));
        assert!(run("A;A;B", None).contains("004146"));
        for m in [
            "Variables B A 3; C A 5; C B 2",
            "Variables A B 3; A C 5",
            "Variables A C 5; A B 3; B C 2",
            "Variables a b 3; a c 5; b c 2",
        ] {
            assert!(run("A;B;C", Some(m)).contains("004160"), "{m}");
        }
        for m in [
            "Variables A B 9.0000001; A C 1; B C 1",
            "Variables A B 0.111111111111111; A C 1; B C 1",
            "Variables A B 0; A C 1; B C 1",
        ] {
            assert!(run("A;B;C", Some(m)).contains("004155"), "{m}");
        }
    }
}
