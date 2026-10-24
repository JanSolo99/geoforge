//! UniGIS's ArcGIS-verified raster tools (`unigis-raster`) as UniGIS tools.
//!
//! Modified for UniGIS (not part of upstream RustGIS).
//!
//! A raster is read from its GeoTIFF (the file it came from, or the bytes a project embeds) so
//! that the pixel type and NoData value are the file's; a raster from another format is converted
//! from the pixels the app holds. Results are written as GeoTIFFs with ArcGIS's tiling and
//! compression and come back as raster layers. Rasters keep their own coordinate system; vector
//! results (contours, points, polygons) are projected to longitude/latitude like every layer.

use std::path::{Path, PathBuf};

use gfr::env::{Reads, Sampling};
use std::sync::atomic::{AtomicU64, Ordering};

use rustgis_analysis::{Inputs, Output, OutputLayer, ParamKind as K, ParamSpec as P, ToolResult};
use rustgis_data::raster::Crs;
use rustgis_data::{Dataset, Field, FieldType, Geometry, GeometryType, Value};
use rustgis_geom::Coord;
use serde_json::Value as Json;
use unigis_raster as gfr;

use crate::gf;
use crate::{Def, INPUT, OUTPUT, flag, layer, number, out_name, table_dataset, text};

const RASTER: P = P { name: "input", label: "Input Raster", kind: K::RasterLayer, required: true, default: "", help: "" };

/// A scratch file in the temp directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(ext: &str) -> Scratch {
        static N: AtomicU64 = AtomicU64::new(0);
        let name = format!("unigis-{}-{}.{ext}", std::process::id(), N.fetch_add(1, Ordering::Relaxed));
        Scratch(std::env::temp_dir().join(name))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let _ = std::fs::remove_file(self.0.with_extension("tif.aux.xml"));
    }
}

fn is_tiff(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".tif") || n.ends_with(".tiff")
}

/// The pixel type named in a raster's file ("UInt8", "Float32", ...).
fn data_type_of(name: &str) -> gfr::DataType {
    use gfr::DataType as D;
    match name.to_ascii_lowercase().as_str() {
        "uint8" | "u8" => D::U8,
        "int8" | "i8" => D::I8,
        "uint16" => D::U16,
        "int16" => D::I16,
        "uint32" => D::U32,
        "int32" => D::I32,
        "float64" => D::F64,
        _ => D::F32,
    }
}

/// The raster the app holds, as UniGIS's model.
fn convert(r: &rustgis_data::raster::Raster) -> gfr::Raster {
    let t = &r.transform;
    let bands = r.bands.iter().map(|b| b.data.iter().map(|v| f64::from(*v)).collect()).collect();
    gfr::Raster {
        width: r.width as usize,
        height: r.height as usize,
        data_type: data_type_of(&r.data_type),
        transform: [t.origin_x, t.pixel_width, 0.0, t.origin_y, 0.0, t.pixel_height],
        nodata: r.nodata,
        epsg: match r.crs.epsg() {
            0 => None,
            c => Some(u32::from(c)),
        },
        statistics: None,
        bands,
    }
}

/// A raster layer, read from its GeoTIFF when it has one.
fn load(inp: &dyn Inputs, name: &str) -> Result<(gfr::Raster, Crs), String> {
    let r = inp.raster(name).ok_or_else(|| format!("no raster layer named '{name}'"))?;
    if let Some(path) = r.path.filter(|p| is_tiff(p))
        && let Ok(g) = gfr::read_geotiff(Path::new(path))
    {
        return Ok((g, r.raster.crs));
    }
    if let Some((file, bytes)) = r.embedded.filter(|(f, _)| is_tiff(f)) {
        let tmp = Scratch::new("tif");
        if std::fs::write(&tmp.0, bytes).is_ok()
            && let Ok(g) = gfr::read_geotiff(&tmp.0)
        {
            return Ok((g, r.raster.crs));
        }
        let _ = file;
    }
    Ok((convert(r.raster), r.raster.crs))
}

fn input(inp: &dyn Inputs, p: &Json, key: &str) -> Result<(gfr::Raster, Crs), String> {
    load(inp, text(p, key))
}

/// ArcGIS's output layout: 128 x 128 tiles, LZW for integers.
fn encode(r: &gfr::Raster) -> Result<Vec<u8>, String> {
    let compression = if r.data_type.is_float() { gfr::Compression::None } else { gfr::Compression::Lzw };
    let tmp = Scratch::new("tif");
    gfr::write_geotiff_with(&tmp.0, r, &gfr::WriteOptions { compression, tile: Some((128, 128)), ..Default::default() })
        .map_err(|e| e.to_string())?;
    std::fs::read(&tmp.0).map_err(|e| e.to_string())
}

fn raster_layer(r: &gfr::Raster, name: &str) -> Result<OutputLayer, String> {
    Ok(OutputLayer::Raster { name: name.to_string(), file_name: format!("{name}.tif"), bytes: encode(r)? })
}

fn done(layers: Vec<(String, gfr::Raster)>) -> Result<ToolResult, String> {
    let mut out = Vec::new();
    let mut messages = Vec::new();
    for (name, r) in &layers {
        out.push(raster_layer(r, name)?);
        messages.push(format!("{name}: {} x {} cells", r.width, r.height));
    }
    Ok(ToolResult { output: Output::Layers(out), messages })
}

/// The environment parameters every environment-aware raster tool takes (`arcpy.env`).
pub(crate) const ENV_PARAMS: &[P] = &[
    P {
        name: "cell_size",
        label: "Cell size (environment)",
        kind: K::Number,
        required: false,
        default: "",
        help: "Output cell size, in the raster's units.",
    },
    P {
        name: "extent",
        label: "Extent (environment)",
        kind: K::Text,
        required: false,
        default: "",
        help: "\"xmin ymin xmax ymax\" in the raster's coordinate system.",
    },
    P { name: "snap_raster", label: "Snap raster (environment)", kind: K::RasterLayer, required: false, default: "", help: "" },
    P {
        name: "mask",
        label: "Mask (environment)",
        kind: K::Text,
        required: false,
        default: "",
        help: "A raster or polygon layer; cells outside it are NoData.",
    },
];

/// The tools that honour the environments.
pub(crate) fn takes_environment(id: &str) -> bool {
    !matches!(
        id,
        "contour"
            | "resample"
            | "euclidean_distance"
            | "euclidean_direction"
            | "euclidean_allocation"
            | "extract_by_mask"
            | "zonal_statistics"
            | "raster_to_point"
            | "raster_to_polygon"
            | "polygon_to_raster"
            | "polyline_to_raster"
            | "point_to_raster"
            | "zonal_statistics_as_table"
            | "aggregate"
            | "block_statistics"
            | "pick"
            | "expand"
            | "shrink"
            | "majority_filter"
    )
}

/// A raster tool's parameters with the environment parameters appended (leaked once per tool).
pub(crate) fn params_of(d: &Def) -> &'static [P] {
    static CACHE: std::sync::Mutex<Vec<(&'static str, &'static [P])>> = std::sync::Mutex::new(Vec::new());
    if !DEFS.iter().any(|x| x.id == d.id) || !takes_environment(d.id) {
        return d.params;
    }
    let mut cache = CACHE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((_, p)) = cache.iter().find(|(id, _)| *id == d.id) {
        return p;
    }
    let env: Vec<P> = ENV_PARAMS.iter().filter(|e| !d.params.iter().any(|x| x.name == e.name)).cloned().collect();
    let all: &'static [P] = Box::leak([d.params, env.as_slice()].concat().into_boxed_slice());
    cache.push((d.id, all));
    all
}

fn polygons_in(ds: &Dataset, crs: Crs) -> Result<Vec<gf::Geometry>, String> {
    let t = crs.transformer()?;
    let mut out = Vec::new();
    for f in &ds.features {
        let Some(Geometry::Polygon(polys)) = &f.geometry else { continue };
        for poly in polys {
            let mut g = gf::Geometry::point(0.0, 0.0);
            g.kind = gf::GeometryKind::Polygon;
            g.parts.clear();
            g.xy.clear();
            for ring in poly {
                g.parts.push(g.xy.len() as u32);
                for c in ring {
                    let c = t.from_lonlat(*c).ok_or("a mask vertex is outside the raster's coordinate system")?;
                    g.xy.push([c.x, c.y]);
                }
            }
            out.push(g);
        }
    }
    Ok(out)
}

/// The environments set in the parameters.
fn env_of(inp: &dyn Inputs, p: &Json, crs: Crs) -> Result<gfr::env::Environment, String> {
    let mut e = gfr::env::Environment::default();
    if p.get("cell_size").is_some_and(|v| !v.is_null() && v.as_str() != Some("")) {
        e.cell_size = Some(number(p, "cell_size").filter(|c| *c > 0.0).ok_or("the cell size environment must be a positive number")?);
    }
    if !text(p, "extent").is_empty() {
        let v: Vec<f64> = text(p, "extent")
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|w| !w.is_empty())
            .map(str::parse)
            .collect::<Result<_, _>>()
            .map_err(|_| format!("extent '{}'", text(p, "extent")))?;
        if v.len() != 4 || v.iter().any(|x| !x.is_finite()) {
            return Err(format!("extent '{}': expected xmin ymin xmax ymax", text(p, "extent")));
        }
        e.extent = Some([v[0], v[1], v[2], v[3]]);
    }
    if !text(p, "snap_raster").is_empty() {
        e.snap = Some(gfr::env::Snap::of(&load(inp, text(p, "snap_raster"))?.0));
    }
    let mask = text(p, "mask");
    if !mask.is_empty() {
        e.mask = Some(if inp.raster(mask).is_some() {
            gfr::env::Mask::Raster(load(inp, mask)?.0)
        } else {
            let ds = inp.dataset(mask).ok_or_else(|| format!("no layer named '{mask}'"))?;
            gfr::env::Mask::Polygons(polygons_in(ds, crs)?)
        });
    }
    Ok(e)
}

/// Runs `f` on `rasters` under the environments in `p`.
fn eval(
    inp: &dyn Inputs,
    p: &Json,
    rasters: &[&gfr::Raster],
    crs: Crs,
    reads: gfr::env::Reads,
    sampling: gfr::env::Sampling,
    f: &dyn Fn(&[gfr::Raster]) -> gfr::Raster,
) -> Result<gfr::Raster, String> {
    let env = env_of(inp, p, crs)?;
    gfr::env::run(rasters, &env, reads, sampling, f).ok_or_else(|| {
        "no output grid: the input rasters do not overlap, or the environments give more than the supported number of cells".to_string()
    })
}

fn one(p: &Json, default: String, r: gfr::Raster) -> Result<ToolResult, String> {
    done(vec![(out_name(p, default), r)])
}

fn choice<'a>(p: &'a Json, key: &str, default: &'a str) -> String {
    let v = text(p, key);
    if v.is_empty() { default } else { v }.to_ascii_uppercase()
}

/// The name of the raster layer parameter `key` names, for naming the result.
fn base_name(_: &dyn Inputs, p: &Json, key: &str) -> String {
    text(p, key).to_string()
}

// ---------------------------------------------------------------------------------------------
// surface

fn slope(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (dem, crs) = input(inp, p, "input")?;
    let unit = match choice(p, "output_measurement", "DEGREE").as_str() {
        "DEGREE" => gfr::surface::SlopeUnit::Degree,
        "PERCENT_RISE" => gfr::surface::SlopeUnit::PercentRise,
        other => return Err(format!("output measurement {other}")),
    };
    let z = number(p, "z_factor").unwrap_or(1.0);
    one(
        p,
        format!("{}_Slope", base_name(inp, p, "input")),
        eval(inp, p, &[&dem], crs, Reads::Context, Sampling::Bilinear, &|r| gfr::surface::slope(&r[0], unit, z))?,
    )
}

fn aspect(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (dem, crs) = input(inp, p, "input")?;
    one(
        p,
        format!("{}_Aspect", base_name(inp, p, "input")),
        eval(inp, p, &[&dem], crs, Reads::Context, Sampling::Bilinear, &|r| gfr::surface::aspect(&r[0]))?,
    )
}

fn hillshade(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (dem, crs) = input(inp, p, "input")?;
    let (az, alt, z) = (number(p, "azimuth").unwrap_or(315.0), number(p, "altitude").unwrap_or(45.0), number(p, "z_factor").unwrap_or(1.0));
    let shadows = flag(p, "model_shadows");
    let out = eval(inp, p, &[&dem], crs, Reads::Cropped, Sampling::Bilinear, &|r| {
        if shadows { gfr::surface::hillshade_with(&r[0], az, alt, z, true) } else { gfr::surface::hillshade(&r[0], az, alt, z) }
    })?;
    one(p, format!("{}_Hillshade", base_name(inp, p, "input")), out)
}

