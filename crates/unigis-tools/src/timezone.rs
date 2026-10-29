//! Convert Time Zone (Data Management): date values from one time zone to another.
//!
//! ArcGIS names Windows time zones; [`ZONES`] maps them to IANA zones (CLDR's windowsZones
//! table, as in Python's tzlocal), whose rules ArcGIS Pro 3.7's results follow.

use chrono::{Duration, NaiveDateTime, Offset, TimeZone};
use chrono_tz::{OffsetComponents, Tz};
use unigis_core::{Error, FeatureClass, Field, FieldType, Result, Value};

use crate::ToolOutput;

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

/// ArcGIS's time zone names (the Windows ones) and the IANA zones with their rules. ArcGIS also
/// lists `Mid-Atlantic Standard Time` and `Kamchatka Standard Time`, which fail there (ERROR
/// 999999) and are refused here.
const ZONES: &[(&str, &str)] = &[
    ("UTC", "Etc/UTC"),
    ("Dateline Standard Time", "Etc/GMT+12"),
    ("UTC-11", "Etc/GMT+11"),
    ("Aleutian Standard Time", "America/Adak"),
    ("Hawaiian Standard Time", "Pacific/Honolulu"),
    ("Marquesas Standard Time", "Pacific/Marquesas"),
    ("Alaskan Standard Time", "America/Anchorage"),
    ("UTC-09", "Etc/GMT+9"),
    ("Pacific Standard Time (Mexico)", "America/Tijuana"),
    ("UTC-08", "Etc/GMT+8"),
    ("Pacific Standard Time", "America/Los_Angeles"),
    ("US Mountain Standard Time", "America/Phoenix"),
    ("Mountain Standard Time (Mexico)", "America/Mazatlan"),
    ("Mountain Standard Time", "America/Denver"),
    ("Yukon Standard Time", "America/Whitehorse"),
    ("Central America Standard Time", "America/Guatemala"),
    ("Central Standard Time", "America/Chicago"),
    ("Easter Island Standard Time", "Pacific/Easter"),
    ("Central Standard Time (Mexico)", "America/Mexico_City"),
    ("Canada Central Standard Time", "America/Regina"),
    ("SA Pacific Standard Time", "America/Bogota"),
    ("Eastern Standard Time (Mexico)", "America/Cancun"),
    ("Eastern Standard Time", "America/New_York"),
    ("Haiti Standard Time", "America/Port-au-Prince"),
    ("Cuba Standard Time", "America/Havana"),
    ("US Eastern Standard Time", "America/Indianapolis"),
    ("Turks And Caicos Standard Time", "America/Grand_Turk"),
    ("Atlantic Standard Time", "America/Halifax"),
    ("Venezuela Standard Time", "America/Caracas"),
    ("Central Brazilian Standard Time", "America/Cuiaba"),
    ("SA Western Standard Time", "America/La_Paz"),
    ("Pacific SA Standard Time", "America/Santiago"),
    ("Newfoundland Standard Time", "America/St_Johns"),
    ("Tocantins Standard Time", "America/Araguaina"),
    ("Paraguay Standard Time", "America/Asuncion"),
    ("E. South America Standard Time", "America/Sao_Paulo"),
    ("SA Eastern Standard Time", "America/Cayenne"),
    ("Argentina Standard Time", "America/Buenos_Aires"),
    ("Montevideo Standard Time", "America/Montevideo"),
    ("Magallanes Standard Time", "America/Punta_Arenas"),
    ("Saint Pierre Standard Time", "America/Miquelon"),
    ("Bahia Standard Time", "America/Bahia"),
    ("UTC-02", "Etc/GMT+2"),
    ("Greenland Standard Time", "America/Godthab"),
    ("Azores Standard Time", "Atlantic/Azores"),
    ("Cape Verde Standard Time", "Atlantic/Cape_Verde"),
    ("GMT Standard Time", "Europe/London"),
    ("Greenwich Standard Time", "Atlantic/Reykjavik"),
    ("Sao Tome Standard Time", "Africa/Sao_Tome"),
    ("Morocco Standard Time", "Africa/Casablanca"),
    ("W. Europe Standard Time", "Europe/Berlin"),
    ("Central Europe Standard Time", "Europe/Budapest"),
    ("Romance Standard Time", "Europe/Paris"),
    ("Central European Standard Time", "Europe/Warsaw"),
    ("W. Central Africa Standard Time", "Africa/Lagos"),
    ("GTB Standard Time", "Europe/Bucharest"),
    ("Middle East Standard Time", "Asia/Beirut"),
    ("Egypt Standard Time", "Africa/Cairo"),
    ("E. Europe Standard Time", "Europe/Chisinau"),
    ("West Bank Standard Time", "Asia/Hebron"),
    ("South Africa Standard Time", "Africa/Johannesburg"),
    ("FLE Standard Time", "Europe/Kiev"),
    ("Israel Standard Time", "Asia/Jerusalem"),
    ("South Sudan Standard Time", "Africa/Juba"),
    ("Kaliningrad Standard Time", "Europe/Kaliningrad"),
    ("Sudan Standard Time", "Africa/Khartoum"),
    ("Libya Standard Time", "Africa/Tripoli"),
    ("Namibia Standard Time", "Africa/Windhoek"),
    ("Jordan Standard Time", "Asia/Amman"),
    ("Arabic Standard Time", "Asia/Baghdad"),
    ("Syria Standard Time", "Asia/Damascus"),
    ("Turkey Standard Time", "Europe/Istanbul"),
    ("Arab Standard Time", "Asia/Riyadh"),
    ("Belarus Standard Time", "Europe/Minsk"),
    ("Russian Standard Time", "Europe/Moscow"),
    ("E. Africa Standard Time", "Africa/Nairobi"),
    ("Volgograd Standard Time", "Europe/Volgograd"),
    ("Iran Standard Time", "Asia/Tehran"),
    ("Arabian Standard Time", "Asia/Dubai"),
    ("Astrakhan Standard Time", "Europe/Astrakhan"),
    ("Azerbaijan Standard Time", "Asia/Baku"),
    ("Russia Time Zone 3", "Europe/Samara"),
    ("Mauritius Standard Time", "Indian/Mauritius"),
    ("Saratov Standard Time", "Europe/Saratov"),
    ("Georgian Standard Time", "Asia/Tbilisi"),
    ("Caucasus Standard Time", "Asia/Yerevan"),
    ("Afghanistan Standard Time", "Asia/Kabul"),
    ("West Asia Standard Time", "Asia/Tashkent"),
    ("Qyzylorda Standard Time", "Asia/Qyzylorda"),
    ("Ekaterinburg Standard Time", "Asia/Yekaterinburg"),
    ("Pakistan Standard Time", "Asia/Karachi"),
    ("India Standard Time", "Asia/Calcutta"),
    ("Sri Lanka Standard Time", "Asia/Colombo"),
    ("Nepal Standard Time", "Asia/Katmandu"),
    ("Central Asia Standard Time", "Asia/Bishkek"),
    ("Bangladesh Standard Time", "Asia/Dhaka"),
    ("Omsk Standard Time", "Asia/Omsk"),
    ("Myanmar Standard Time", "Asia/Rangoon"),
    ("SE Asia Standard Time", "Asia/Bangkok"),
    ("Altai Standard Time", "Asia/Barnaul"),
    ("W. Mongolia Standard Time", "Asia/Hovd"),
    ("North Asia Standard Time", "Asia/Krasnoyarsk"),
    ("N. Central Asia Standard Time", "Asia/Novosibirsk"),
    ("Tomsk Standard Time", "Asia/Tomsk"),
    ("China Standard Time", "Asia/Shanghai"),
    ("North Asia East Standard Time", "Asia/Irkutsk"),
    ("Singapore Standard Time", "Asia/Singapore"),
    ("W. Australia Standard Time", "Australia/Perth"),
    ("Taipei Standard Time", "Asia/Taipei"),
    ("Ulaanbaatar Standard Time", "Asia/Ulaanbaatar"),
    ("Aus Central W. Standard Time", "Australia/Eucla"),
    ("Transbaikal Standard Time", "Asia/Chita"),
    ("Tokyo Standard Time", "Asia/Tokyo"),
    ("North Korea Standard Time", "Asia/Pyongyang"),
    ("Korea Standard Time", "Asia/Seoul"),
    ("Yakutsk Standard Time", "Asia/Yakutsk"),
    ("Cen. Australia Standard Time", "Australia/Adelaide"),
    ("AUS Central Standard Time", "Australia/Darwin"),
    ("E. Australia Standard Time", "Australia/Brisbane"),
    ("AUS Eastern Standard Time", "Australia/Sydney"),
    ("West Pacific Standard Time", "Pacific/Port_Moresby"),
    ("Tasmania Standard Time", "Australia/Hobart"),
    ("Vladivostok Standard Time", "Asia/Vladivostok"),
    ("Lord Howe Standard Time", "Australia/Lord_Howe"),
    ("Bougainville Standard Time", "Pacific/Bougainville"),
    ("Russia Time Zone 10", "Asia/Srednekolymsk"),
    ("Magadan Standard Time", "Asia/Magadan"),
    ("Norfolk Standard Time", "Pacific/Norfolk"),
    ("Sakhalin Standard Time", "Asia/Sakhalin"),
    ("Central Pacific Standard Time", "Pacific/Guadalcanal"),
    ("Russia Time Zone 11", "Asia/Kamchatka"),
    ("New Zealand Standard Time", "Pacific/Auckland"),
    ("UTC+12", "Etc/GMT-12"),
    ("Fiji Standard Time", "Pacific/Fiji"),
    ("Chatham Islands Standard Time", "Pacific/Chatham"),
    ("UTC+13", "Etc/GMT-13"),
    ("Tonga Standard Time", "Pacific/Tongatapu"),
    ("Samoa Standard Time", "Pacific/Apia"),
    ("Line Islands Standard Time", "Pacific/Kiritimati"),
];

