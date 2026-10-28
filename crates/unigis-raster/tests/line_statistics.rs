//! Failure controls: invalid parameters must not make a raster or overflow grid allocation.
use unigis_raster::interpolate::line_statistics;

#[test]
fn invalid_line_statistics_inputs_do_not_allocate_or_compute() {
    let lines = vec![(vec![vec![[0.0, 0.0], [20.0, 20.0]]], 1.0)];
    for cell in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::MIN_POSITIVE] {
        assert!(line_statistics(&lines, true, cell, 6.0, "MEAN", None).is_none());
    }
    for radius in [0.0, -6.0, f64::NAN, f64::INFINITY] {
        assert!(line_statistics(&lines, true, 10.0, radius, "MEAN", None).is_none());
    }
    for stat in ["VARIETY", "MAJORITY", "MINORITY", "UNKNOWN"] {
        assert!(line_statistics(&lines, false, 10.0, 6.0, stat, None).is_none());
    }
    assert!(line_statistics(&[], true, 10.0, 6.0, "MEAN", None).is_none());
    let flat = vec![(vec![vec![[0.0, 0.0], [20.0, 0.0]]], 1.0)];
    assert!(line_statistics(&flat, true, 10.0, 6.0, "MEAN", None).is_none());
}
