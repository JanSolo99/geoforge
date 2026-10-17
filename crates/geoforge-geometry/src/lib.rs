//! GeoForge geometry engine. Boolean overlay is delegated to `geo` (i_overlay);
//! this crate supplies Esri semantics on top: ring rules, multipart results, and
//! buffers densified exactly the way ArcGIS densifies arcs.

pub mod bounding;
pub mod buffer;
pub mod clip;
pub mod convert;
pub mod geodesic;
pub mod index;
pub mod linework;
pub mod overlay;
pub mod proximity;
pub mod validate;

pub use bounding::{Bounding, BoundingKind, area, bounding, centroid, inside_point, length};
pub use buffer::{
    BufferCircle, BufferOptions, LineEnd, LineSide, buffer, buffer_union, recover_arcs,
};
pub use clip::{ClipArea, clip};
pub use geodesic::{GeodesicOptions, buffer_geodesic, buffer_geodesic_union};
pub use index::EnvelopeIndex;
pub use linework::{LineNodes, Meeting, XY_TOLERANCE, meeting, on_path};
pub use overlay::{
    OverlayCell, cell_geometry, dissolve, enclosed_gaps, erase, explode, intersection,
    paths_geometry, polygon_overlay,
};
pub use proximity::{Nearest, center, completely_within, nearest, outline, relate_matches, within};
pub use validate::{Problem, ValidationMethod, check, repair};