/// The zone of an ArcGIS name, with spaces or underscores (`Pacific_Standard_Time`).
fn zone(name: &str) -> Result<Tz> {
    let wanted = name.trim().replace('_', " ");
    let iana = ZONES
        .iter()
        .find(|(w, _)| w.eq_ignore_ascii_case(&wanted))
        .map(|(_, i)| *i)
        .ok_or_else(|| {
            err(format!(
                "ERROR 000800: The value is not a member of the time zones: {name}"
            ))
        })?;
    iana.parse::<Tz>()
        .map_err(|e| err(format!("time zone {iana}: {e}")))
}

/// The UTC time of local time `t` in `tz`: with `dst` an ambiguous time takes its first (summer)
/// reading and a time in the spring gap the offset after it; without, the standard offset.
fn to_utc(t: NaiveDateTime, tz: Tz, dst: bool) -> NaiveDateTime {
    if !dst {
        let standard = tz.offset_from_utc_datetime(&t).base_utc_offset();
        return t - standard;
    }
    match tz.from_local_datetime(&t) {
        chrono::LocalResult::Single(d) => d.naive_utc(),
        chrono::LocalResult::Ambiguous(first, _) => first.naive_utc(),
        chrono::LocalResult::None => {
            let after = tz.from_local_datetime(&(t + Duration::hours(1))).earliest();
            let offset = after.map_or(0, |d| d.offset().fix().local_minus_utc());
            t - Duration::seconds(i64::from(offset))
        }
    }
}

