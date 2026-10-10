//! GeoForge geometry engine. Boolean overlay is delegated to `geo` (i_overlay);
//! this crate supplies Esri semantics on top: ring rules, multipart results, and
//! buffers densified exactly the way ArcGIS densifies arcs.

pub mod buffer;
pub mod clip;
pub mod convert;

pub use buffer::{BufferOptions, LineEnd, buffer, buffer_union};
pub use clip::{ClipArea, clip};
