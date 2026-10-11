//! GeoForge geometry engine. Boolean overlay is delegated to `geo` (i_overlay);
//! this crate supplies Esri semantics on top: ring rules, multipart results, and
//! buffers densified exactly the way ArcGIS densifies arcs.

pub mod buffer;
pub mod clip;
pub mod convert;
pub mod overlay;
pub mod proximity;

pub use buffer::{BufferOptions, LineEnd, buffer, buffer_union};
pub use clip::{ClipArea, clip};
pub use overlay::{
    OverlayCell, cell_geometry, dissolve, erase, explode, intersection, polygon_overlay,
};
pub use proximity::{Nearest, completely_within, nearest, within};
