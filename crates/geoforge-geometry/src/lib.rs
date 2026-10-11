//! GeoForge geometry engine. Boolean overlay is delegated to `geo` (i_overlay);
//! this crate supplies Esri semantics on top: ring rules, multipart results, and
//! buffers densified exactly the way ArcGIS densifies arcs.

pub mod buffer;
pub mod clip;
pub mod convert;
pub mod overlay;

pub use buffer::{BufferOptions, LineEnd, buffer, buffer_union};
pub use clip::{ClipArea, clip};
pub use overlay::{
    OverlayCell, cell_geometry, dissolve, erase, explode, intersection, polygon_overlay,
};