fn curvature(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (dem, crs) = input(inp, p, "input")?;
    let z = number(p, "z_factor").unwrap_or(1.0);
    let call = |r: &gfr::Raster| gfr::surface::curvature(r, z);
    let (curv, profile, plan) = if env_of(inp, p, crs)?.is_empty() {
        call(&dem)
    } else {
        // The three outputs share one grid: run the tool once per output.
        let pick = |k: usize| {
            eval(inp, p, &[&dem], crs, Reads::Context, Sampling::Bilinear, &move |r| {
                let (a, b, c) = call(&r[0]);
                [a, b, c][k].clone()
            })
        };
        (pick(0)?, pick(1)?, pick(2)?)
    };
    let n = out_name(p, format!("{}_Curvature", base_name(inp, p, "input")));
    let mut layers = vec![(n.clone(), curv)];
    if flag(p, "profile") {
        layers.push((format!("{n}_Profile"), profile));
    }
    if flag(p, "plan") {
        layers.push((format!("{n}_Plan"), plan));
    }
    done(layers)
}

/// A projected coordinate in longitude/latitude, when the raster's system can be converted.
fn to_lonlat(crs: Crs) -> Result<impl Fn(f64, f64) -> Option<Coord>, String> {
    let t = crs.transformer().map_err(|e| e.to_string())?;
    Ok(move |x: f64, y: f64| t.to_lonlat(Coord::new(x, y)).filter(|c| c.x.is_finite() && c.y.is_finite()))
}

fn contour(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (r, crs) = input(inp, p, "input")?;
    let interval = number(p, "contour_interval").filter(|i| *i > 0.0).ok_or("Contour: 'Contour Interval' must be a positive number")?;
    let base = number(p, "base_contour").unwrap_or(0.0);
    if gfr::contour::level_count(&r, interval, base) > gfr::contour::MAX_LEVELS {
        return Err(format!("Contour: the interval {interval} gives more than {} contour levels", gfr::contour::MAX_LEVELS));
    }
    let lines = gfr::contour::contour(&r, interval, base);
    let ll = to_lonlat(crs)?;
    let mut out = Dataset::new(
        out_name(p, format!("{}_Contour", base_name(inp, p, "input"))),
        GeometryType::Polyline,
        vec![Field::new("Id", FieldType::Integer), Field::new("Contour", FieldType::Double)],
    );
    for (i, l) in lines.into_iter().enumerate() {
        let pts: Vec<Coord> = l.points.iter().filter_map(|c| ll(c[0], c[1])).collect();
        if pts.len() >= 2 {
            out.push_next(Some(Geometry::Line(vec![pts])), vec![Value::Int(i as i64 + 1), Value::Float(l.value)]);
        }
    }
    let m = vec![format!("{}: {} contours", out.name, out.features.len())];
    Ok(ToolResult { output: Output::Layer(out), messages: m })
}

// ---------------------------------------------------------------------------------------------
// hydrology

fn fill(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (dem, crs) = input(inp, p, "input")?;
    let limit = number(p, "z_limit");
    one(
        p,
        format!("{}_Fill", base_name(inp, p, "input")),
        eval(inp, p, &[&dem], crs, Reads::Cropped, Sampling::Nearest, &|r| gfr::hydro::fill(&r[0], limit))?,
    )
}

fn flow_direction(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (dem, crs) = input(inp, p, "input")?;
    let force = match choice(p, "force_flow", "NORMAL").as_str() {
        "NORMAL" => false,
        "FORCE" => true,
        other => return Err(format!("force_flow {other}")),
    };
    one(
        p,
        format!("{}_FlowDir", base_name(inp, p, "input")),
        eval(inp, p, &[&dem], crs, Reads::Context, Sampling::Nearest, &|r| gfr::hydro::flow_direction(&r[0], force))?,
    )
}

fn flow_accumulation(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (d, crs) = input(inp, p, "input")?;
    let weight = if text(p, "weight").is_empty() { None } else { Some(input(inp, p, "weight")?.0) };
    let integer = match choice(p, "data_type", "FLOAT").as_str() {
        "FLOAT" => false,
        "INTEGER" => true,
        other => return Err(format!("data type {other}")),
    };
    one(p, format!("{}_FlowAcc", base_name(inp, p, "input")), {
        let mut rs = vec![&d];
        rs.extend(weight.as_ref());
        let has_weight = weight.is_some();
        eval(inp, p, &rs, crs, Reads::Cropped, Sampling::Nearest, &|r| {
            gfr::hydro::flow_accumulation(&r[0], if has_weight { r.get(1) } else { None }, integer)
        })?
    })
}

fn basin(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (d, crs) = input(inp, p, "input")?;
    one(
        p,
        format!("{}_Basin", base_name(inp, p, "input")),
        eval(inp, p, &[&d], crs, Reads::Cropped, Sampling::Nearest, &|r| gfr::hydro::basin(&r[0]))?,
    )
}

fn watershed(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (d, crs) = input(inp, p, "input")?;
    let (pour, _) = input(inp, p, "pour_points")?;
    one(
        p,
        format!("{}_Watershed", base_name(inp, p, "input")),
        eval(inp, p, &[&d, &pour], crs, Reads::Cropped, Sampling::Nearest, &|r| gfr::hydro::watershed(&r[0], &r[1]))?,
    )
}

fn stream_link(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (s, crs) = input(inp, p, "streams")?;
    let (d, _) = input(inp, p, "input")?;
    one(
        p,
        format!("{}_StreamLink", base_name(inp, p, "streams")),
        eval(inp, p, &[&s, &d], crs, Reads::Cropped, Sampling::Nearest, &|r| gfr::hydro::stream_link(&r[0], &r[1]))?,
    )
}

fn stream_order(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    use gfr::hydro::StreamOrder;
    let (s, crs) = input(inp, p, "streams")?;
    let (d, _) = input(inp, p, "input")?;
    let method = match choice(p, "order_method", "STRAHLER").as_str() {
        "STRAHLER" => StreamOrder::Strahler,
        "SHREVE" => StreamOrder::Shreve,
        other => return Err(format!("order method {other}")),
    };
    one(
        p,
        format!("{}_StreamOrder", base_name(inp, p, "streams")),
        eval(inp, p, &[&s, &d], crs, Reads::Cropped, Sampling::Nearest, &|r| gfr::hydro::stream_order(&r[0], &r[1], method))?,
    )
}

fn flow_length(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (d, crs) = input(inp, p, "input")?;
    let upstream = match choice(p, "direction_measurement", "DOWNSTREAM").as_str() {
        "DOWNSTREAM" => false,
        "UPSTREAM" => true,
        other => return Err(format!("direction {other}")),
    };
    one(
        p,
        format!("{}_FlowLength", base_name(inp, p, "input")),
        eval(inp, p, &[&d], crs, Reads::Cropped, Sampling::Nearest, &|r| gfr::hydro::flow_length(&r[0], upstream))?,
    )
}

// ---------------------------------------------------------------------------------------------
// neighbourhood, reclassify, zonal, resample

fn data_flag(p: &Json, key: &str) -> Result<bool, String> {
    match choice(p, key, "DATA").as_str() {
        "DATA" => Ok(true),
        "NODATA" => Ok(false),
        other => Err(format!("{key} {other}")),
    }
}

fn focal_statistics(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    use gfr::focal::{FocalStatistic, Neighborhood};
    let (r, crs) = input(inp, p, "input")?;
    let nbr = Neighborhood::parse(if text(p, "neighborhood").is_empty() { "Rectangle 3 3 CELL" } else { text(p, "neighborhood") })
        .ok_or_else(|| format!("neighborhood {}", text(p, "neighborhood")))?;
    if nbr.cell_count() > 4_000_000 {
        return Err(format!("Focal Statistics: the neighborhood spans {} cells", nbr.cell_count()));
    }
    let stat =
        FocalStatistic::parse(&choice(p, "statistics_type", "MEAN")).ok_or_else(|| format!("statistic {}", text(p, "statistics_type")))?;
    one(p, format!("{}_Focal", base_name(inp, p, "input")), {
        let ignore = data_flag(p, "ignore_nodata")?;
        eval(inp, p, &[&r], crs, Reads::Cropped, Sampling::Nearest, &|x| gfr::focal::focal_statistics(&x[0], nbr, stat, ignore))?
    })
}

fn reclassify(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (r, crs) = input(inp, p, "input")?;
    let entries = gfr::zonal::parse_remap(text(p, "remap")).ok_or_else(|| format!("remap {}", text(p, "remap")))?;
    one(p, format!("{}_Reclass", base_name(inp, p, "input")), {
        let missing = data_flag(p, "missing_values")?;
        eval(inp, p, &[&r], crs, Reads::Cropped, Sampling::Nearest, &|x| gfr::zonal::reclassify(&x[0], &entries, missing))?
    })
}

fn zonal_statistics(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (zones, _) = input(inp, p, "zones")?;
    let (values, _) = input(inp, p, "input")?;
    let stat = gfr::zonal::ZonalStatistic::parse(&choice(p, "statistics_type", "MEAN"))
        .ok_or_else(|| format!("statistic {}", text(p, "statistics_type")))?;
    let (zones, values) = gfr::resample::zones_on_value_grid(&zones, &values).ok_or("zone and value rasters do not overlap")?;
    one(
        p,
        format!("{}_Zonal", base_name(inp, p, "zones")),
        gfr::zonal::zonal_statistics(&zones, &values, stat, data_flag(p, "ignore_nodata")?),
    )
}

fn resample(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    use gfr::resample::Technique;
    let (r, _) = input(inp, p, "input")?;
    let cell = number(p, "cell_size").filter(|c| *c > 0.0).ok_or("Resample: 'Cell Size' must be a positive number")?;
    let (cols, rows) = gfr::resample::output_size(&r, cell);
    if cols.saturating_mul(rows) > gfr::MAX_CELLS {
        return Err(format!("Resample: a cell size of {cell} would give {cols} x {rows} cells"));
    }
    let t = match choice(p, "resampling_type", "NEAREST").as_str() {
        "NEAREST" => Technique::Nearest,
        "BILINEAR" => Technique::Bilinear,
        "CUBIC" => Technique::Cubic,
        "MAJORITY" => Technique::Majority,
        other => return Err(format!("resampling type {other}")),
    };
    one(p, format!("{}_Resample", base_name(inp, p, "input")), gfr::resample::resample(&r, cell, t))
}

fn euclidean_distance(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (r, _) = input(inp, p, "input")?;
    one(p, format!("{}_EucDist", base_name(inp, p, "input")), gfr::resample::euclidean_distance(&r, number(p, "maximum_distance")))
}

fn euclidean_direction(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (r, _) = input(inp, p, "input")?;
    one(p, format!("{}_EucDir", base_name(inp, p, "input")), gfr::resample::euclidean_direction(&r, number(p, "maximum_distance")))
}

fn euclidean_allocation(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (r, _) = input(inp, p, "input")?;
    one(p, format!("{}_EucAllo", base_name(inp, p, "input")), gfr::resample::euclidean_allocation(&r, number(p, "maximum_distance")))
}

fn extract_by_mask(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (r, _) = input(inp, p, "input")?;
    let (m, _) = input(inp, p, "mask")?;
    let out = gfr::resample::extract_by_mask(&r, &m).ok_or("raster and mask do not overlap")?;
    one(p, format!("{}_Extract", base_name(inp, p, "input")), out)
}

// ---------------------------------------------------------------------------------------------
// local

/// A raster layer's name or a number.
fn operand(inp: &dyn Inputs, text_value: &str) -> Result<gfr::local::Operand, String> {
    match gfr::local::Operand::constant(text_value) {
        Some(c) => Ok(c),
        None => Ok(gfr::local::Operand::Raster(load(inp, text_value)?.0)),
    }
}

