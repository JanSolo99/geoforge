//! GeoForge's ArcGIS-verified raster tools (`geoforge-raster`) as GeoForge tools.
//!
//! Modified for GeoForge (not part of upstream GeoForge).
//!
//! A raster is read from its GeoTIFF (the file it came from, or the bytes a project embeds) so
//! that the pixel type and NoData value are the file's; a raster from another format is converted
//! from the pixels the app holds. Results are written as GeoTIFFs with ArcGIS's tiling and
//! compression and come back as raster layers. Rasters keep their own coordinate system; vector
//! results (contours, points, polygons) are projected to longitude/latitude like every layer.

use std::path::{Path, PathBuf};

use gfr::env::{Reads, Sampling};
use std::sync::atomic::{AtomicU64, Ordering};

use geoforge_raster as gfr;
use rustgis_analysis::{Inputs, Output, OutputLayer, ParamKind as K, ParamSpec as P, ToolResult};
use rustgis_data::raster::Crs;
use rustgis_data::{Dataset, Field, FieldType, Geometry, GeometryType, Value};
use rustgis_geom::Coord;
use serde_json::Value as Json;

use crate::gf;
use crate::{Def, INPUT, OUTPUT, flag, layer, number, out_name, table_dataset, text};

const RASTER: P = P { name: "input", label: "Input Raster", kind: K::RasterLayer, required: true, default: "", help: "" };

/// A scratch file in the temp directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(ext: &str) -> Scratch {
        static N: AtomicU64 = AtomicU64::new(0);
        let name = format!("geoforge-{}-{}.{ext}", std::process::id(), N.fetch_add(1, Ordering::Relaxed));
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

