/// Axis-aligned bounding box, with optional Z and M ranges.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Envelope {
    pub xmin: f64,
    pub ymin: f64,
    pub xmax: f64,
    pub ymax: f64,
    pub z: Option<(f64, f64)>,
    pub m: Option<(f64, f64)>,
}

impl Envelope {
    /// An envelope that contains nothing; expanding it by any point yields that point.
    pub const EMPTY: Envelope = Envelope {
        xmin: f64::INFINITY,
        ymin: f64::INFINITY,
        xmax: f64::NEG_INFINITY,
        ymax: f64::NEG_INFINITY,
        z: None,
        m: None,
    };

    pub fn is_empty(&self) -> bool {
        self.xmin > self.xmax || self.ymin > self.ymax
    }

    pub fn expand_xy(&mut self, x: f64, y: f64) {
        self.xmin = self.xmin.min(x);
        self.ymin = self.ymin.min(y);
        self.xmax = self.xmax.max(x);
        self.ymax = self.ymax.max(y);
    }

    pub fn expand_z(&mut self, z: f64) {
        self.z = Some(expand_range(self.z, z));
    }

    /// NaN (no measure) is ignored.
    pub fn expand_m(&mut self, m: f64) {
        if !m.is_nan() {
            self.m = Some(expand_range(self.m, m));
        }
    }

    pub fn union(&mut self, other: &Envelope) {
        if !other.is_empty() {
            self.expand_xy(other.xmin, other.ymin);
            self.expand_xy(other.xmax, other.ymax);
        }
        if let Some((lo, hi)) = other.z {
            self.expand_z(lo);
            self.expand_z(hi);
        }
        if let Some((lo, hi)) = other.m {
            self.expand_m(lo);
            self.expand_m(hi);
        }
    }
}

fn expand_range(range: Option<(f64, f64)>, v: f64) -> (f64, f64) {
    match range {
        Some((lo, hi)) => (lo.min(v), hi.max(v)),
        None => (v, v),
    }
}