/// Runs `f` on operands under the environments: the raster operands go onto the processing grid, the
/// constants stay.
fn eval_operands(
    inp: &dyn Inputs,
    p: &Json,
    ops: &[&gfr::local::Operand],
    f: &dyn Fn(&[gfr::local::Operand]) -> Option<gfr::Raster>,
) -> Result<gfr::Raster, String> {
    use gfr::local::Operand;
    let rasters: Vec<&gfr::Raster> = ops.iter().filter_map(|o| if let Operand::Raster(r) = o { Some(r) } else { None }).collect();
    let crs = match ["in1", "input"].into_iter().find(|k| inp.raster(text(p, k)).is_some()) {
        Some(k) => Some(load(inp, text(p, k))?.1),
        None => None,
    };
    let env = match crs {
        Some(c) => env_of(inp, p, c)?,
        None => gfr::env::Environment::default(),
    };
    if env.is_empty() || rasters.is_empty() {
        let owned: Vec<Operand> = ops.iter().map(|o| (*o).clone()).collect();
        return f(&owned).ok_or_else(|| "the input rasters do not overlap".to_string());
    }
    gfr::env::run(&rasters, &env, Reads::Cropped, Sampling::Nearest, &|prepared| {
        let mut it = prepared.iter();
        let swapped: Vec<Operand> = ops
            .iter()
            .map(|o| {
                if let Operand::Raster(_) = o {
                    it.next().map_or_else(|| (*o).clone(), |r| Operand::Raster(r.clone()))
                } else {
                    (*o).clone()
                }
            })
            .collect();
        f(&swapped).unwrap_or_else(|| gfr::Raster::new(1, 1, gfr::DataType::F32, [0.0, 1.0, 0.0, 1.0, 0.0, -1.0]))
    })
    .ok_or_else(|| "the input rasters do not overlap".to_string())
}

fn math(op: gfr::local::MathOp, suffix: &str, inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (a, b) = (operand(inp, text(p, "in1"))?, operand(inp, text(p, "in2"))?);
    let out = eval_operands(inp, p, &[&a, &b], &|o| gfr::local::math(op, &o[0], &o[1]))?;
    one(p, format!("{}_{suffix}", base_name(inp, p, "in1")), out)
}

fn plus(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    math(gfr::local::MathOp::Plus, "Plus", inp, p)
}
fn minus(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    math(gfr::local::MathOp::Minus, "Minus", inp, p)
}
fn times(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    math(gfr::local::MathOp::Times, "Times", inp, p)
}
fn divide(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    math(gfr::local::MathOp::Divide, "Divide", inp, p)
}

fn int(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (r, crs) = input(inp, p, "input")?;
    one(
        p,
        format!("{}_Int", base_name(inp, p, "input")),
        eval(inp, p, &[&r], crs, Reads::Cropped, Sampling::Nearest, &|x| gfr::local::int(&x[0]))?,
    )
}
fn float(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (r, crs) = input(inp, p, "input")?;
    one(
        p,
        format!("{}_Float", base_name(inp, p, "input")),
        eval(inp, p, &[&r], crs, Reads::Cropped, Sampling::Nearest, &|x| gfr::local::float(&x[0]))?,
    )
}
fn is_null(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (r, crs) = input(inp, p, "input")?;
    one(
        p,
        format!("{}_IsNull", base_name(inp, p, "input")),
        eval(inp, p, &[&r], crs, Reads::Cropped, Sampling::Nearest, &|x| gfr::local::is_null(&x[0]))?,
    )
}

fn clause(p: &Json) -> Result<Option<gfr::local::ValueClause>, String> {
    let t = text(p, "where_clause");
    if t.is_empty() {
        return Ok(None);
    }
    gfr::local::ValueClause::parse(t).map(Some).ok_or_else(|| format!("where clause {t}"))
}

fn con(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (cond, _) = input(inp, p, "input")?;
    let t = operand(inp, text(p, "true_value"))?;
    let f = if text(p, "false_value").is_empty() { None } else { Some(operand(inp, text(p, "false_value"))?) };
    let cl = clause(p)?;
    let cond_op = gfr::local::Operand::Raster(cond);
    let fv = f.clone().unwrap_or(gfr::local::Operand::Constant(0.0));
    let out = eval_operands(inp, p, &[&cond_op, &t, &fv], &|o| {
        let gfr::local::Operand::Raster(c) = &o[0] else { return None };
        gfr::local::con(c, &o[1], f.as_ref().map(|_| &o[2]), cl.as_ref())
    })?;
    one(p, format!("{}_Con", base_name(inp, p, "input")), out)
}

fn set_null(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (cond, _) = input(inp, p, "input")?;
    let f = operand(inp, text(p, "false_value"))?;
    let cl = clause(p)?;
    let cond_op = gfr::local::Operand::Raster(cond);
    let out = eval_operands(inp, p, &[&cond_op, &f], &|o| {
        let gfr::local::Operand::Raster(c) = &o[0] else { return None };
        gfr::local::set_null(c, &o[1], cl.as_ref())
    })?;
    one(p, format!("{}_SetNull", base_name(inp, p, "input")), out)
}

// ---------------------------------------------------------------------------------------------
// conversion

fn raster_to_point(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (r, crs) = input(inp, p, "input")?;
    let float = r.data_type.is_float();
    let ll = to_lonlat(crs)?;
    let mut out = Dataset::new(
        out_name(p, format!("{}_Points", base_name(inp, p, "input"))),
        GeometryType::Point,
        vec![
            Field::new("pointid", FieldType::Integer),
            Field::new("grid_code", if float { FieldType::Double } else { FieldType::Integer }),
        ],
    );
    for (i, (pt, v)) in gfr::conversion::raster_to_points(&r).into_iter().enumerate() {
        if let Some(c) = ll(pt[0], pt[1]) {
            out.push_next(
                Some(Geometry::Point(c)),
                vec![Value::Int(i as i64 + 1), if float { Value::Float(v) } else { Value::Int(v as i64) }],
            );
        }
    }
    let m = vec![format!("{}: {} points", out.name, out.features.len())];
    Ok(ToolResult { output: Output::Layer(out), messages: m })
}

fn raster_to_polygon(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (r, crs) = input(inp, p, "input")?;
    if r.data_type.is_float() {
        return Err("ERROR 010247: The input raster must be integer".into());
    }
    let ll = to_lonlat(crs)?;
    let mut out = Dataset::new(
        out_name(p, format!("{}_Polygons", base_name(inp, p, "input"))),
        GeometryType::Polygon,
        vec![Field::new("Id", FieldType::Integer), Field::new("gridcode", FieldType::Integer)],
    );
    for (i, poly) in gfr::conversion::raster_to_polygons(&r).into_iter().enumerate() {
        // The first ring is the outline, the others holes.
        let rings: Vec<Vec<Coord>> = poly.rings.iter().map(|ring| ring.iter().filter_map(|c| ll(c[0], c[1])).collect()).collect();
        if rings.first().is_some_and(|r| r.len() >= 4) {
            out.push_next(Some(Geometry::Polygon(vec![rings])), vec![Value::Int(i as i64 + 1), Value::Int(poly.value as i64)]);
        }
    }
    let m = vec![format!("{}: {} polygons", out.name, out.features.len())];
    Ok(ToolResult { output: Output::Layer(out), messages: m })
}

// ---------------------------------------------------------------------------------------------
// vector to raster, zonal table

type Rasterized = (Vec<(unigis_core::Geometry, f64)>, gfr::rasterize::ValueKind);

/// The features of a layer with a numeric value field (the feature id when no field is named), as
/// `(geometry, value)` pairs in longitude/latitude, and the field's value kind.
fn rasterize_input(inp: &dyn Inputs, p: &Json) -> Result<Rasterized, String> {
    use gfr::rasterize::ValueKind;
    let (ds, fc) = layer(inp, p, "input")?;
    let field = text(p, "value_field");
    let by_id = field.is_empty() || field.eq_ignore_ascii_case("OBJECTID") || field.eq_ignore_ascii_case("FID");
    let index = ds.field_index(field);
    let kind = if by_id {
        ValueKind::Integer
    } else {
        match index.map(|i| ds.fields[i].ty) {
            Some(FieldType::Integer) => ValueKind::Integer,
            Some(FieldType::Double) => ValueKind::Double,
            _ => return Err(format!("field '{field}' is missing or not numeric")),
        }
    };
    let mut out = Vec::new();
    for f in &fc.features {
        let value = if by_id {
            Some(f.oid as f64)
        } else {
            match f.attributes.get(index.unwrap_or(0)) {
                Some(unigis_core::Value::BigInteger(v)) => Some(*v as f64),
                Some(unigis_core::Value::Double(v)) => Some(*v),
                _ => None,
            }
        };
        if let (Some(g), Some(v)) = (&f.geometry, value) {
            out.push((g.clone(), v));
        }
    }
    if out.is_empty() {
        return Err("no features with a value to rasterize".into());
    }
    Ok((out, kind))
}

/// The cell size, refused when the features would need more cells than a raster may have.
fn cell_size(p: &Json, features: &[(unigis_core::Geometry, f64)]) -> Result<f64, String> {
    let cell = number(p, "cell_size").filter(|c| *c > 0.0).ok_or("'Cell size' must be a positive number (in degrees)")?;
    let mut e = unigis_core::Envelope::EMPTY;
    for (g, _) in features {
        let b = g.envelope();
        e.expand_xy(b.xmin, b.ymin);
        e.expand_xy(b.xmax, b.ymax);
    }
    let cells = ((e.xmax - e.xmin) / cell + 1.0) * ((e.ymax - e.ymin) / cell + 1.0);
    if !cells.is_finite() || cells > gfr::MAX_CELLS as f64 {
        return Err(format!("a cell size of {cell} would need {cells:e} cells"));
    }
    Ok(cell)
}

fn finish_rasterized(p: &Json, r: Option<gfr::Raster>, default: String) -> Result<ToolResult, String> {
    let mut r = r.ok_or("no features to rasterize")?;
    r.epsg = Some(4326);
    one(p, default, r)
}

fn polygon_to_raster(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    use gfr::rasterize::*;
    let (features, kind) = rasterize_input(inp, p)?;
    let cell = cell_size(p, &features)?;
    let assignment = match choice(p, "cell_assignment", "CELL_CENTER").as_str() {
        "CELL_CENTER" => PolygonAssignment::CellCenter,
        "MAXIMUM_AREA" => PolygonAssignment::MaximumArea,
        "MAXIMUM_COMBINED_AREA" => PolygonAssignment::MaximumCombinedArea,
        other => return Err(format!("cell assignment {other}")),
    };
    finish_rasterized(p, polygons_to_raster(&features, cell, assignment, kind), format!("{}_Raster", text(p, "input")))
}

fn polyline_to_raster(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    use gfr::rasterize::*;
    let (features, kind) = rasterize_input(inp, p)?;
    let cell = cell_size(p, &features)?;
    let assignment = match choice(p, "cell_assignment", "MAXIMUM_LENGTH").as_str() {
        "MAXIMUM_LENGTH" => LineAssignment::MaximumLength,
        "MAXIMUM_COMBINED_LENGTH" => LineAssignment::MaximumCombinedLength,
        other => return Err(format!("cell assignment {other}")),
    };
    finish_rasterized(p, polyline_to_raster_fn(&features, cell, assignment, kind), format!("{}_Raster", text(p, "input")))
}

fn polyline_to_raster_fn(
    features: &[(unigis_core::Geometry, f64)],
    cell: f64,
    assignment: gfr::rasterize::LineAssignment,
    kind: gfr::rasterize::ValueKind,
) -> Option<gfr::Raster> {
    gfr::rasterize::polyline_to_raster(features, cell, assignment, kind)
}

fn point_to_raster(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    use gfr::rasterize::*;
    let (features, kind) = rasterize_input(inp, p)?;
    let cell = cell_size(p, &features)?;
    let assignment = match choice(p, "cell_assignment", "MOST_FREQUENT").as_str() {
        "MOST_FREQUENT" => PointAssignment::MostFrequent,
        "SUM" => PointAssignment::Sum,
        "MEAN" => PointAssignment::Mean,
        "MAXIMUM" => PointAssignment::Maximum,
        "MINIMUM" => PointAssignment::Minimum,
        "COUNT" => PointAssignment::Count,
        "RANGE" => PointAssignment::Range,
        "STANDARD_DEVIATION" => PointAssignment::StandardDeviation,
        other => return Err(format!("cell assignment {other}")),
    };
    finish_rasterized(p, points_to_raster(&features, cell, assignment, kind), format!("{}_Raster", text(p, "input")))
}

// ---------------------------------------------------------------------------------------------
// overlay and fuzzy

/// The rasters named in the semicolon list `key`, and the first one's coordinate system.
fn rasters_in(inp: &dyn Inputs, p: &Json, key: &str) -> Result<(Vec<gfr::Raster>, Crs), String> {
    let names: Vec<String> = text(p, key).split(';').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
    if names.is_empty() {
        return Err("name the input rasters, separated by semicolons".into());
    }
    let mut out = Vec::new();
    let mut crs = Crs::LonLat;
    for (i, n) in names.iter().enumerate() {
        let (r, c) = load(inp, n)?;
        if i == 0 {
            crs = c;
        }
        out.push(r);
    }
    Ok((out, crs))
}

