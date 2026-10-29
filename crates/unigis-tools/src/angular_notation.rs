//! Angular input syntax observed with ArcGIS Pro 3.7 Convert Coordinate Notation.
use unigis_core::Value;

pub(crate) fn input_parts(format: &str) -> Option<(usize, bool)> {
    Some(match format {
        "DD_1" => (1, true),
        "DD_2" => (1, false),
        "DDM_1" => (2, true),
        "DDM_2" => (2, false),
        "DMS_1" => (3, true),
        "DMS_2" => (3, false),
        _ => return None,
    })
}

// Both axes have the same component count. Hemispheres override signed degrees,
// and an explicit E/W first axis reverses the default latitude-first text order.
fn text_pair(text: &str, parts: usize) -> Option<(f64, f64)> {
    let mut numbers = Vec::new();
    let mut hemispheres = Vec::new();
    let chars: Vec<_> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() || c == '+' || c == '-' {
            let start = i;
            if c == '+' || c == '-' {
                i += 1;
            }
            if !chars.get(i)?.is_ascii_digit() {
                return None;
            }
            while chars.get(i).is_some_and(char::is_ascii_digit) {
                i += 1;
            }
            if chars.get(i) == Some(&'.') {
                i += 1;
                while chars.get(i).is_some_and(char::is_ascii_digit) {
                    i += 1;
                }
            }
            numbers.push(chars[start..i].iter().collect::<String>());
            continue;
        }
        match c.to_ascii_uppercase() {
            h @ ('N' | 'S' | 'E' | 'W') => hemispheres.push(h),
            ' ' | '\t' | '\r' | '\n' | ',' | '°' | '\'' | '"' => (),
            _ => return None,
        }
        i += 1;
    }
    if !hemispheres.is_empty() && hemispheres.len() != 2 {
        return None;
    }
    if hemispheres.len() == 2 && is_lat(hemispheres[0]) == is_lat(hemispheres[1]) {
        return None;
    }
    let n = numbers.len() / 2;
    if numbers.len() != 2 * n || n == 0 || n > parts {
        return None;
    }
    // DDM/DMS may be packed, but mixing packed and split axes fails in ArcGIS.
    if n != parts && n != 1 {
        return None;
    }
    let a = axis(&numbers[..n], parts)?;
    let b = axis(&numbers[n..], parts)?;
    let signed = |v: f64, h: char| {
        if h == 'S' || h == 'W' {
            -v.abs()
        } else {
            v.abs()
        }
    };
    let (lat, lon) = if hemispheres.is_empty() {
        (a, b)
    } else {
        let (a, b) = (signed(a, hemispheres[0]), signed(b, hemispheres[1]));
        if is_lat(hemispheres[0]) {
            (a, b)
        } else {
            (b, a)
        }
    };
    valid(lon, lat)
}

fn is_lat(h: char) -> bool {
    h == 'N' || h == 'S'
}

fn axis(tokens: &[String], parts: usize) -> Option<f64> {
    if tokens.len() == 1 && parts > 1 {
        let s = tokens[0].trim_start_matches(['+', '-']);
        let digits = s.split('.').next()?.len();
        let value: f64 = s.parse().ok()?;
        let angle = match (parts, digits) {
            (_, 1..=3) => value,
            (_, 4..=5) => (value / 100.0).floor() + (value % 100.0) / 60.0,
            (3, 6..=7) => {
                (value / 10000.0).floor()
                    + ((value / 100.0).floor() % 100.0) / 60.0
                    + (value % 100.0) / 3600.0
            }
            _ => return None,
        };
        return Some(if tokens[0].starts_with('-') {
            -angle
        } else {
            angle
        });
    }
    let degrees: f64 = tokens[0].parse().ok()?;
    if tokens.len() > 1 && tokens[..tokens.len() - 1].iter().any(|s| s.contains('.')) {
        return None;
    }
    let mut angle = degrees.abs();
    for (i, token) in tokens.iter().enumerate().skip(1) {
        if token.starts_with(['+', '-']) {
            return None;
        }
        angle += token.parse::<f64>().ok()? / 60_f64.powi(i as i32);
    }
    Some(if degrees.is_sign_negative() {
        -angle
    } else {
        angle
    })
}

fn valid(lon: f64, lat: f64) -> Option<(f64, f64)> {
    (lon.is_finite() && lat.is_finite() && lon.abs() <= 180.0 && lat.abs() <= 90.0)
        .then_some((lon, lat))
}

pub(crate) fn parse(x: &Value, y: Option<&Value>, parts: usize) -> Option<(f64, f64)> {
    if let Some(y) = y {
        if parts == 1
            && let (Some(x), Some(y)) = (crate::number(x), crate::number(y))
        {
            return valid(x, y);
        }
        let text = |v: &Value| match v {
            Value::String(s) => Some(s.clone()),
            _ => crate::number(v).map(|v| format!("{v:.15}")),
        };
        text_pair(&format!("{} {}", text(y)?, text(x)?), parts)
    } else if let Value::String(s) = x {
        text_pair(s, parts)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observed_angular_syntax() {
        for (text, parts, lon, lat) in [
            ("15.123456W 45.678901S", 1, -15.123456, -45.678901),
            ("N45.678901 E15.123456", 1, 15.123456, 45.678901),
            ("-45.678901N -15.123456E", 1, 15.123456, 45.678901),
            ("45°40.73406'N 15°07.40736'E", 2, 15.123456, 45.678901),
            ("4540.73406N 01507.40736E", 2, 15.123456, 45.678901),
            ("150724.4416E 454044.0436N", 3, 15.123456, 45.678901),
            ("45 40 61N 15 7 24E", 3, 15.123333333333, 45.683611111111),
            ("4540N 01507E", 3, 15.116666666667, 45.666666666667),
        ] {
            let (x, y) = text_pair(text, parts).unwrap_or_else(|| panic!("{text}"));
            assert!(
                (x - lon).abs() < 1e-10 && (y - lat).abs() < 1e-10,
                "{text}: {x}, {y}"
            );
        }
        for (s, parts) in [
            (".5 .25", 1),
            ("45N 15N", 1),
            ("91 15", 1),
            ("45 181", 1),
            ("45 15 7", 1),
            ("45.5 40 15 7", 2),
            ("45:40:44N 15:7:24E", 3),
            ("45 40 44N 0150724E", 3),
            ("45404404N 015072444E", 3),
            ("junk", 1),
        ] {
            assert!(text_pair(s, parts).is_none(), "{s}");
        }
    }
}
