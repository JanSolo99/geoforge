//! Export Raster World File (Data Management): the world file beside a raster.
//!
//! As ArcGIS Pro 3.7 writes it: six lines, each a number printed with `%.18g` -- the cell width,
//! `0`, `-0`, the negated cell height, then the map coordinates of the centre of the upper-left
//! cell. The file sits beside the raster with the extension of its first and last letters plus
//! `w` (`.tif` -> `.tfw`, `.img` -> `.igw`, `.jp2` -> `.j2w`) and replaces an existing one. Lines end
//! with CR LF.

use std::path::{Path, PathBuf};

use crate::Raster;

/// `v` as C's `%.18g` prints it.
pub fn format_g18(v: f64) -> String {
    const P: i32 = 18;
    if v == 0.0 {
        return if v.is_sign_negative() { "-0" } else { "0" }.into();
    }
    if !v.is_finite() {
        return v.to_string();
    }
    let sci = format!("{:.*e}", (P - 1) as usize, v);
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let x: i32 = exp.parse().unwrap_or(0);
    if !(-4..P).contains(&x) {
        let m = mantissa.trim_end_matches('0').trim_end_matches('.');
        let sign = if x < 0 { '-' } else { '+' };
        return format!("{m}e{sign}{:02}", x.abs());
    }
    let fixed = format!("{:.*}", (P - 1 - x).max(0) as usize, v);
    if fixed.contains('.') {
        fixed.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        fixed
    }
}

/// The world file's text for `raster`; rotated or skewed rasters are not supported.
pub fn world_file_text(raster: &Raster) -> Result<String, String> {
    let [x0, a, b, y0, d, e] = raster.transform;
    if b != 0.0 || d != 0.0 {
        return Err("a rotated raster's world file is not supported".into());
    }
    // ArcGIS takes the cell size from the extent, so it carries the extent's rounding
    // (0.0001 degrees prints as 9.99999999999978635e-05 on a 150-column raster).
    let (cols, rows) = (raster.width as f64, raster.height as f64);
    let width = (x0 + cols * a - x0) / cols;
    let height = (y0 - (y0 + rows * e)) / rows;
    let lines = [
        width,
        0.0,
        -0.0,
        -height,
        x0 + width / 2.0,
        y0 - height / 2.0,
    ];
    Ok(lines.iter().map(|v| format_g18(*v) + "\r\n").collect())
}

/// Where the world file of the raster at `path` goes.
pub fn world_file_path(path: &Path) -> Option<PathBuf> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    let (first, last) = (ext.chars().next()?, ext.chars().last()?);
    Some(path.with_extension(format!("{first}{last}w")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataType;

    #[test]
    fn numbers_print_like_printf_g_18() {
        assert_eq!(format_g18(10.0), "10");
        assert_eq!(format_g18(-10.0), "-10");
        assert_eq!(format_g18(500005.0), "500005");
        assert_eq!(format_g18(-0.0), "-0");
        assert_eq!(format_g18(0.0), "0");
        assert_eq!(format_g18(9.999999999999786e-05), "9.99999999999978635e-05");
        assert_eq!(
            format_g18(-0.00010000000000000379),
            "-0.000100000000000003786"
        );
        assert_eq!(
            format_g18(5.0 + 9.999999999999786e-05 / 2.0),
            "5.00004999999999988"
        );
        assert_eq!(format_g18(1e20), "1e+20");
    }

    #[test]
    fn a_world_file_has_the_centre_of_the_upper_left_cell() {
        let r = Raster::new(
            150,
            120,
            DataType::F32,
            [500000.0, 10.0, 0.0, 5001200.0, 0.0, -10.0],
        );
        assert_eq!(
            world_file_text(&r).unwrap(),
            "10\r\n0\r\n-0\r\n-10\r\n500005\r\n5001195\r\n"
        );
        let nonsquare = Raster::new(4, 3, DataType::F32, [1000.0, 20.0, 0.0, 2030.0, 0.0, -10.0]);
        assert_eq!(
            world_file_text(&nonsquare).unwrap(),
            "20\r\n0\r\n-0\r\n-10\r\n1010\r\n2025\r\n"
        );
        // The cell size follows from the extent, as in ArcGIS (150 x 120 cells of 0.0001 degrees).
        let geographic = Raster::new(
            150,
            120,
            DataType::U8,
            [5.0, 0.0001, 0.0, 52.0, 0.0, -0.0001],
        );
        assert_eq!(
            world_file_text(&geographic).unwrap(),
            "9.99999999999978635e-05\r\n0\r\n-0\r\n-0.000100000000000003786\r\n5.00004999999999988\r\n51.9999499999999983\r\n"
        );
        let rotated = Raster::new(2, 2, DataType::F32, [0.0, 1.0, 0.5, 0.0, 0.5, -1.0]);
        assert!(world_file_text(&rotated).is_err());
        assert_eq!(
            world_file_path(Path::new("/a/dem.tif")).unwrap(),
            Path::new("/a/dem.tfw")
        );
        assert_eq!(
            world_file_path(Path::new("/a/x.img")).unwrap(),
            Path::new("/a/x.igw")
        );
        assert_eq!(
            world_file_path(Path::new("/a/x.jp2")).unwrap(),
            Path::new("/a/x.j2w")
        );
        assert!(world_file_path(Path::new("/a/noext")).is_none());
    }
}