/// Local time in `tz` of UTC `t`; without `dst` at the standard offset.
fn from_utc(t: NaiveDateTime, tz: Tz, dst: bool) -> NaiveDateTime {
    let local = tz.from_utc_datetime(&t);
    if dst {
        local.naive_local()
    } else {
        t + local.offset().base_utc_offset()
    }
}

/// Convert Time Zone: a new date field `output_name` with `field`'s values (local times in
/// `input_zone`) as local times in `output_zone`. `input_dst`/`output_dst`: whether the times
/// observe daylight saving time (else the standard offset all year).
pub fn convert_time_zone(
    input: &dyn FeatureClass,
    field: &str,
    input_zone: &str,
    output_name: &str,
    output_zone: &str,
    input_dst: bool,
    output_dst: bool,
) -> Result<ToolOutput> {
    let (zin, zout) = (zone(input_zone)?, zone(output_zone)?);
    let fields = input.fields().to_vec();
    let fi = fields
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(field))
        .ok_or_else(|| err(format!("ERROR 000728: Field {field} does not exist")))?;
    if !matches!(fields[fi].field_type, FieldType::Date) {
        return Err(err(format!("ERROR 000800: {field} is not a date field")));
    }
    let mut out_field = Field::new(output_name, FieldType::Date);
    out_field.length = 8;
    let mut features = Vec::new();
    for f in input.features() {
        let mut f = f?;
        let v = match &f.attributes[fi] {
            Value::Date(t) => Value::Date(from_utc(to_utc(*t, zin, input_dst), zout, output_dst)),
            _ => Value::Null,
        };
        f.attributes.push(v);
        features.push(f);
    }
    let mut out_fields = fields;
    out_fields.push(out_field);
    Ok(ToolOutput {
        kind: input
            .geometry_kind()
            .unwrap_or(unigis_core::GeometryKind::Point),
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields: out_fields,
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap()
    }

    /// ArcGIS Pro 3.7 results (memory table, so times are kept).
    #[test]
    fn conversions_match_arcgis() {
        let inputs = [
            "2024-01-15 12:00:00",
            "2024-07-15 12:00:00",
            "2024-03-10 01:30:00",
            "2024-03-10 02:30:00",
            "2024-03-10 03:30:00",
            "2024-11-03 01:30:00",
            "2024-11-03 00:30:00",
            "1999-06-01 23:00:00",
            "2024-12-31 23:30:00",
        ];
        let cases: &[(&str, &str, bool, bool, [&str; 9])] = &[
            (
                "Pacific Standard Time",
                "UTC",
                true,
                true,
                [
                    "2024-01-15 20:00:00",
                    "2024-07-15 19:00:00",
                    "2024-03-10 09:30:00",
                    "2024-03-10 09:30:00",
                    "2024-03-10 10:30:00",
                    "2024-11-03 08:30:00",
                    "2024-11-03 07:30:00",
                    "1999-06-02 06:00:00",
                    "2025-01-01 07:30:00",
                ],
            ),
            (
                "Pacific_Standard_Time",
                "UTC",
                false,
                true,
                [
                    "2024-01-15 20:00:00",
                    "2024-07-15 20:00:00",
                    "2024-03-10 09:30:00",
                    "2024-03-10 10:30:00",
                    "2024-03-10 11:30:00",
                    "2024-11-03 09:30:00",
                    "2024-11-03 08:30:00",
                    "1999-06-02 07:00:00",
                    "2025-01-01 07:30:00",
                ],
            ),
            (
                "UTC",
                "Pacific Standard Time",
                true,
                true,
                [
                    "2024-01-15 04:00:00",
                    "2024-07-15 05:00:00",
                    "2024-03-09 17:30:00",
                    "2024-03-09 18:30:00",
                    "2024-03-09 19:30:00",
                    "2024-11-02 18:30:00",
                    "2024-11-02 17:30:00",
                    "1999-06-01 16:00:00",
                    "2024-12-31 15:30:00",
                ],
            ),
            (
                "UTC",
                "Pacific Standard Time",
                true,
                false,
                [
                    "2024-01-15 04:00:00",
                    "2024-07-15 04:00:00",
                    "2024-03-09 17:30:00",
                    "2024-03-09 18:30:00",
                    "2024-03-09 19:30:00",
                    "2024-11-02 17:30:00",
                    "2024-11-02 16:30:00",
                    "1999-06-01 15:00:00",
                    "2024-12-31 15:30:00",
                ],
            ),
            (
                "W. Europe Standard Time",
                "India Standard Time",
                true,
                true,
                [
                    "2024-01-15 16:30:00",
                    "2024-07-15 15:30:00",
                    "2024-03-10 06:00:00",
                    "2024-03-10 07:00:00",
                    "2024-03-10 08:00:00",
                    "2024-11-03 06:00:00",
                    "2024-11-03 05:00:00",
                    "1999-06-02 02:30:00",
                    "2025-01-01 04:00:00",
                ],
            ),
            (
                "UTC",
                "Nepal Standard Time",
                true,
                true,
                [
                    "2024-01-15 17:45:00",
                    "2024-07-15 17:45:00",
                    "2024-03-10 07:15:00",
                    "2024-03-10 08:15:00",
                    "2024-03-10 09:15:00",
                    "2024-11-03 07:15:00",
                    "2024-11-03 06:15:00",
                    "1999-06-02 04:45:00",
                    "2025-01-01 05:15:00",
                ],
            ),
            (
                "UTC",
                "AUS Eastern Standard Time",
                true,
                true,
                [
                    "2024-01-15 23:00:00",
                    "2024-07-15 22:00:00",
                    "2024-03-10 12:30:00",
                    "2024-03-10 13:30:00",
                    "2024-03-10 14:30:00",
                    "2024-11-03 12:30:00",
                    "2024-11-03 11:30:00",
                    "1999-06-02 09:00:00",
                    "2025-01-01 10:30:00",
                ],
            ),
            (
                "UTC",
                "UTC-11",
                true,
                true,
                [
                    "2024-01-15 01:00:00",
                    "2024-07-15 01:00:00",
                    "2024-03-09 14:30:00",
                    "2024-03-09 15:30:00",
                    "2024-03-09 16:30:00",
                    "2024-11-02 14:30:00",
                    "2024-11-02 13:30:00",
                    "1999-06-01 12:00:00",
                    "2024-12-31 12:30:00",
                ],
            ),
        ];
        for (zin, zout, din, dout, want) in cases {
            let (a, b) = (zone(zin).unwrap(), zone(zout).unwrap());
            for (input, w) in inputs.iter().zip(want) {
                let got = from_utc(to_utc(t(input), a, *din), b, *dout);
                assert_eq!(got, t(w), "{zin} -> {zout} {din} {dout}: {input}");
            }
        }
        assert!(zone("Kamchatka Standard Time").is_err());
    }
}