/// Runs `f` over `rasters` on the processing grid; a tool that finds no overlap gives no raster.
fn overlay_eval(
    inp: &dyn Inputs,
    p: &Json,
    rasters: &[gfr::Raster],
    crs: Crs,
    f: &dyn Fn(&[&gfr::Raster]) -> Option<gfr::Raster>,
) -> Result<gfr::Raster, String> {
    let refs: Vec<&gfr::Raster> = rasters.iter().collect();
    if gfr::local::local_grid(&refs).is_none() {
        return Err("the input rasters do not overlap".into());
    }
    let nothing = || gfr::Raster::new(1, 1, gfr::DataType::F32, [0.0, 1.0, 0.0, 1.0, 0.0, -1.0]);
    eval(inp, p, &refs, crs, Reads::Cropped, Sampling::Nearest, &|r| {
        let rs: Vec<&gfr::Raster> = r.iter().collect();
        f(&rs).unwrap_or_else(nothing)
    })
}

fn cell_statistics(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (rs, crs) = rasters_in(inp, p, "inputs")?;
    let stat =
        gfr::overlay::CellStat::parse(&choice(p, "statistic", "MEAN")).ok_or_else(|| format!("statistic {}", text(p, "statistic")))?;
    let ignore = match choice(p, "ignore_nodata", "DATA").as_str() {
        "DATA" => true,
        "NODATA" => false,
        other => return Err(format!("ignore NoData {other}: DATA or NODATA")),
    };
    one(p, "CellStatistics".into(), overlay_eval(inp, p, &rs, crs, &|r| gfr::overlay::cell_statistics(r, stat, ignore))?)
}

fn weighted_sum(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (rs, crs) = rasters_in(inp, p, "inputs")?;
    let weights: Vec<f64> = text(p, "weights")
        .split(';')
        .map(|w| w.trim().parse::<f64>().map_err(|_| format!("weight '{w}' is not a number")))
        .collect::<Result<_, _>>()?;
    if weights.len() != rs.len() {
        return Err(format!("{} weights for {} rasters", weights.len(), rs.len()));
    }
    one(p, "WeightedSum".into(), overlay_eval(inp, p, &rs, crs, &|r| gfr::overlay::weighted_sum(r, &weights))?)
}

fn position(inp: &dyn Inputs, p: &Json, highest: bool) -> Result<ToolResult, String> {
    let (rs, crs) = rasters_in(inp, p, "inputs")?;
    one(
        p,
        (if highest { "HighestPosition" } else { "LowestPosition" }).into(),
        overlay_eval(inp, p, &rs, crs, &|r| gfr::overlay::position(r, highest))?,
    )
}

fn highest_position(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    position(inp, p, true)
}

fn lowest_position(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    position(inp, p, false)
}

fn combine(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (rs, crs) = rasters_in(inp, p, "inputs")?;
    one(p, "Combine".into(), overlay_eval(inp, p, &rs, crs, &gfr::overlay::combine)?)
}

/// `layer:influence:from=to,from=to` entries separated by semicolons (`to` a number or NODATA).
fn weighted_overlay(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    use gfr::overlay::{Scale, Weighted};
    let mut rasters = Vec::new();
    let mut specs: Vec<(f64, Vec<(f64, Scale)>)> = Vec::new();
    let mut crs = Crs::LonLat;
    for (i, entry) in text(p, "table").split(';').map(str::trim).filter(|e| !e.is_empty()).enumerate() {
        let parts: Vec<&str> = entry.splitn(3, ':').collect();
        if parts.len() != 3 {
            return Err(format!("'{entry}': expected layer:influence:from=to,from=to"));
        }
        let (r, c) = load(inp, parts[0].trim())?;
        if i == 0 {
            crs = c;
        }
        let influence: f64 = parts[1].trim().parse().map_err(|_| format!("influence '{}' is not a number", parts[1]))?;
        let mut remap = Vec::new();
        for pair in parts[2].split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let (from, to) = pair.split_once('=').ok_or_else(|| format!("'{pair}': expected from=to"))?;
            let from: f64 = from.trim().parse().map_err(|_| format!("value '{from}' is not a number"))?;
            let to = if to.trim().eq_ignore_ascii_case("NODATA") {
                Scale::NoData
            } else {
                Scale::Value(to.trim().parse().map_err(|_| format!("scale value '{to}' is not a number"))?)
            };
            remap.push((from, to));
        }
        rasters.push(r);
        specs.push((influence, remap));
    }
    if rasters.is_empty() {
        return Err("fill the table: layer:influence:from=to,from=to".into());
    }
    if (specs.iter().map(|s| s.0).sum::<f64>() - 100.0).abs() > 1e-9 {
        return Err("the influences must add up to 100".into());
    }
    let scale: Vec<f64> = text(p, "evaluation_scale")
        .split(|c: char| c.is_whitespace() || c == ';' || c == ',')
        .filter(|w| !w.is_empty())
        .map(|w| w.parse::<f64>().map_err(|_| format!("evaluation scale '{w}'")))
        .collect::<Result<_, _>>()?;
    let scale = match scale.as_slice() {
        [] => (1.0, 9.0),
        [lo, hi, ..] if lo <= hi => (*lo, *hi),
        _ => return Err("the evaluation scale is 'from to by', for example 1 9 1".into()),
    };
    let out = overlay_eval(inp, p, &rasters, crs, &|r| {
        let inputs: Vec<Weighted> = r
            .iter()
            .zip(&specs)
            .map(|(raster, (influence, remap))| Weighted { raster, influence: *influence, remap: remap.clone() })
            .collect();
        gfr::overlay::weighted_overlay(&inputs, scale)
    })?;
    one(p, "WeightedOverlay".into(), out)
}

fn fuzzy_membership(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    use gfr::overlay::{Fuzzy, Hedge};
    let (r, crs) = input(inp, p, "input")?;
    let need = |k: &str| number(p, k).ok_or_else(|| format!("'{k}' is needed for this function"));
    let class = match choice(p, "function", "LINEAR").as_str() {
        "LINEAR" => Fuzzy::Linear { min: need("minimum")?, max: need("maximum")? },
        "GAUSSIAN" => Fuzzy::Gaussian { midpoint: need("midpoint")?, spread: number(p, "spread").unwrap_or(0.1) },
        "LARGE" => Fuzzy::Large { midpoint: need("midpoint")?, spread: number(p, "spread").unwrap_or(5.0) },
        "SMALL" => Fuzzy::Small { midpoint: need("midpoint")?, spread: number(p, "spread").unwrap_or(5.0) },
        "NEAR" => Fuzzy::Near { midpoint: need("midpoint")?, spread: number(p, "spread").unwrap_or(0.1) },
        "MSLARGE" => Fuzzy::MsLarge {
            mean_multiplier: number(p, "mean_multiplier").unwrap_or(1.0),
            std_multiplier: number(p, "std_multiplier").unwrap_or(1.0),
        },
        "MSSMALL" => Fuzzy::MsSmall {
            mean_multiplier: number(p, "mean_multiplier").unwrap_or(1.0),
            std_multiplier: number(p, "std_multiplier").unwrap_or(1.0),
        },
        other => return Err(format!("function {other}")),
    };
    let hedge = match choice(p, "hedge", "NONE").as_str() {
        "NONE" => Hedge::None,
        "SOMEWHAT" => Hedge::Somewhat,
        "VERY" => Hedge::Very,
        other => return Err(format!("hedge {other}")),
    };
    // The statistics of MS Large and MS Small are the input's own: fuzzify before any environment.
    let out = eval(inp, p, &[&r], crs, Reads::Cropped, Sampling::Nearest, &|x| gfr::overlay::fuzzy_membership(&x[0], class, hedge))?;
    one(p, format!("{}_Fuzzy", text(p, "input")), out)
}

fn fuzzy_overlay(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    use gfr::overlay::FuzzyOverlayType;
    let (rs, crs) = rasters_in(inp, p, "inputs")?;
    let kind = match choice(p, "overlay_type", "AND").as_str() {
        "AND" => FuzzyOverlayType::And,
        "OR" => FuzzyOverlayType::Or,
        "PRODUCT" => FuzzyOverlayType::Product,
        "SUM" => FuzzyOverlayType::Sum,
        "GAMMA" => FuzzyOverlayType::Gamma(number(p, "gamma").filter(|g| (0.0..=1.0).contains(g)).ok_or("gamma is a number from 0 to 1")?),
        other => return Err(format!("overlay type {other}")),
    };
    one(p, "FuzzyOverlay".into(), overlay_eval(inp, p, &rs, crs, &|r| gfr::overlay::fuzzy_overlay(r, kind))?)
}

// ---------------------------------------------------------------------------------------------
// math functions (gfr::mathfn)

/// The one-raster math functions: tool id, ArcGIS name, the name `mathfn` parses, summary.
const MATH_UNARY: &[(&str, &str, &str, &str)] = &[
    ("abs", "Abs", "Abs", "The absolute value of each cell."),
    ("exp", "Exp", "Exp", "e raised to the power of each cell's value."),
    ("exp2", "Exp2", "Exp2", "2 raised to the power of each cell's value."),
    ("exp10", "Exp10", "Exp10", "10 raised to the power of each cell's value."),
    ("ln", "Ln", "Ln", "The natural logarithm of each cell (NoData for values of 0 or less)."),
    ("log2", "Log2", "Log2", "The base 2 logarithm of each cell."),
    ("log10", "Log10", "Log10", "The base 10 logarithm of each cell."),
    ("square_root", "Square Root", "SquareRoot", "The square root of each cell (NoData for negative values)."),
    ("square", "Square", "Square", "The square of each cell."),
    ("negate", "Negate", "Negate", "Each cell's value with its sign changed."),
    ("sin", "Sin", "Sin", "The sine of each cell (radians)."),
    ("cos", "Cos", "Cos", "The cosine of each cell (radians)."),
    ("tan", "Tan", "Tan", "The tangent of each cell (radians)."),
    ("asin", "ASin", "ASin", "The inverse sine of each cell (NoData outside -1 to 1)."),
    ("acos", "ACos", "ACos", "The inverse cosine of each cell (NoData outside -1 to 1)."),
    ("atan", "ATan", "ATan", "The inverse tangent of each cell."),
    ("sinh", "SinH", "SinH", "The hyperbolic sine of each cell."),
    ("cosh", "CosH", "CosH", "The hyperbolic cosine of each cell."),
    ("tanh", "TanH", "TanH", "The hyperbolic tangent of each cell."),
    ("asinh", "ASinH", "ASinH", "The inverse hyperbolic sine of each cell."),
    ("acosh", "ACosH", "ACosH", "The inverse hyperbolic cosine of each cell (NoData below 1)."),
    ("atanh", "ATanH", "ATanH", "The inverse hyperbolic tangent of each cell (NoData outside -1 to 1)."),
    ("round_down", "Round Down", "RoundDown", "Each cell's value rounded down to a whole number (a float raster)."),
    ("round_up", "Round Up", "RoundUp", "Each cell's value rounded up to a whole number (a float raster)."),
    ("boolean_not", "Boolean Not", "BooleanNot", "1 where a cell is 0, else 0."),
    ("bitwise_not", "Bitwise Not", "BitwiseNot", "The bitwise complement of an integer raster."),
];

/// The two-operand math functions.
const MATH_BINARY: &[(&str, &str, &str, &str)] = &[
    ("power", "Power", "Power", "The first raster raised to the power of the second."),
    ("mod", "Mod", "Mod", "The remainder of the first raster divided by the second (the dividend's sign)."),
    ("atan2", "ATan2", "ATan2", "The angle from the x-axis of a point (second, first)."),
    ("equal_to", "Equal To", "EqualTo", "1 where the operands are equal, else 0."),
    ("not_equal", "Not Equal", "NotEqual", "1 where the operands differ, else 0."),
    ("greater_than", "Greater Than", "GreaterThan", "1 where the first is greater, else 0."),
    ("greater_than_equal", "Greater Than Equal", "GreaterThanEqual", "1 where the first is greater or equal, else 0."),
    ("less_than", "Less Than", "LessThan", "1 where the first is less, else 0."),
    ("less_than_equal", "Less Than Equal", "LessThanEqual", "1 where the first is less or equal, else 0."),
    ("boolean_and", "Boolean And", "BooleanAnd", "1 where both are not 0, else 0."),
    ("boolean_or", "Boolean Or", "BooleanOr", "1 where either is not 0, else 0."),
    ("boolean_xor", "Boolean XOr", "BooleanXor", "1 where exactly one is not 0, else 0."),
    ("diff", "Diff", "Diff", "The first value where the operands differ, else 0."),
    ("over", "Over", "Over", "The first value where it is not 0, else the second."),
    ("bitwise_and", "Bitwise And", "BitwiseAnd", "The bitwise AND of two integer rasters."),
    ("bitwise_or", "Bitwise Or", "BitwiseOr", "The bitwise OR of two integer rasters."),
    ("bitwise_xor", "Bitwise XOr", "BitwiseXor", "The bitwise exclusive OR of two integer rasters."),
    ("bitwise_left_shift", "Bitwise Left Shift", "BitwiseLeftShift", "The first shifted left by the second (32-bit)."),
    ("bitwise_right_shift", "Bitwise Right Shift", "BitwiseRightShift", "The first shifted right by the second (32-bit)."),
];

