//! Raster model and GeoTIFF I/O.
//!
//! A [`Raster`] holds one or more bands of cell values as `f64` together with the
//! stored cell type, the affine geotransform (GDAL order: origin x, cell width,
//! row rotation, origin y, column rotation, cell height — negative for north-up),
//! the NoData value and the coordinate system code.

pub mod blockops;
pub mod composite;
pub mod contour;
pub mod conversion;
pub mod cost;
pub mod density;
pub mod env;
pub mod extract;
pub mod focal;
pub mod frequency;
pub mod generalize;
mod geographic_crs;
pub mod geomorphon;
mod geotiff;
pub mod grid_import;
pub mod hydro;
pub mod interpolate;
pub mod local;
pub mod mathfn;
pub mod multiscale;
pub mod overlay;
pub mod rasterize;
pub mod registry;
pub mod resample;
pub mod rescale;
pub mod sample;
pub mod stats;
pub mod stats_metadata;
pub mod surface;
mod writer;
pub mod zonal;
pub mod zonal_tables;

pub use geotiff::{Compression, read_geotiff, write_geotiff};
pub use writer::{OverviewResampling, WriteOptions, write_geotiff_with};

/// The most cells an output raster may have (tools refuse parameters that would need more).
pub const MAX_CELLS: u64 = 1 << 31;

/// The pixel type a raster is stored with (ArcGIS "Pixel Type").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    F32,
    F64,
}

impl DataType {
    pub fn is_float(self) -> bool {
        matches!(self, DataType::F32 | DataType::F64)
    }

    /// Converts `v` the way a cast to this type stores it: floats round to f32,
    /// integers round half away from zero and saturate.
    pub fn store(self, v: f64) -> f64 {
        fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
            v.round().clamp(lo, hi)
        }
        match self {
            DataType::F64 => v,
            DataType::F32 => v as f32 as f64,
            DataType::U8 => clamp(v, 0.0, u8::MAX as f64),
            DataType::I8 => clamp(v, i8::MIN as f64, i8::MAX as f64),
            DataType::U16 => clamp(v, 0.0, u16::MAX as f64),
            DataType::I16 => clamp(v, i16::MIN as f64, i16::MAX as f64),
            DataType::U32 => clamp(v, 0.0, u32::MAX as f64),
            DataType::I32 => clamp(v, i32::MIN as f64, i32::MAX as f64),
        }
    }
}

/// An in-memory raster dataset.
#[derive(Debug, Clone)]
pub struct Raster {
    pub width: usize,
    pub height: usize,
    pub data_type: DataType,
    /// GDAL-order affine transform from (column, row) to map coordinates of the
    /// cell's upper-left corner.
    pub transform: [f64; 6],
    pub nodata: Option<f64>,
    /// EPSG code of the coordinate system, when the file names one.
    pub epsg: Option<u32>,
    /// Union of all bands' stored extrema (TIFF metadata or `.aux.xml`), used
    /// to choose a common output type. Only set when every band has extrema.
    /// Invalid multiband extrema yield NaNs, retaining float cache presence;
    /// registered integer multiband unary tools refuse those inputs.
    pub statistics: Option<(f64, f64)>,
    /// One `width * height` row-major vector per band. NoData cells hold the
    /// NoData value as stored.
    pub bands: Vec<Vec<f64>>,
}

impl Raster {
    /// A single-band raster of zeros.
    pub fn new(width: usize, height: usize, data_type: DataType, transform: [f64; 6]) -> Self {
        Raster {
            width,
            height,
            data_type,
            transform,
            nodata: None,
            epsg: None,
            statistics: None,
            bands: vec![vec![0.0; width * height]],
        }
    }

    /// A single-band raster on the same grid and coordinate system as `self`.
    pub fn like(&self, data_type: DataType) -> Self {
        Raster {
            nodata: None,
            statistics: None,
            data_type,
            bands: vec![vec![0.0; self.width * self.height]],
            ..self.clone()
        }
    }

    pub fn cell_width(&self) -> f64 {
        self.transform[1]
    }

    /// Cell height as a positive number.
    pub fn cell_height(&self) -> f64 {
        self.transform[5].abs()
    }

    /// The value at (column, row) of band 0, or `None` for NoData.
    pub fn get(&self, col: usize, row: usize) -> Option<f64> {
        let v = self.bands[0][row * self.width + col];
        if self.is_nodata(v) { None } else { Some(v) }
    }

    pub fn is_nodata(&self, v: f64) -> bool {
        match self.nodata {
            Some(nd) => v == nd || (nd.is_nan() && v.is_nan()),
            None => false,
        }
    }

    /// Map extent as (xmin, ymin, xmax, ymax) for a north-up raster.
    pub fn extent(&self) -> (f64, f64, f64, f64) {
        let t = &self.transform;
        let x2 = t[0] + t[1] * self.width as f64;
        let y2 = t[3] + t[5] * self.height as f64;
        (t[0].min(x2), t[3].min(y2), t[0].max(x2), t[3].max(y2))
    }
}