/// The raster the app holds, as GeoForge's model.
fn convert(r: &rustgis_data::raster::Raster) -> gfr::Raster {
    let t = &r.transform;
    let band = r.bands.first().map(|b| b.data.iter().map(|v| f64::from(*v)).collect()).unwrap_or_default();
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
        bands: vec![band],
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
            | "extract_by_mask"
            | "zonal_statistics"
            | "raster_to_point"
            | "raster_to_polygon"
            | "polygon_to_raster"
            | "polyline_to_raster"
            | "point_to_raster"
            | "zonal_statistics_as_table"
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

type Rasterized = (Vec<(geoforge_core::Geometry, f64)>, gfr::rasterize::ValueKind);

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
                Some(geoforge_core::Value::BigInteger(v)) => Some(*v as f64),
                Some(geoforge_core::Value::Double(v)) => Some(*v),
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
fn cell_size(p: &Json, features: &[(geoforge_core::Geometry, f64)]) -> Result<f64, String> {
    let cell = number(p, "cell_size").filter(|c| *c > 0.0).ok_or("'Cell size' must be a positive number (in degrees)")?;
    let mut e = geoforge_core::Envelope::EMPTY;
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
    features: &[(geoforge_core::Geometry, f64)],
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

/// Inverse Distance Weighted interpolation of a point layer's field (Spatial Analyst Idw).
fn idw(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    use gfr::interpolate::{Sample, SearchRadius};
    let (ds, fc) = layer(inp, p, "input")?;
    let field = text(p, "z_field");
    let index = ds.field_index(field).ok_or_else(|| format!("field '{field}' is missing"))?;
    let mut samples = Vec::new();
    for f in &fc.features {
        let z = match f.attributes.get(index) {
            Some(geoforge_core::Value::BigInteger(v)) => *v as f64,
            Some(geoforge_core::Value::Double(v)) => *v,
            _ => continue,
        };
        if let Some(g) = &f.geometry
            && matches!(g.kind, geoforge_core::GeometryKind::Point | geoforge_core::GeometryKind::Multipoint)
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

fn rustgis_tools_table(fields: Vec<geoforge_core::Field>, rows: Vec<Vec<geoforge_core::Value>>) -> geoforge_tools::TableOutput {
    geoforge_tools::TableOutput { fields, rows }
}

pub(crate) const DEFS: &[Def] = &[
    Def {
        id: "idw",
        name: "IDW",
        toolbox: "GeoForge › Spatial Analyst › Interpolation",
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
        toolbox: "GeoForge › Spatial Analyst › Surface",
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
        toolbox: "GeoForge › Spatial Analyst › Surface",
        summary: "Compass direction the slope faces (ArcGIS Pro Aspect).",
        params: &[RASTER, OUTPUT],
        run: aspect,
    },
    Def {
        id: "hillshade",
        name: "Hillshade",
        toolbox: "GeoForge › Spatial Analyst › Surface",
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
        toolbox: "GeoForge › Spatial Analyst › Surface",
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
        toolbox: "GeoForge › Spatial Analyst › Surface",
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
        toolbox: "GeoForge › Spatial Analyst › Hydrology",
        summary: "Fills sinks in a surface (ArcGIS Pro Fill).",
        params: &[RASTER, P { name: "z_limit", label: "Z limit", kind: K::Number, required: false, default: "", help: "" }, OUTPUT],
        run: fill,
    },
    Def {
        id: "flow_direction",
        name: "Flow Direction",
        toolbox: "GeoForge › Spatial Analyst › Hydrology",
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
        toolbox: "GeoForge › Spatial Analyst › Hydrology",
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
        toolbox: "GeoForge › Spatial Analyst › Hydrology",
        summary: "Drainage basins (ArcGIS Pro Basin).",
        params: &[P { name: "input", label: "Flow direction raster", kind: K::RasterLayer, required: true, default: "", help: "" }, OUTPUT],
        run: basin,
    },
    Def {
        id: "watershed",
        name: "Watershed",
        toolbox: "GeoForge › Spatial Analyst › Hydrology",
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
        toolbox: "GeoForge › Spatial Analyst › Hydrology",
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
        toolbox: "GeoForge › Spatial Analyst › Hydrology",
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
        toolbox: "GeoForge › Spatial Analyst › Hydrology",
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
        toolbox: "GeoForge › Spatial Analyst › Neighborhood",
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
        toolbox: "GeoForge › Spatial Analyst › Reclass",
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
        toolbox: "GeoForge › Spatial Analyst › Zonal",
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
        toolbox: "GeoForge › Data Management › Raster",
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
        toolbox: "GeoForge › Spatial Analyst › Distance",
        summary: "Distance from each cell to the nearest source cell (ArcGIS Pro Euclidean Distance).",
        params: &[
            RASTER,
            P { name: "maximum_distance", label: "Maximum distance", kind: K::Number, required: false, default: "", help: "" },
            OUTPUT,
        ],
        run: euclidean_distance,
    },
    Def {
        id: "extract_by_mask",
        name: "Extract by Mask",
        toolbox: "GeoForge › Spatial Analyst › Extraction",
        summary: "The cells of a raster that fall within a mask raster (ArcGIS Pro Extract by Mask).",
        params: &[RASTER, P { name: "mask", label: "Mask raster", kind: K::RasterLayer, required: true, default: "", help: "" }, OUTPUT],
        run: extract_by_mask,
    },
    Def {
        id: "plus",
        name: "Plus",
        toolbox: "GeoForge › Spatial Analyst › Math",
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
        toolbox: "GeoForge › Spatial Analyst › Math",
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
        toolbox: "GeoForge › Spatial Analyst › Math",
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
        toolbox: "GeoForge › Spatial Analyst › Math",
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
        toolbox: "GeoForge › Spatial Analyst › Math",
        summary: "Cell values truncated to integers (ArcGIS Pro Int).",
        params: &[RASTER, OUTPUT],
        run: int,
    },
    Def {
        id: "float",
        name: "Float",
        toolbox: "GeoForge › Spatial Analyst › Math",
        summary: "Cell values as floating point (ArcGIS Pro Float).",
        params: &[RASTER, OUTPUT],
        run: float,
    },
    Def {
        id: "is_null",
        name: "Is Null",
        toolbox: "GeoForge › Spatial Analyst › Math",
        summary: "1 where a cell is NoData, else 0 (ArcGIS Pro Is Null).",
        params: &[RASTER, OUTPUT],
        run: is_null,
    },
    Def {
        id: "con",
        name: "Con",
        toolbox: "GeoForge › Spatial Analyst › Conditional",
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
        toolbox: "GeoForge › Spatial Analyst › Conditional",
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
        toolbox: "GeoForge › Conversion › From Raster",
        summary: "A point at the center of each cell with data (ArcGIS Pro Raster To Point).",
        params: &[RASTER, OUTPUT],
        run: raster_to_point,
    },
    Def {
        id: "raster_to_polygon",
        name: "Raster To Polygon",
        toolbox: "GeoForge › Conversion › From Raster",
        summary: "Polygons of connected cells of the same integer value (ArcGIS Pro Raster To Polygon, NO_SIMPLIFY).",
        params: &[RASTER, OUTPUT],
        run: raster_to_polygon,
    },
    Def {
        id: "polygon_to_raster",
        name: "Polygon To Raster",
        toolbox: "GeoForge › Conversion › To Raster",
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
        toolbox: "GeoForge › Conversion › To Raster",
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
        toolbox: "GeoForge › Conversion › To Raster",
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
        toolbox: "GeoForge › Spatial Analyst › Zonal",
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