/// The tool specs of the math functions: `(id, name, toolbox, summary, params)`.
pub(crate) fn math_specs() -> Vec<(&'static str, &'static str, &'static str, &'static str, &'static [P])> {
    let mut out = Vec::new();
    for (unary, table) in [(true, MATH_UNARY), (false, MATH_BINARY)] {
        for (id, name, _, summary) in table {
            let mut params: Vec<P> = if unary {
                vec![P {
                    name: "in1",
                    label: "Input raster",
                    kind: K::Text,
                    required: true,
                    default: "",
                    help: "A raster layer name or a number.",
                }]
            } else {
                vec![
                    P {
                        name: "in1",
                        label: "Input raster or constant 1",
                        kind: K::Text,
                        required: true,
                        default: "",
                        help: "A raster layer name or a number.",
                    },
                    P { name: "in2", label: "Input raster or constant 2", kind: K::Text, required: true, default: "", help: "" },
                ]
            };
            params.push(OUTPUT);
            params.extend(ENV_PARAMS.iter().cloned());
            let leaked: &'static [P] = Box::leak(params.into_boxed_slice());
            out.push((*id, *name, "UniGIS › Spatial Analyst › Math", *summary, leaked));
        }
    }
    out
}

/// Runs a math function by tool id; `None` when `id` is not one.
pub(crate) fn math_run(id: &str, inp: &dyn Inputs, p: &Json) -> Option<Result<ToolResult, String>> {
    use gfr::mathfn::{Binary, Unary, binary, unary};
    if let Some((_, name, parse, _)) = MATH_UNARY.iter().find(|t| t.0 == id) {
        let op = Unary::parse(parse)?;
        return Some((|| {
            let a = operand(inp, text(p, "in1"))?;
            let out = eval_operands(inp, p, &[&a], &|o| match &o[0] {
                gfr::local::Operand::Raster(r) => Some(unary(op, r)),
                gfr::local::Operand::Constant(_) => None,
            })
            .map_err(|e| if e.contains("overlap") { format!("{name} needs a raster, not a number") } else { e })?;
            one(p, format!("{}_{}", base_name(inp, p, "in1"), name.replace(' ', "")), out)
        })());
    }
    if let Some((_, name, parse, _)) = MATH_BINARY.iter().find(|t| t.0 == id) {
        let op = Binary::parse(parse)?;
        return Some((|| {
            let (a, b) = (operand(inp, text(p, "in1"))?, operand(inp, text(p, "in2"))?);
            let out = eval_operands(inp, p, &[&a, &b], &|o| binary(op, &o[0], &o[1])).map_err(|e| {
                if e.contains("overlap") { format!("{name}: the operands do not overlap, or an operand is not an integer") } else { e }
            })?;
            one(p, format!("{}_{}", base_name(inp, p, "in1"), name.replace(' ', "")), out)
        })());
    }
    None
}

// ---------------------------------------------------------------------------------------------
// aggregate, block statistics, slice, pick, region group

fn aggregate(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (r, _) = input(inp, p, "input")?;
    let factor =
        number(p, "cell_factor").filter(|f| *f >= 2.0 && f.fract() == 0.0).ok_or("'Cell factor' is a whole number of 2 or more")? as usize;
    let stat = gfr::blockops::AggregateStat::parse(&choice(p, "aggregation_type", "SUM"))
        .ok_or_else(|| format!("aggregation type {}", text(p, "aggregation_type")))?;
    let expand = match choice(p, "extent_handling", "EXPAND").as_str() {
        "EXPAND" => true,
        "TRUNCATE" => false,
        other => return Err(format!("extent handling {other}")),
    };
    let ignore = match choice(p, "ignore_nodata", "DATA").as_str() {
        "DATA" => true,
        "NODATA" => false,
        other => return Err(format!("ignore NoData {other}")),
    };
    let out = gfr::blockops::aggregate(&r, factor, stat, expand, ignore).ok_or("the raster is smaller than one block")?;
    one(p, format!("{}_Aggr", base_name(inp, p, "input")), out)
}

fn block_statistics(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (r, _) = input(inp, p, "input")?;
    let nb = gfr::blockops::block_neighborhood(
        if text(p, "neighborhood").is_empty() { "Rectangle 3 3 CELL" } else { text(p, "neighborhood") },
        r.cell_width(), r.cell_height(),
    ).ok_or("neighborhood: use Rectangle W H CELL|MAP, Circle R CELL|MAP or Annulus R1 R2 CELL|MAP; circular MAP neighborhoods require square cells")?;
    let stat = gfr::focal::FocalStatistic::parse(&choice(p, "statistics_type", "MEAN"))
        .ok_or_else(|| format!("statistic {}", text(p, "statistics_type")))?;
    gfr::blockops::block_check(stat, r.data_type)?;
    let ignore = match choice(p, "ignore_nodata", "DATA").as_str() {
        "DATA" => true,
        "NODATA" => false,
        other => return Err(format!("ignore NoData {other}")),
    };
    let out = gfr::blockops::block_statistics(&r, nb, stat, ignore).ok_or("neighborhood")?;
    one(p, format!("{}_Block", base_name(inp, p, "input")), out)
}

fn slice(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (r, crs) = input(inp, p, "input")?;
    let zones = number(p, "number_zones").filter(|z| *z >= 1.0 && z.fract() == 0.0).ok_or("'Number of zones' is a whole number")? as usize;
    if choice(p, "slice_type", "EQUAL_INTERVAL") != "EQUAL_INTERVAL" {
        return Err("only EQUAL_INTERVAL is done: equal area, natural breaks and the others are not".into());
    }
    let base = number(p, "base_output_zone").unwrap_or(1.0) as i64;
    let out = eval(inp, p, &[&r], crs, Reads::Cropped, Sampling::Nearest, &|x| {
        gfr::blockops::slice_equal_interval(&x[0], zones, base).unwrap_or_else(|| x[0].clone())
    })?;
    one(p, format!("{}_Slice", base_name(inp, p, "input")), out)
}

fn pick(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (pos, _) = input(inp, p, "position")?;
    let names: Vec<String> = text(p, "inputs").split(';').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
    if names.is_empty() {
        return Err("name the rasters or numbers to pick from, separated by semicolons".into());
    }
    let ops: Vec<gfr::local::Operand> = names.iter().map(|n| operand(inp, n)).collect::<Result<_, _>>()?;
    let out = gfr::blockops::pick(&pos, &ops).ok_or("the rasters must share the position raster's grid")?;
    one(p, format!("{}_Pick", base_name(inp, p, "position")), out)
}

fn region_group(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (r, crs) = input(inp, p, "input")?;
    let eight = match choice(p, "number_neighbors", "FOUR").as_str() {
        "FOUR" => false,
        "EIGHT" => true,
        other => return Err(format!("number of neighbors {other}")),
    };
    let cross = match choice(p, "zone_connectivity", "WITHIN").as_str() {
        "WITHIN" => false,
        "CROSS" => true,
        other => return Err(format!("zone connectivity {other}")),
    };
    let excluded = number(p, "excluded_value");
    let out =
        eval(inp, p, &[&r], crs, Reads::Cropped, Sampling::Nearest, &|x| gfr::generalize::region_group(&x[0], eight, cross, excluded))?;
    one(p, format!("{}_Region", base_name(inp, p, "input")), out)
}

/// Expand (`grow`) or Shrink.
fn expand_shrink(inp: &dyn Inputs, p: &Json, grow: bool) -> Result<ToolResult, String> {
    use gfr::generalize::{Method, number_of_cells};
    let (r, _) = input(inp, p, "input")?;
    let n = number_of_cells(number(p, "number_cells").ok_or("'Number of cells' is required")?).map_err(|e| e.0)?;
    let zones: Vec<f64> = text(p, "zone_values")
        .split(';')
        .map(str::trim)
        .filter(|z| !z.is_empty())
        .map(|z| z.parse::<f64>().map_err(|_| format!("zone value {z}")))
        .collect::<Result<_, _>>()?;
    let method = Method::parse(&choice(p, "method", "MORPHOLOGICAL")).ok_or("method is MORPHOLOGICAL or DISTANCE")?;
    let out = if grow { gfr::generalize::expand(&r, n, &zones, method) } else { gfr::generalize::shrink(&r, n, &zones, method) }
        .map_err(|e| e.0)?;
    one(p, format!("{}_{}", base_name(inp, p, "input"), if grow { "Expand" } else { "Shrink" }), out)
}

fn expand(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    expand_shrink(inp, p, true)
}

fn shrink(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    expand_shrink(inp, p, false)
}

fn majority_filter(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (r, _) = input(inp, p, "input")?;
    let eight = match choice(p, "number_neighbors", "FOUR").as_str() {
        "FOUR" => false,
        "EIGHT" => true,
        other => return Err(format!("number of neighbors {other}")),
    };
    let half = match choice(p, "majority_definition", "MAJORITY").as_str() {
        "MAJORITY" => false,
        "HALF" => true,
        other => return Err(format!("majority definition {other}")),
    };
    let out = gfr::generalize::majority_filter(&r, eight, half).map_err(|e| e.0)?;
    one(p, format!("{}_Majority", base_name(inp, p, "input")), out)
}

/// Inverse Distance Weighted interpolation of a point layer's field (Spatial Analyst Idw).
fn idw(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    use gfr::interpolate::{Sample, SearchRadius};
    let (ds, fc) = layer(inp, p, "input")?;
    let field = text(p, "z_field");
    let index = ds.field_index(field).ok_or_else(|| format!("field '{field}' is missing"))?;
    let mut samples = Vec::new();
    for f in &fc.features {
        let z = match f.attributes.get(index) {
            Some(unigis_core::Value::BigInteger(v)) => *v as f64,
            Some(unigis_core::Value::Double(v)) => *v,
            _ => continue,
        };
        if let Some(g) = &f.geometry
            && matches!(g.kind, unigis_core::GeometryKind::Point | unigis_core::GeometryKind::Multipoint)
        {
            samples.extend(g.xy.iter().map(|c| Sample { x: c[0], y: c[1], z }));
        }
    }
    if samples.is_empty() {
        return Err(format!("no point has a value in '{field}'"));
    }
    let mut env = env_of(inp, p, Crs::LonLat)?;
    // The tool's own cell size wins over the environment's.
    if let Some(c) = number(p, "cell_size") {
        env.cell_size = Some(c).filter(|c| *c > 0.0).or(env.cell_size);
        if number(p, "cell_size").is_some_and(|c| c <= 0.0) {
            return Err("'Cell size' must be a positive number (in degrees)".into());
        }
    }
    let power = number(p, "power").unwrap_or(2.0);
    if !(power.is_finite() && power > 0.0) {
        return Err("'Power' must be positive".into());
    }
    let search = match choice(p, "search_type", "VARIABLE").as_str() {
        "VARIABLE" => SearchRadius::Variable {
            points: number(p, "num_points").map_or(12, |n| n.max(1.0) as usize),
            max_distance: number(p, "max_distance").filter(|d| *d > 0.0),
        },
        "FIXED" => SearchRadius::Fixed {
            distance: number(p, "fixed_distance").filter(|d| *d > 0.0),
            min_points: number(p, "min_points").map_or(0, |n| n.max(0.0) as usize),
        },
        other => return Err(format!("search radius type {other}")),
    };
    let out = gfr::interpolate::idw(&samples, power, search, &env, Some(4326))
        .ok_or("no output grid: check the cell size and the extent environment")?;
    one(p, format!("{}_Idw", text(p, "input")), out)
}

fn zonal_statistics_as_table(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (zones, _) = input(inp, p, "zones")?;
    let (values, _) = input(inp, p, "input")?;
    let stats = gfr::zonal::table_statistics(&choice(p, "statistics_type", "ALL"))
        .ok_or_else(|| format!("statistic {}", text(p, "statistics_type")))?;
    let (zones, values) = gfr::resample::zones_on_value_grid(&zones, &values).ok_or("zone and value rasters do not overlap")?;
    let (fields, rows) = gfr::zonal::zonal_table(&zones, &values, &stats, data_flag(p, "ignore_nodata")?);
    let t = table_dataset(rustgis_tools_table(fields, rows), out_name(p, format!("{}_ZonalTable", text(p, "zones"))));
    let m = vec![format!("{}: {} zones", t.name, t.features.len())];
    Ok(ToolResult { output: Output::Table(t), messages: m })
}

