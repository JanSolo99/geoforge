//! Stored band statistics in PAM sidecars and the TIFF GDAL_METADATA tag.
//! Reading preserves cached values; it never recomputes them from pixels.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use tiff::{decoder::Decoder, tags::Tag};
use unigis_core::{Error, Result};

use crate::Raster;

pub type BandStatistics = BTreeMap<String, String>;

/// Reads embedded statistics, overlaid by PAM statistics for each band/key.
pub fn read(path: &Path) -> Result<Vec<BandStatistics>> {
    let error = |e| Error::format("GeoTIFF", format!("{}: {e}", path.display()));
    let file = File::open(path).map_err(|e| Error::io(path, e))?;
    let mut decoder = Decoder::new(BufReader::new(file)).map_err(error)?;
    let samples = decoder
        .find_tag_unsigned::<u16>(Tag::SamplesPerPixel)
        .map_err(error)?
        .unwrap_or(1) as usize;
    let embedded = decoder
        .find_tag(Tag::Unknown(42112))
        .map_err(error)?
        .and_then(|v| v.into_string().ok());
    let mut aux_path = path.as_os_str().to_owned();
    aux_path.push(".aux.xml");
    let aux = match std::fs::read_to_string(&aux_path) {
        Ok(xml) => Some(xml),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(Error::io(Path::new(&aux_path), e)),
    };
    Ok(from_xml(samples, embedded.as_deref(), aux.as_deref()))
}

/// Shared band/domain-aware parsing for raster type planning and cache copying.
pub(crate) fn from_xml(
    samples: usize,
    embedded: Option<&str>,
    aux: Option<&str>,
) -> Vec<BandStatistics> {
    let mut bands = vec![BandStatistics::new(); samples];
    if let Some(xml) = embedded {
        for (head, text) in elements(xml, "Item") {
            if attribute(head, "domain").is_some_and(|v| !v.is_empty()) {
                continue;
            }
            let Some(index) = attribute(head, "sample").and_then(|s| s.parse::<usize>().ok())
            else {
                continue;
            };
            if let Some(band) = bands.get_mut(index) {
                insert(band, attribute(head, "name"), text);
            }
        }
    }
    if let Some(xml) = aux {
        for (head, body) in elements(xml, "PAMRasterBand") {
            let Some(index) = attribute(head, "band")
                .and_then(|s| s.parse::<usize>().ok())
                .and_then(|v| v.checked_sub(1))
            else {
                continue;
            };
            let Some(band) = bands.get_mut(index) else {
                continue;
            };
            for (head, body) in elements(body, "Metadata") {
                if attribute(head, "domain").is_some_and(|v| !v.is_empty()) {
                    continue;
                }
                for (head, text) in elements(body, "MDI") {
                    insert(band, attribute(head, "key"), text);
                }
            }
        }
    }
    bands
}

/// Encodes stored statistics as the public GDAL_METADATA XML representation.
pub fn encode(bands: &[BandStatistics]) -> Option<String> {
    let mut xml = String::from("<GDALMetadata>\n");
    let mut any = false;
    for (index, band) in bands.iter().enumerate() {
        for (name, value) in band {
            if name.starts_with("STATISTICS_") {
                any = true;
                xml += &format!(
                    "  <Item name=\"{}\" sample=\"{index}\">{}</Item>\n",
                    escape(name),
                    escape(value)
                );
            }
        }
    }
    any.then(|| xml + "</GDALMetadata>\n")
}

/// Computes statistics for valid cells in every band for a new embedded cache.
pub fn for_raster(raster: &Raster) -> Option<String> {
    encode(&values_for_raster(raster))
}

/// Computes values for a new cache, without changing existing stored values.
fn values_for_raster(raster: &Raster) -> Vec<BandStatistics> {
    (0..raster.bands.len())
        .map(|index| {
            let Some(s) = crate::stats::statistics_for_band(raster, index) else {
                return BandStatistics::new();
            };
            let values = [
                ("COUNT", format!("{:.6}", s.count as f64)),
                ("MINIMUM", crate::stats::format_g14(s.min)),
                ("MAXIMUM", crate::stats::format_g14(s.max)),
                ("MEAN", crate::stats::format_g14(s.mean)),
                ("STDDEV", crate::stats::format_g14(s.std_dev)),
                ("MEDIAN", "nan".into()),
                ("SKIPFACTORX", "1".into()),
                ("SKIPFACTORY", "1".into()),
            ];
            values
                .into_iter()
                .map(|(key, value)| (format!("STATISTICS_{key}"), value))
                .collect()
        })
        .collect::<Vec<_>>()
}

fn insert(band: &mut BandStatistics, name: Option<&str>, text: &str) {
    if let Some(name) = name {
        let name = unescape(name);
        if name.starts_with("STATISTICS_") {
            band.insert(name, unescape(text));
        }
    }
}

fn elements<'a>(mut xml: &'a str, name: &str) -> Vec<(&'a str, &'a str)> {
    let open = format!("<{name}");
    let close = format!("</{name}>");
    let mut result = Vec::new();
    while let Some(start) = xml.find(&open) {
        xml = &xml[start + open.len()..];
        if !xml.starts_with('>') && !xml.starts_with(char::is_whitespace) {
            continue;
        }
        let Some(end) = xml.find('>') else { break };
        let head = &xml[..end];
        xml = &xml[end + 1..];
        let Some(end) = xml.find(&close) else { break };
        result.push((head, &xml[..end]));
        xml = &xml[end + close.len()..];
    }
    result
}

fn attribute<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    for (start, _) in head.match_indices(name) {
        if start > 0 && !head[..start].ends_with(char::is_whitespace) {
            continue;
        }
        let tail = head[start + name.len()..]
            .trim_start()
            .strip_prefix('=')?
            .trim_start();
        let quote = tail.chars().next()?;
        if quote == '\'' || quote == '"' {
            return tail[1..].split_once(quote).map(|v| v.0);
        }
    }
    None
}

fn unescape(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
