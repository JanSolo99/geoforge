//! Add GPS Metadata Fields: schema from public ArcGIS Pro 3.7 controls.
use crate::registry::{Context, Kind, Param, Report, Runner, SchemaDomain, SchemaDomainKind, Tool};
use unigis_core::{Error, Field, FieldType, GeometryKind, Result, Value};

type Spec = (&'static str, &'static str, FieldType, u32, &'static str);

const POINT: &[Spec] = &[
    (
        "ESRIGNSS_POSITIONSOURCETYPE",
        "Position source type",
        FieldType::SmallInteger,
        2,
        "ESRI_POSITIONSOURCETYPE_DOMAIN",
    ),
    (
        "ESRIGNSS_RECEIVER",
        "Receiver Name",
        FieldType::String,
        50,
        "",
    ),
    ("ESRIGNSS_LATITUDE", "Latitude", FieldType::Double, 8, ""),
    ("ESRIGNSS_LONGITUDE", "Longitude", FieldType::Double, 8, ""),
    ("ESRIGNSS_ALTITUDE", "Altitude", FieldType::Double, 8, ""),
    (
        "ESRIGNSS_H_RMS",
        "Horizontal Accuracy (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_V_RMS",
        "Vertical Accuracy (m)",
        FieldType::Double,
        8,
        "",
    ),
    ("ESRIGNSS_FIXDATETIME", "Fix Time", FieldType::Date, 8, ""),
    (
        "ESRIGNSS_FIXTYPE",
        "Fix Type",
        FieldType::SmallInteger,
        2,
        "ESRI_FIX_TYPE_DOMAIN",
    ),
    (
        "ESRIGNSS_CORRECTIONAGE",
        "Correction Age",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_STATIONID",
        "Station ID",
        FieldType::SmallInteger,
        2,
        "",
    ),
    (
        "ESRIGNSS_NUMSATS",
        "Number of Satellites",
        FieldType::SmallInteger,
        2,
        "ESRI_NUM_SATS_DOMAIN",
    ),
    ("ESRIGNSS_PDOP", "PDOP", FieldType::Double, 8, ""),
    ("ESRIGNSS_HDOP", "HDOP", FieldType::Double, 8, ""),
    ("ESRIGNSS_VDOP", "VDOP", FieldType::Double, 8, ""),
    (
        "ESRIGNSS_DIRECTION",
        "Direction of travel (°)",
        FieldType::Double,
        8,
        "",
    ),
    ("ESRIGNSS_SPEED", "Speed (km/h)", FieldType::Double, 8, ""),
    (
        "ESRISNSR_AZIMUTH",
        "Compass reading (°)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_AVG_H_RMS",
        "Average Horizontal Accuracy (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_AVG_V_RMS",
        "Average Vertical Accuracy (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_AVG_POSITIONS",
        "Averaged Positions",
        FieldType::SmallInteger,
        2,
        "",
    ),
    (
        "ESRIGNSS_H_STDDEV",
        "Standard Deviation (m)",
        FieldType::Double,
        8,
        "",
    ),
];

const OFFSET: &[Spec] = &[
    (
        "ESRISNSR_OFFSET_METHOD",
        "Offset method",
        FieldType::String,
        255,
        "ESRI_OFFSET_METHOD_DOMAIN",
    ),
    (
        "ESRISNSR_OFFSET_DEVICE",
        "Rangefinder name",
        FieldType::String,
        255,
        "",
    ),
    (
        "ESRIGNSS_ANTENNA_HEIGHT",
        "GNSS Antenna height (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRISNSR_OFFSET_LASER_HEIGHT",
        "Rangefinder height (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRISNSR_OFFSET_LASER_TARGET_HT",
        "Laser target height (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRISNSR_OFFSET_MAG_VAR",
        "Rangefinder Magnetic variation (°)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_CP1_LATITUDE",
        "CP1 Latitude",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_CP1_LONGITUDE",
        "CP1 Longitude",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_CP1_ALTITUDE",
        "CP1 Ellipsoidal height (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_CP1_H_RMS",
        "CP1 Horizontal accuracy (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_CP1_V_RMS",
        "CP1 Vertical accuracy (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_CP1_FIXDATETIME",
        "CP1 Fix time",
        FieldType::Date,
        8,
        "",
    ),
    (
        "ESRIGNSS_CP1_FIXTYPE",
        "CP1 Fix type",
        FieldType::SmallInteger,
        2,
        "ESRI_FIX_TYPE_DOMAIN",
    ),
    (
        "ESRIGNSS_CP1_CORR_AGE",
        "CP1 Correction age",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_CP1_STATIONID",
        "CP1 Station ID",
        FieldType::SmallInteger,
        2,
        "",
    ),
    (
        "ESRIGNSS_CP1_NUMSATS",
        "CP1 Number of satellites",
        FieldType::Double,
        8,
        "",
    ),
    ("ESRIGNSS_CP1_PDOP", "CP1 PDOP", FieldType::Double, 8, ""),
    ("ESRIGNSS_CP1_HDOP", "CP1 HDOP", FieldType::Double, 8, ""),
    ("ESRIGNSS_CP1_VDOP", "CP1 VDOP", FieldType::Double, 8, ""),
    (
        "ESRIGNSS_CP1_AVG_H_RMS",
        "CP1 Average horizontal accuracy (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_CP1_AVG_V_RMS",
        "CP1 Average vertical accuracy (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_CP1_AVG_POSITIONS",
        "CP1 Averaged positions",
        FieldType::SmallInteger,
        2,
        "",
    ),
    (
        "ESRIGNSS_CP1_H_STDDEV",
        "CP1 Standard deviation (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRISNSR_CP1_SLOPE_DIST",
        "CP1 Laser slope distance (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRISNSR_CP1_INCLINATION",
        "CP1 Laser inclination (°)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRISNSR_CP1_AZIMUTH",
        "CP1 Laser azimuth (°)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_CP2_LATITUDE",
        "CP2 Latitude",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_CP2_LONGITUDE",
        "CP2 Longitude",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_CP2_ALTITUDE",
        "CP2 Ellipsoidal height (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_CP2_H_RMS",
        "CP2 Horizontal accuracy (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_CP2_V_RMS",
        "CP2 Vertical accuracy (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_CP2_FIXDATETIME",
        "CP2 Fix time",
        FieldType::Date,
        8,
        "",
    ),
    (
        "ESRIGNSS_CP2_FIXTYPE",
        "CP2 Fix type",
        FieldType::SmallInteger,
        2,
        "ESRI_FIX_TYPE_DOMAIN",
    ),
    (
        "ESRIGNSS_CP2_CORR_AGE",
        "CP2 Correction age",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_CP2_STATIONID",
        "CP2 Station ID",
        FieldType::SmallInteger,
        2,
        "",
    ),
    (
        "ESRIGNSS_CP2_NUMSATS",
        "CP2 Number of satellites",
        FieldType::Double,
        8,
        "",
    ),
    ("ESRIGNSS_CP2_PDOP", "CP2 PDOP", FieldType::Double, 8, ""),
    ("ESRIGNSS_CP2_HDOP", "CP2 HDOP", FieldType::Double, 8, ""),
    ("ESRIGNSS_CP2_VDOP", "CP2 VDOP", FieldType::Double, 8, ""),
    (
        "ESRIGNSS_CP2_AVG_H_RMS",
        "CP2 Average horizontal accuracy (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_CP2_AVG_V_RMS",
        "CP2 Average vertical accuracy (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_CP2_AVG_POSITIONS",
        "CP2 Averaged positions",
        FieldType::SmallInteger,
        2,
        "",
    ),
    (
        "ESRIGNSS_CP2_H_STDDEV",
        "CP2 Standard deviation (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRISNSR_CP2_SLOPE_DIST",
        "CP2 Laser slope distance (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRISNSR_CP2_INCLINATION",
        "CP2 Laser inclination (°)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRISNSR_CP2_AZIMUTH",
        "CP2 Laser azimuth (°)",
        FieldType::Double,
        8,
        "",
    ),
];

const LINE: &[Spec] = &[
    (
        "ESRIGNSS_AVG_H_RMS",
        "Average Horizontal Accuracy (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_AVG_V_RMS",
        "Average Vertical Accuracy (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_WORST_H_RMS",
        "Worst Horizontal Accuracy (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_WORST_V_RMS",
        "Worst Vertical Accuracy (m)",
        FieldType::Double,
        8,
        "",
    ),
    (
        "ESRIGNSS_WORST_FIXTYPE",
        "Worst Fix Type",
        FieldType::SmallInteger,
        2,
        "ESRI_FIX_TYPE_DOMAIN",
    ),
    (
        "ESRIGNSS_MANUAL_LOCATIONS",
        "Number of Manual Locations",
        FieldType::Integer,
        4,
        "",
    ),
];

/// Full desired schema; hosts preserve already present field definitions and values.
pub fn schema(kind: GeometryKind, offset: bool) -> (Vec<Field>, Vec<SchemaDomain>) {
    let point = kind == GeometryKind::Point;
    let specs = if point { POINT } else { LINE };
    let fields = specs
        .iter()
        .chain(if point && offset { OFFSET } else { &[] })
        .map(|&(name, alias, field_type, length, domain)| {
            let mut f = Field::new(name, field_type);
            f.alias = Some(alias.into());
            f.length = length;
            f.domain = (!domain.is_empty()).then(|| domain.into());
            f
        })
        .collect();
    let mut domains = Vec::new();
    if point {
        domains.push(SchemaDomain {
            name: "ESRI_NUM_SATS_DOMAIN",
            description: "Number of Satellites",
            field_type: FieldType::SmallInteger,
            kind: SchemaDomainKind::Range(Value::SmallInteger(0), Value::SmallInteger(99)),
        });
    }
    if point {
        domains.push(SchemaDomain {
            name: "ESRI_POSITIONSOURCETYPE_DOMAIN",
            description: "Position Source Type",
            field_type: FieldType::SmallInteger,
            kind: SchemaDomainKind::Coded(vec![
                (Value::SmallInteger(0), "Unknown".into()),
                (Value::SmallInteger(1), "User defined".into()),
                (
                    Value::SmallInteger(2),
                    "Integrated (System) Location Provider".into(),
                ),
                (Value::SmallInteger(3), "External GNSS Receiver".into()),
                (Value::SmallInteger(4), "Network Location Provider".into()),
                (Value::SmallInteger(5), "Snapped".into()),
            ]),
        });
    }
    domains.push(SchemaDomain {
        name: "ESRI_FIX_TYPE_DOMAIN",
        description: "Fix Type",
        field_type: FieldType::SmallInteger,
        kind: SchemaDomainKind::Coded(vec![
            (Value::SmallInteger(0), "Fix not valid".into()),
            (Value::SmallInteger(1), "GPS".into()),
            (Value::SmallInteger(2), "Differential GPS".into()),
            (Value::SmallInteger(4), "RTK Fixed".into()),
            (Value::SmallInteger(5), "RTK Float".into()),
        ]),
    });
    if point && offset {
        domains.push(SchemaDomain {
            name: "ESRI_OFFSET_METHOD_DOMAIN",
            description: "Offset Method Domain",
            field_type: FieldType::String,
            kind: SchemaDomainKind::Coded(vec![
                (
                    Value::String("azimuth-azimuth".into()),
                    "Azimuth-Azimuth".into(),
                ),
                (
                    Value::String("distance-angle".into()),
                    "Distance-Angle".into(),
                ),
                (
                    Value::String("distance-azimuth".into()),
                    "Distance-Azimuth".into(),
                ),
                (
                    Value::String("distance-distance".into()),
                    "Distance-Distance".into(),
                ),
                (
                    Value::String("azimuth-azimuth-backsight".into()),
                    "Azimuth-Azimuth-Backsight".into(),
                ),
                (
                    Value::String("distance-azimuth-backsight".into()),
                    "Distance-Azimuth-Backsight".into(),
                ),
                (
                    Value::String("distance-distance-backsight".into()),
                    "Distance-Distance-Backsight".into(),
                ),
                (Value::String("user-defined".into()), "User-Defined".into()),
            ]),
        });
    }
    (fields, domains)
}

pub const TOOL: Tool = Tool {
    id: "add_gps_metadata_fields",
    name: "Add GPS Metadata Fields",
    toolbox: "Data Management › GPS",
    summary: "Adds GNSS metadata fields and domains to a geodatabase feature class in place.",
    params: &[
        Param {
            name: "input",
            label: "Input Features",
            kind: Kind::Features,
            required: true,
            default: "",
            help: "Geodatabase feature class to update in place.",
        },
        Param {
            name: "include_offset_fields",
            label: "Include offset metadata fields",
            kind: Kind::Choice(&["INCLUDE_OFFSET_FIELDS", "EXCLUDE_OFFSET_FIELDS"]),
            required: false,
            default: "EXCLUDE_OFFSET_FIELDS",
            help: "Offset fields apply to point features only.",
        },
    ],
    output_suffix: "gps",
    measures: false,
    run: Runner::Report(run),
};

fn run(ctx: &dyn Context) -> Result<Report> {
    let input = ctx.features("input")?;
    let kind = input.geometry_kind().ok_or_else(|| {
        Error::Unsupported("ERROR 000732: Input Features: dataset is not supported".into())
    })?;
    if kind == GeometryKind::MultiPatch {
        return Err(Error::Unsupported(
            "ERROR 000732: Input Features: MultiPatch is not supported".into(),
        ));
    }
    let offset = ctx
        .get("include_offset_fields")
        .is_some_and(|v| v.eq_ignore_ascii_case("INCLUDE_OFFSET_FIELDS"));
    let (fields, domains) = schema(kind, offset);
    drop(input);
    ctx.add_schema_fields("input", &fields, &domains)?;
    Ok(Report {
        rows: Vec::new(),
        messages: vec!["GPS metadata fields updated".into()],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ToolOutput, registry, tables::Mem};
    use unigis_core::FeatureClass;

    #[test]
    fn read_only_host_never_reports_unwritten_schema_as_success() {
        struct ReadOnly;
        impl Context for ReadOnly {
            fn get(&self, name: &str) -> Option<String> {
                (name == "input").then(|| "point".into())
            }
            fn features(&self, _: &str) -> Result<Box<dyn FeatureClass + '_>> {
                Ok(Box::new(Mem(ToolOutput {
                    kind: GeometryKind::Point,
                    has_z: false,
                    has_m: false,
                    fields: vec![],
                    spatial_reference_wkt: None,
                    features: vec![],
                    messages: vec![],
                })))
            }
        }
        let Err(error) = registry::run(&TOOL, &ReadOnly) else {
            panic!("host must fail");
        };
        assert!(
            error
                .to_string()
                .contains("schema edits are not supported by this host")
        );
    }
}