fn rustgis_tools_table(fields: Vec<unigis_core::Field>, rows: Vec<Vec<unigis_core::Value>>) -> unigis_tools::TableOutput {
    unigis_tools::TableOutput { fields, rows }
}

pub(crate) const DEFS: &[Def] = &[
    Def {
        id: "aggregate",
        name: "Aggregate",
        toolbox: "UniGIS › Spatial Analyst › Generalization",
        summary: "A coarser raster from blocks of cells, counted from the lower-left corner (ArcGIS Pro Aggregate).",
        params: &[
            RASTER,
            P {
                name: "cell_factor",
                label: "Cell factor",
                kind: K::Number,
                required: true,
                default: "",
                help: "A whole number of 2 or more.",
            },
            P {
                name: "aggregation_type",
                label: "Aggregation technique",
                kind: K::Choice(&["SUM", "MEAN", "MEDIAN", "MAXIMUM", "MINIMUM"]),
                required: false,
                default: "\"SUM\"",
                help: "",
            },
            P {
                name: "extent_handling",
                label: "Extent handling",
                kind: K::Choice(&["EXPAND", "TRUNCATE"]),
                required: false,
                default: "\"EXPAND\"",
                help: "",
            },
            P {
                name: "ignore_nodata",
                label: "Ignore NoData in calculations",
                kind: K::Choice(&["DATA", "NODATA"]),
                required: false,
                default: "\"DATA\"",
                help: "",
            },
            OUTPUT,
        ],
        run: aggregate,
    },
    Def {
        id: "block_statistics",
        name: "Block Statistics",
        toolbox: "UniGIS › Spatial Analyst › Neighborhood",
        summary: "A statistic of each non-overlapping block, given to all its cells (ArcGIS Pro Block Statistics).",
        params: &[
            RASTER,
            P {
                name: "neighborhood",
                label: "Neighborhood",
                kind: K::Text,
                required: false,
                default: "\"Rectangle 3 3 CELL\"",
                help: "Rectangle W H CELL|MAP, Circle R CELL|MAP or Annulus R1 R2 CELL|MAP. Circular MAP neighborhoods require square cells.",
            },
            P {
                name: "statistics_type",
                label: "Statistics type",
                kind: K::Choice(&["MEAN", "MAJORITY", "MAXIMUM", "MEDIAN", "MINIMUM", "MINORITY", "RANGE", "STD", "SUM", "VARIETY"]),
                required: false,
                default: "\"MEAN\"",
                help: "",
            },
            P {
                name: "ignore_nodata",
                label: "Ignore NoData in calculations",
                kind: K::Choice(&["DATA", "NODATA"]),
                required: false,
                default: "\"DATA\"",
                help: "",
            },
            OUTPUT,
        ],
        run: block_statistics,
    },
    Def {
        id: "slice",
        name: "Slice",
        toolbox: "UniGIS › Spatial Analyst › Reclass",
        summary: "Cuts the values into zones of equal interval (ArcGIS Pro Slice; the other methods are not done).",
        params: &[
            RASTER,
            P { name: "number_zones", label: "Number of output zones", kind: K::Number, required: true, default: "", help: "" },
            P {
                name: "slice_type",
                label: "Slice type",
                kind: K::Choice(&["EQUAL_INTERVAL"]),
                required: false,
                default: "\"EQUAL_INTERVAL\"",
                help: "",
            },
            P {
                name: "base_output_zone",
                label: "Base output zone",
                kind: K::Number,
                required: false,
                default: "1",
                help: "The number of the lowest zone.",
            },
            OUTPUT,
        ],
        run: slice,
    },
    Def {
        id: "pick",
        name: "Pick",
        toolbox: "UniGIS › Spatial Analyst › Local",
        summary: "Each cell takes its value from the input its position raster names (ArcGIS Pro Pick).",
        params: &[
            P { name: "position", label: "Input position raster", kind: K::RasterLayer, required: true, default: "", help: "" },
            P {
                name: "inputs",
                label: "Input rasters or constants",
                kind: K::Text,
                required: true,
                default: "",
                help: "Raster layers or numbers separated by semicolons; position 1 is the first.",
            },
            OUTPUT,
        ],
        run: pick,
    },
    Def {
        id: "region_group",
        name: "Region Group",
        toolbox: "UniGIS › Spatial Analyst › Generalization",
        summary: "Numbers the connected regions (ArcGIS Pro Region Group).",
        params: &[
            RASTER,
            P {
                name: "number_neighbors",
                label: "Number of neighbors to use",
                kind: K::Choice(&["FOUR", "EIGHT"]),
                required: false,
                default: "\"FOUR\"",
                help: "",
            },
            P {
                name: "zone_connectivity",
                label: "Zone grouping method",
                kind: K::Choice(&["WITHIN", "CROSS"]),
                required: false,
                default: "\"WITHIN\"",
                help: "",
            },
            P {
                name: "excluded_value",
                label: "Excluded value",
                kind: K::Number,
                required: false,
                default: "",
                help: "Cells of this value join no region (0 in the output).",
            },
            OUTPUT,
        ],
        run: region_group,
    },
    Def {
        id: "expand",
        name: "Expand",
        toolbox: "UniGIS › Spatial Analyst › Generalization",
        summary: "Grows zones by a number of cells (ArcGIS Pro Expand).",
        params: &[
            RASTER,
            P {
                name: "number_cells",
                label: "Number of cells",
                kind: K::Number,
                required: true,
                default: "",
                help: "Rounded half to even; at least 1.",
            },
            P {
                name: "zone_values",
                label: "Zone values",
                kind: K::Text,
                required: true,
                default: "",
                help: "Values separated by semicolons.",
            },
            P {
                name: "method",
                label: "Method",
                kind: K::Choice(&["MORPHOLOGICAL", "DISTANCE"]),
                required: false,
                default: "\"MORPHOLOGICAL\"",
                help: "",
            },
            OUTPUT,
        ],
        run: expand,
    },
    Def {
        id: "shrink",
        name: "Shrink",
        toolbox: "UniGIS › Spatial Analyst › Generalization",
        summary: "Shrinks zones by a number of cells (ArcGIS Pro Shrink).",
        params: &[
            RASTER,
            P {
                name: "number_cells",
                label: "Number of cells",
                kind: K::Number,
                required: true,
                default: "",
                help: "Rounded half to even; at least 1.",
            },
            P {
                name: "zone_values",
                label: "Zone values",
                kind: K::Text,
                required: true,
                default: "",
                help: "Values separated by semicolons.",
            },
            P {
                name: "method",
                label: "Method",
                kind: K::Choice(&["MORPHOLOGICAL", "DISTANCE"]),
                required: false,
                default: "\"MORPHOLOGICAL\"",
                help: "",
            },
            OUTPUT,
        ],
        run: shrink,
    },
    Def {
        id: "majority_filter",
        name: "Majority Filter",
        toolbox: "UniGIS › Spatial Analyst › Generalization",
        summary: "Replaces cells by the majority of their contiguous neighbors (ArcGIS Pro Majority Filter).",
        params: &[
            RASTER,
            P {
                name: "number_neighbors",
                label: "Number of neighbors to use",
                kind: K::Choice(&["FOUR", "EIGHT"]),
                required: false,
                default: "\"FOUR\"",
                help: "",
            },
            P {
                name: "majority_definition",
                label: "Replacement threshold",
                kind: K::Choice(&["MAJORITY", "HALF"]),
                required: false,
                default: "\"MAJORITY\"",
                help: "",
            },
            OUTPUT,
        ],
        run: majority_filter,
    },
    Def {
        id: "cell_statistics",
        name: "Cell Statistics",
        toolbox: "UniGIS › Spatial Analyst › Local",
        summary: "A statistic of each cell across several rasters (ArcGIS Pro Cell Statistics).",
        params: &[
            P {
                name: "inputs",
                label: "Input rasters",
                kind: K::Text,
                required: true,
                default: "",
                help: "Raster layer names separated by semicolons.",
            },
            P {
                name: "statistic",
                label: "Overlay statistic",
                kind: K::Choice(&["MEAN", "MAJORITY", "MAXIMUM", "MEDIAN", "MINIMUM", "MINORITY", "RANGE", "STD", "SUM", "VARIETY"]),
                required: false,
                default: "\"MEAN\"",
                help: "",
            },
            P {
                name: "ignore_nodata",
                label: "Ignore NoData",
                kind: K::Choice(&["DATA", "NODATA"]),
                required: false,
                default: "\"DATA\"",
                help: "DATA: use the cells that have values. NODATA: a cell with any NoData input is NoData.",
            },
            OUTPUT,
        ],
        run: cell_statistics,
    },
    Def {
        id: "weighted_sum",
        name: "Weighted Sum",
        toolbox: "UniGIS › Spatial Analyst › Overlay",
        summary: "Weights and adds rasters cell by cell (ArcGIS Pro Weighted Sum).",
        params: &[
            P {
                name: "inputs",
                label: "Input rasters",
                kind: K::Text,
                required: true,
                default: "",
                help: "Raster layer names separated by semicolons.",
            },
            P {
                name: "weights",
                label: "Weights",
                kind: K::Text,
                required: true,
                default: "",
                help: "One number per raster, separated by semicolons.",
            },
            OUTPUT,
        ],
        run: weighted_sum,
    },
    Def {
        id: "highest_position",
        name: "Highest Position",
        toolbox: "UniGIS › Spatial Analyst › Local",
        summary: "The position of the raster with the highest value in each cell (ArcGIS Pro Highest Position).",
        params: &[
            P {
                name: "inputs",
                label: "Input rasters",
                kind: K::Text,
                required: true,
                default: "",
                help: "Raster layer names separated by semicolons.",
            },
            OUTPUT,
        ],
        run: highest_position,
    },
    Def {
        id: "lowest_position",
        name: "Lowest Position",
        toolbox: "UniGIS › Spatial Analyst › Local",
        summary: "The position of the raster with the lowest value in each cell (ArcGIS Pro Lowest Position).",
        params: &[
            P {
                name: "inputs",
                label: "Input rasters",
                kind: K::Text,
                required: true,
                default: "",
                help: "Raster layer names separated by semicolons.",
            },
            OUTPUT,
        ],
        run: lowest_position,
    },
    Def {
        id: "combine",
        name: "Combine",
        toolbox: "UniGIS › Spatial Analyst › Local",
        summary: "A new value for each unique combination of the input values (ArcGIS Pro Combine).",
        params: &[
            P {
                name: "inputs",
                label: "Input rasters",
                kind: K::Text,
                required: true,
                default: "",
                help: "Raster layer names separated by semicolons.",
            },
            OUTPUT,
        ],
        run: combine,
    },
    Def {
        id: "weighted_overlay",
        name: "Weighted Overlay",
        toolbox: "UniGIS › Spatial Analyst › Overlay",
        summary: "Overlays rasters on a common scale, each weighted by its influence (ArcGIS Pro Weighted Overlay).",
        params: &[
            P {
                name: "table",
                label: "Weighted overlay table",
                kind: K::Text,
                required: true,
                default: "",
                help: "layer:influence:from=to,from=to entries separated by semicolons; `to` is a scale value or NODATA. The influences add up to 100.",
            },
            P {
                name: "evaluation_scale",
                label: "Evaluation scale",
                kind: K::Text,
                required: false,
                default: "\"1 9 1\"",
                help: "From, to, by.",
            },
            OUTPUT,
        ],
        run: weighted_overlay,
    },
    Def {
        id: "fuzzy_membership",
        name: "Fuzzy Membership",
        toolbox: "UniGIS › Spatial Analyst › Overlay",
        summary: "Rescales a raster to 0–1, the strength of membership in a set (ArcGIS Pro Fuzzy Membership).",
        params: &[
            RASTER,
            P {
                name: "function",
                label: "Membership type",
                kind: K::Choice(&["LINEAR", "GAUSSIAN", "LARGE", "SMALL", "NEAR", "MSLARGE", "MSSMALL"]),
                required: false,
                default: "\"LINEAR\"",
                help: "",
            },
            P { name: "minimum", label: "Minimum (linear)", kind: K::Number, required: false, default: "", help: "" },
            P {
                name: "maximum",
                label: "Maximum (linear)",
                kind: K::Number,
                required: false,
                default: "",
                help: "Smaller than the minimum for a decreasing function.",
            },
            P { name: "midpoint", label: "Midpoint", kind: K::Number, required: false, default: "", help: "Gaussian, Large, Small, Near." },
            P {
                name: "spread",
                label: "Spread",
                kind: K::Number,
                required: false,
                default: "",
                help: "Default 0.1 (Gaussian, Near) or 5 (Large, Small).",
            },
            P { name: "mean_multiplier", label: "Mean multiplier (MS)", kind: K::Number, required: false, default: "1", help: "" },
            P {
                name: "std_multiplier",
                label: "Standard deviation multiplier (MS)",
                kind: K::Number,
                required: false,
                default: "1",
                help: "",
            },
            P {
                name: "hedge",
                label: "Hedge",
                kind: K::Choice(&["NONE", "SOMEWHAT", "VERY"]),
                required: false,
                default: "\"NONE\"",
                help: "Somewhat: square root. Very: square.",
            },
            OUTPUT,
        ],
        run: fuzzy_membership,
    },
    Def {
        id: "fuzzy_overlay",
        name: "Fuzzy Overlay",
        toolbox: "UniGIS › Spatial Analyst › Overlay",
        summary: "Combines fuzzy membership rasters (ArcGIS Pro Fuzzy Overlay).",
        params: &[
            P {
                name: "inputs",
                label: "Input rasters",
                kind: K::Text,
                required: true,
                default: "",
                help: "Raster layer names separated by semicolons.",
            },
            P {
                name: "overlay_type",
                label: "Overlay type",
                kind: K::Choice(&["AND", "OR", "PRODUCT", "SUM", "GAMMA"]),
                required: false,
                default: "\"AND\"",
                help: "",
            },
            P { name: "gamma", label: "Gamma", kind: K::Number, required: false, default: "", help: "0 to 1, for GAMMA." },
            OUTPUT,
        ],
        run: fuzzy_overlay,
    },
    Def {
        id: "idw",
        name: "IDW",
        toolbox: "UniGIS › Spatial Analyst › Interpolation",
        summary: "Interpolates a surface from points by inverse distance weighting; the cell size is in degrees (ArcGIS Pro IDW).",
        params: &[
            P { name: "input", label: "Input point features", kind: K::PointLayer, required: true, default: "", help: "" },
            P { name: "z_field", label: "Z value field", kind: K::NumericField, required: true, default: "", help: "" },
            P {
                name: "cell_size",
                label: "Output cell size",
                kind: K::Number,
                required: false,
                default: "",
                help: "In degrees. Default: the shorter side of the points' extent / 250.",
            },
            P {
                name: "power",
                label: "Power",
                kind: K::Number,
                required: false,
                default: "2",
                help: "The exponent of the distance; a higher power favours near points.",
            },
            P {
                name: "search_type",
                label: "Search radius type",
                kind: K::Choice(&["VARIABLE", "FIXED"]),
                required: false,
                default: "\"VARIABLE\"",
                help: "Variable: the nearest points. Fixed: the points within a distance.",
            },
            P { name: "num_points", label: "Number of points (variable)", kind: K::Number, required: false, default: "12", help: "" },
            P {
                name: "max_distance",
                label: "Maximum distance (variable)",
                kind: K::Number,
                required: false,
                default: "",
                help: "In degrees. Default: the output extent's width plus height.",
            },
            P {
                name: "fixed_distance",
                label: "Distance (fixed)",
                kind: K::Number,
                required: false,
                default: "",
                help: "In degrees. Default: five cells.",
            },
            P {
                name: "min_points",
                label: "Minimum number of points (fixed)",
                kind: K::Number,
                required: false,
                default: "0",
                help: "A cell with fewer points inside widens the radius a cell at a time.",
            },
            OUTPUT,
        ],
        run: idw,
    },
    Def {
        id: "slope",
        name: "Slope",
        toolbox: "UniGIS › Spatial Analyst › Surface",
        summary: "Steepness of each cell of a surface (ArcGIS Pro Slope).",
        params: &[
            RASTER,
            P {
                name: "output_measurement",
                label: "Output measurement",
                kind: K::Choice(&["DEGREE", "PERCENT_RISE"]),
                required: false,
                default: "\"DEGREE\"",
                help: "",
            },
            P { name: "z_factor", label: "Z factor", kind: K::Number, required: false, default: "1", help: "" },
            OUTPUT,
        ],
        run: slope,
    },
    Def {
        id: "aspect",
        name: "Aspect",
        toolbox: "UniGIS › Spatial Analyst › Surface",
        summary: "Compass direction the slope faces (ArcGIS Pro Aspect).",
        params: &[RASTER, OUTPUT],
        run: aspect,
    },
    Def {
        id: "hillshade",
        name: "Hillshade",
        toolbox: "UniGIS › Spatial Analyst › Surface",
        summary: "Shaded relief of a surface (ArcGIS Pro Hillshade).",
        params: &[
            RASTER,
            P { name: "azimuth", label: "Azimuth", kind: K::Number, required: false, default: "315", help: "" },
            P { name: "altitude", label: "Altitude", kind: K::Number, required: false, default: "45", help: "" },
            P { name: "model_shadows", label: "Model shadows", kind: K::Bool, required: false, default: "false", help: "" },
            P { name: "z_factor", label: "Z factor", kind: K::Number, required: false, default: "1", help: "" },
            OUTPUT,
        ],
        run: hillshade,
    },
    Def {
        id: "curvature",
        name: "Curvature",
        toolbox: "UniGIS › Spatial Analyst › Surface",
        summary: "Curvature of a surface, with optional profile and plan curvature (ArcGIS Pro Curvature).",
        params: &[
            RASTER,
            P { name: "z_factor", label: "Z factor", kind: K::Number, required: false, default: "1", help: "" },
            P { name: "profile", label: "Also profile curvature", kind: K::Bool, required: false, default: "false", help: "" },
            P { name: "plan", label: "Also plan curvature", kind: K::Bool, required: false, default: "false", help: "" },
            OUTPUT,
        ],
        run: curvature,
    },
    Def {
        id: "contour",
        name: "Contour",
        toolbox: "UniGIS › Spatial Analyst › Surface",
        summary: "Lines of equal value of a surface (ArcGIS Pro Contour).",
        params: &[
            RASTER,
            P { name: "contour_interval", label: "Contour interval", kind: K::Number, required: true, default: "", help: "" },
            P { name: "base_contour", label: "Base contour", kind: K::Number, required: false, default: "0", help: "" },
            OUTPUT,
        ],
        run: contour,
    },
    Def {
        id: "fill",
        name: "Fill",
        toolbox: "UniGIS › Spatial Analyst › Hydrology",
        summary: "Fills sinks in a surface (ArcGIS Pro Fill).",
        params: &[RASTER, P { name: "z_limit", label: "Z limit", kind: K::Number, required: false, default: "", help: "" }, OUTPUT],
        run: fill,
    },
    Def {
        id: "flow_direction",
        name: "Flow Direction",
        toolbox: "UniGIS › Spatial Analyst › Hydrology",
        summary: "Direction of steepest descent from each cell, D8 (ArcGIS Pro Flow Direction).",
        params: &[
            RASTER,
            P {
                name: "force_flow",
                label: "Force all edge cells to flow outward",
                kind: K::Choice(&["NORMAL", "FORCE"]),
                required: false,
                default: "\"NORMAL\"",
                help: "",
            },
            OUTPUT,
        ],
        run: flow_direction,
    },
    Def {
        id: "flow_accumulation",
        name: "Flow Accumulation",
        toolbox: "UniGIS › Spatial Analyst › Hydrology",
        summary: "Accumulated weight of the cells that flow into each cell (ArcGIS Pro Flow Accumulation).",
        params: &[
            P { name: "input", label: "Flow direction raster", kind: K::RasterLayer, required: true, default: "", help: "" },
            P { name: "weight", label: "Weight raster", kind: K::RasterLayer, required: false, default: "", help: "" },
            P {
                name: "data_type",
                label: "Output data type",
                kind: K::Choice(&["FLOAT", "INTEGER"]),
                required: false,
                default: "\"FLOAT\"",
                help: "",
            },
            OUTPUT,
        ],
        run: flow_accumulation,
    },
    Def {
        id: "basin",
        name: "Basin",
        toolbox: "UniGIS › Spatial Analyst › Hydrology",
        summary: "Drainage basins (ArcGIS Pro Basin).",
        params: &[P { name: "input", label: "Flow direction raster", kind: K::RasterLayer, required: true, default: "", help: "" }, OUTPUT],
        run: basin,
    },
    Def {
        id: "watershed",
        name: "Watershed",
        toolbox: "UniGIS › Spatial Analyst › Hydrology",
        summary: "The area draining to pour points (ArcGIS Pro Watershed).",
        params: &[
            P { name: "input", label: "Flow direction raster", kind: K::RasterLayer, required: true, default: "", help: "" },
            P { name: "pour_points", label: "Pour point raster", kind: K::RasterLayer, required: true, default: "", help: "" },
            OUTPUT,
        ],
        run: watershed,
    },
    Def {
        id: "stream_link",
        name: "Stream Link",
        toolbox: "UniGIS › Spatial Analyst › Hydrology",
        summary: "Numbers the stretches of a stream network (ArcGIS Pro Stream Link).",
        params: &[
            P { name: "streams", label: "Stream raster", kind: K::RasterLayer, required: true, default: "", help: "" },
            P { name: "input", label: "Flow direction raster", kind: K::RasterLayer, required: true, default: "", help: "" },
            OUTPUT,
        ],
        run: stream_link,
    },
    Def {
        id: "stream_order",
        name: "Stream Order",
        toolbox: "UniGIS › Spatial Analyst › Hydrology",
        summary: "Strahler or Shreve order of stream segments (ArcGIS Pro Stream Order).",
        params: &[
            P { name: "streams", label: "Stream raster", kind: K::RasterLayer, required: true, default: "", help: "" },
            P { name: "input", label: "Flow direction raster", kind: K::RasterLayer, required: true, default: "", help: "" },
            P {
                name: "order_method",
                label: "Order method",
                kind: K::Choice(&["STRAHLER", "SHREVE"]),
                required: false,
                default: "\"STRAHLER\"",
                help: "",
            },
            OUTPUT,
        ],
        run: stream_order,
    },
    Def {
        id: "flow_length",
        name: "Flow Length",
        toolbox: "UniGIS › Spatial Analyst › Hydrology",
        summary: "Distance along the flow path, downstream or upstream (ArcGIS Pro Flow Length).",
        params: &[
            P { name: "input", label: "Flow direction raster", kind: K::RasterLayer, required: true, default: "", help: "" },
            P {
                name: "direction_measurement",
                label: "Direction",
                kind: K::Choice(&["DOWNSTREAM", "UPSTREAM"]),
                required: false,
                default: "\"DOWNSTREAM\"",
                help: "",
            },
            OUTPUT,
        ],
        run: flow_length,
    },
    Def {
        id: "focal_statistics",
        name: "Focal Statistics",
        toolbox: "UniGIS › Spatial Analyst › Neighborhood",
        summary: "A statistic over each cell's neighborhood (ArcGIS Pro Focal Statistics).",
        params: &[
            RASTER,
            P {
                name: "neighborhood",
                label: "Neighborhood",
                kind: K::Text,
                required: false,
                default: "\"Rectangle 3 3 CELL\"",
                help: "Rectangle W H CELL|MAP, Circle R CELL|MAP, Annulus R1 R2 CELL|MAP",
            },
            P {
                name: "statistics_type",
                label: "Statistic",
                kind: K::Choice(&["MEAN", "MAJORITY", "MAXIMUM", "MEDIAN", "MINIMUM", "MINORITY", "RANGE", "STD", "SUM", "VARIETY"]),
                required: false,
                default: "\"MEAN\"",
                help: "",
            },
            P {
                name: "ignore_nodata",
                label: "Ignore NoData",
                kind: K::Choice(&["DATA", "NODATA"]),
                required: false,
                default: "\"DATA\"",
                help: "",
            },
            OUTPUT,
        ],
        run: focal_statistics,
    },
    Def {
        id: "reclassify",
        name: "Reclassify",
        toolbox: "UniGIS › Spatial Analyst › Reclass",
        summary: "Changes cell values by a table of old to new values (ArcGIS Pro Reclassify).",
        params: &[
            RASTER,
            P { name: "remap", label: "Reclass table", kind: K::Text, required: true, default: "", help: "e.g. 0 10 1;10 20 2;NODATA 0" },
            P {
                name: "missing_values",
                label: "Missing values",
                kind: K::Choice(&["DATA", "NODATA"]),
                required: false,
                default: "\"DATA\"",
                help: "",
            },
            OUTPUT,
        ],
        run: reclassify,
    },
    Def {
        id: "zonal_statistics",
        name: "Zonal Statistics",
        toolbox: "UniGIS › Spatial Analyst › Zonal",
        summary: "A statistic of the values in each zone, as a raster (ArcGIS Pro Zonal Statistics).",
        params: &[
            P { name: "zones", label: "Zone raster", kind: K::RasterLayer, required: true, default: "", help: "" },
            P { name: "input", label: "Value raster", kind: K::RasterLayer, required: true, default: "", help: "" },
            P {
                name: "statistics_type",
                label: "Statistic",
                kind: K::Choice(&["MEAN", "MAJORITY", "MAXIMUM", "MEDIAN", "MINIMUM", "MINORITY", "RANGE", "STD", "SUM", "VARIETY"]),
                required: false,
                default: "\"MEAN\"",
                help: "",
            },
            P {
                name: "ignore_nodata",
                label: "Ignore NoData",
                kind: K::Choice(&["DATA", "NODATA"]),
                required: false,
                default: "\"DATA\"",
                help: "",
            },
            OUTPUT,
        ],
        run: zonal_statistics,
    },
    Def {
        id: "resample",
        name: "Resample",
        toolbox: "UniGIS › Data Management › Raster",
        summary: "Changes the cell size of a raster (ArcGIS Pro Resample).",
        params: &[
            RASTER,
            P {
                name: "cell_size",
                label: "Output cell size",
                kind: K::Number,
                required: true,
                default: "",
                help: "In the raster's units.",
            },
            P {
                name: "resampling_type",
                label: "Resampling technique",
                kind: K::Choice(&["NEAREST", "BILINEAR", "CUBIC", "MAJORITY"]),
                required: false,
                default: "\"NEAREST\"",
                help: "",
            },
            OUTPUT,
        ],
        run: resample,
    },
    Def {
        id: "euclidean_distance",
        name: "Euclidean Distance",
        toolbox: "UniGIS › Spatial Analyst › Distance",
        summary: "Distance from each cell to the nearest source cell (ArcGIS Pro Euclidean Distance).",
        params: &[
            RASTER,
            P { name: "maximum_distance", label: "Maximum distance", kind: K::Number, required: false, default: "", help: "" },
            OUTPUT,
        ],
        run: euclidean_distance,
    },
    Def {
        id: "euclidean_direction",
        name: "Euclidean Direction",
        toolbox: "UniGIS › Spatial Analyst › Distance",
        summary: "Azimuth in degrees from each cell to the nearest source cell (ArcGIS Pro Euclidean Direction).",
        params: &[
            RASTER,
            P { name: "maximum_distance", label: "Maximum distance", kind: K::Number, required: false, default: "", help: "" },
            OUTPUT,
        ],
        run: euclidean_direction,
    },
    Def {
        id: "euclidean_allocation",
        name: "Euclidean Allocation",
        toolbox: "UniGIS › Spatial Analyst › Distance",
        summary: "Value of the nearest source cell for each cell (ArcGIS Pro Euclidean Allocation).",
        params: &[
            RASTER,
            P { name: "maximum_distance", label: "Maximum distance", kind: K::Number, required: false, default: "", help: "" },
            OUTPUT,
        ],
        run: euclidean_allocation,
    },
    Def {
        id: "extract_by_mask",
        name: "Extract by Mask",
        toolbox: "UniGIS › Spatial Analyst › Extraction",
        summary: "The cells of a raster that fall within a mask raster (ArcGIS Pro Extract by Mask).",
        params: &[RASTER, P { name: "mask", label: "Mask raster", kind: K::RasterLayer, required: true, default: "", help: "" }, OUTPUT],
        run: extract_by_mask,
    },
    Def {
        id: "plus",
        name: "Plus",
        toolbox: "UniGIS › Spatial Analyst › Math",
        summary: "Adds two rasters or a raster and a number (ArcGIS Pro Plus).",
        params: &[
            P {
                name: "in1",
                label: "Input raster or constant 1",
                kind: K::Text,
                required: true,
                default: "",
                help: "A raster layer name or a number.",
            },
            P { name: "in2", label: "Input raster or constant 2", kind: K::Text, required: true, default: "", help: "" },
            OUTPUT,
        ],
        run: plus,
    },
    Def {
        id: "minus",
        name: "Minus",
        toolbox: "UniGIS › Spatial Analyst › Math",
        summary: "Subtracts the second raster or number from the first (ArcGIS Pro Minus).",
        params: &[
            P {
                name: "in1",
                label: "Input raster or constant 1",
                kind: K::Text,
                required: true,
                default: "",
                help: "A raster layer name or a number.",
            },
            P { name: "in2", label: "Input raster or constant 2", kind: K::Text, required: true, default: "", help: "" },
            OUTPUT,
        ],
        run: minus,
    },
    Def {
        id: "times",
        name: "Times",
        toolbox: "UniGIS › Spatial Analyst › Math",
        summary: "Multiplies two rasters or a raster and a number (ArcGIS Pro Times).",
        params: &[
            P {
                name: "in1",
                label: "Input raster or constant 1",
                kind: K::Text,
                required: true,
                default: "",
                help: "A raster layer name or a number.",
            },
            P { name: "in2", label: "Input raster or constant 2", kind: K::Text, required: true, default: "", help: "" },
            OUTPUT,
        ],
        run: times,
    },
    Def {
        id: "divide",
        name: "Divide",
        toolbox: "UniGIS › Spatial Analyst › Math",
        summary: "Divides the first raster or number by the second (ArcGIS Pro Divide).",
        params: &[
            P {
                name: "in1",
                label: "Input raster or constant 1",
                kind: K::Text,
                required: true,
                default: "",
                help: "A raster layer name or a number.",
            },
            P { name: "in2", label: "Input raster or constant 2", kind: K::Text, required: true, default: "", help: "" },
            OUTPUT,
        ],
        run: divide,
    },
    Def {
        id: "int",
        name: "Int",
        toolbox: "UniGIS › Spatial Analyst › Math",
        summary: "Cell values truncated to integers (ArcGIS Pro Int).",
        params: &[RASTER, OUTPUT],
        run: int,
    },
    Def {
        id: "float",
        name: "Float",
        toolbox: "UniGIS › Spatial Analyst › Math",
        summary: "Cell values as floating point (ArcGIS Pro Float).",
        params: &[RASTER, OUTPUT],
        run: float,
    },
    Def {
        id: "is_null",
        name: "Is Null",
        toolbox: "UniGIS › Spatial Analyst › Math",
        summary: "1 where a cell is NoData, else 0 (ArcGIS Pro Is Null).",
        params: &[RASTER, OUTPUT],
        run: is_null,
    },
    Def {
        id: "con",
        name: "Con",
        toolbox: "UniGIS › Spatial Analyst › Conditional",
        summary: "A cell takes the true value where the condition holds, else the false value (ArcGIS Pro Con).",
        params: &[
            P { name: "input", label: "Conditional raster", kind: K::RasterLayer, required: true, default: "", help: "" },
            P { name: "true_value", label: "True raster or constant", kind: K::Text, required: true, default: "", help: "" },
            P { name: "false_value", label: "False raster or constant", kind: K::Text, required: false, default: "", help: "" },
            P { name: "where_clause", label: "Expression", kind: K::Text, required: false, default: "", help: "e.g. VALUE > 100" },
            OUTPUT,
        ],
        run: con,
    },
    Def {
        id: "set_null",
        name: "Set Null",
        toolbox: "UniGIS › Spatial Analyst › Conditional",
        summary: "NoData where the condition holds, else the false value (ArcGIS Pro Set Null).",
        params: &[
            P { name: "input", label: "Conditional raster", kind: K::RasterLayer, required: true, default: "", help: "" },
            P { name: "false_value", label: "False raster or constant", kind: K::Text, required: true, default: "", help: "" },
            P { name: "where_clause", label: "Expression", kind: K::Text, required: false, default: "", help: "e.g. VALUE < 0" },
            OUTPUT,
        ],
        run: set_null,
    },
    Def {
        id: "raster_to_point",
        name: "Raster To Point",
        toolbox: "UniGIS › Conversion › From Raster",
        summary: "A point at the center of each cell with data (ArcGIS Pro Raster To Point).",
        params: &[RASTER, OUTPUT],
        run: raster_to_point,
    },
    Def {
        id: "raster_to_polygon",
        name: "Raster To Polygon",
        toolbox: "UniGIS › Conversion › From Raster",
        summary: "Polygons of connected cells of the same integer value (ArcGIS Pro Raster To Polygon, NO_SIMPLIFY).",
        params: &[RASTER, OUTPUT],
        run: raster_to_polygon,
    },
    Def {
        id: "polygon_to_raster",
        name: "Polygon To Raster",
        toolbox: "UniGIS › Conversion › To Raster",
        summary: "Polygons to cells carrying a field's value; the cell size is in degrees (ArcGIS Pro Polygon To Raster).",
        params: &[
            INPUT,
            P {
                name: "value_field",
                label: "Value field",
                kind: K::Field,
                required: false,
                default: "",
                help: "A numeric field; empty = the feature id.",
            },
            P {
                name: "cell_assignment",
                label: "Cell assignment type",
                kind: K::Choice(&["CELL_CENTER", "MAXIMUM_AREA", "MAXIMUM_COMBINED_AREA"]),
                required: false,
                default: "\"CELL_CENTER\"",
                help: "",
            },
            P { name: "cell_size", label: "Cell size", kind: K::Number, required: true, default: "", help: "In degrees." },
            OUTPUT,
        ],
        run: polygon_to_raster,
    },
    Def {
        id: "polyline_to_raster",
        name: "Polyline To Raster",
        toolbox: "UniGIS › Conversion › To Raster",
        summary: "Lines to cells carrying a field's value; the cell size is in degrees (ArcGIS Pro Polyline To Raster).",
        params: &[
            INPUT,
            P {
                name: "value_field",
                label: "Value field",
                kind: K::Field,
                required: false,
                default: "",
                help: "A numeric field; empty = the feature id.",
            },
            P {
                name: "cell_assignment",
                label: "Cell assignment type",
                kind: K::Choice(&["MAXIMUM_LENGTH", "MAXIMUM_COMBINED_LENGTH"]),
                required: false,
                default: "\"MAXIMUM_LENGTH\"",
                help: "",
            },
            P { name: "cell_size", label: "Cell size", kind: K::Number, required: true, default: "", help: "In degrees." },
            OUTPUT,
        ],
        run: polyline_to_raster,
    },
    Def {
        id: "point_to_raster",
        name: "Point To Raster",
        toolbox: "UniGIS › Conversion › To Raster",
        summary: "Points to cells carrying a field's value; the cell size is in degrees (ArcGIS Pro Point To Raster).",
        params: &[
            INPUT,
            P {
                name: "value_field",
                label: "Value field",
                kind: K::Field,
                required: false,
                default: "",
                help: "A numeric field; empty = the feature id.",
            },
            P {
                name: "cell_assignment",
                label: "Cell assignment type",
                kind: K::Choice(&["MOST_FREQUENT", "SUM", "MEAN", "MAXIMUM", "MINIMUM", "COUNT", "RANGE", "STANDARD_DEVIATION"]),
                required: false,
                default: "\"MOST_FREQUENT\"",
                help: "",
            },
            P { name: "cell_size", label: "Cell size", kind: K::Number, required: true, default: "", help: "In degrees." },
            OUTPUT,
        ],
        run: point_to_raster,
    },
    Def {
        id: "zonal_statistics_as_table",
        name: "Zonal Statistics as Table",
        toolbox: "UniGIS › Spatial Analyst › Zonal",
        summary: "A table of statistics of the values in each zone (ArcGIS Pro Zonal Statistics as Table).",
        params: &[
            P { name: "zones", label: "Zone raster", kind: K::RasterLayer, required: true, default: "", help: "" },
            P { name: "input", label: "Value raster", kind: K::RasterLayer, required: true, default: "", help: "" },
            P {
                name: "statistics_type",
                label: "Statistics type",
                kind: K::Text,
                required: false,
                default: "\"ALL\"",
                help: "ALL, MEAN, MEDIAN, MIN_MAX_MEAN, ... as ArcGIS names them.",
            },
            P {
                name: "ignore_nodata",
                label: "Ignore NoData",
                kind: K::Choice(&["DATA", "NODATA"]),
                required: false,
                default: "\"DATA\"",
                help: "",
            },
            OUTPUT,
        ],
        run: zonal_statistics_as_table,
    },
];
