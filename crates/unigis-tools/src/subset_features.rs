//! Subset Features: a record partition, with both outputs committed by the host.
use crate::{
    TableOutput, ToolOutput,
    registry::{Context, Kind, Param, Report, Runner, Tool, WorkspaceOutput},
};
use unigis_core::{Error, Feature, FeatureClass, FieldType, Result, Value};

pub const TOOL: Tool = Tool {
    id: "subset_features",
    name: "Subset Features",
    toolbox: "Data Management › Sampling",
    summary: "Randomly partitions features or table records into disjoint training and optional test datasets.",
    params: &[
        Param {
            name: "input",
            label: "Input Features",
            kind: Kind::Features,
            required: true,
            default: "",
            help: "Feature class or table to partition by whole records.",
        },
        Param {
            name: "out_training_feature_class",
            label: "Output Training Feature Class",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "New training dataset. Existing outputs are refused.",
        },
        Param {
            name: "out_test_feature_class",
            label: "Output Test Feature Class",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "Optional complementary test dataset in the same workspace.",
        },
        Param {
            name: "size_of_training_dataset",
            label: "Size of Training Feature Subset",
            kind: Kind::Number,
            required: false,
            default: "50",
            help: "Percentage (rounded) or absolute record count (truncated, capped at input count).",
        },
        Param {
            name: "subset_size_units",
            label: "Subset Size Units",
            kind: Kind::Choice(&["PERCENTAGE_OF_INPUT", "ABSOLUTE_VALUE"]),
            required: false,
            default: "PERCENTAGE_OF_INPUT",
            help: "Interpret the training size as a percentage or count.",
        },
        Param {
            name: "seed",
            label: "Random Seed",
            kind: Kind::Text,
            required: false,
            default: "0",
            help: "Reproducible native seed; differs from ArcGIS Mersenne Twister streams.",
        },
    ],
    output_suffix: "training",
    measures: false,
    run: Runner::Report(run),
};
fn bad(s: impl Into<String>) -> Error {
    Error::Unsupported(s.into())
}
fn value(ctx: &dyn Context, k: &str, d: &str) -> String {
    ctx.get(k).unwrap_or_else(|| d.into())
}
fn training_count(total: usize, size: f64, percentage: bool) -> Result<usize> {
    if !size.is_finite()
        || size <= 0.0
        || (!percentage && size < 1.0)
        || (percentage && size > 100.0)
        || (!percentage && size > i32::MAX as f64)
    {
        return Err(bad("ERROR 000854: The value is out of the valid range."));
    }
    Ok(if percentage {
        (total as f64 * size / 100.0).round() as usize
    } else {
        (size.trunc() as usize).min(total)
    })
}
fn output(fc: &dyn FeatureClass, mut features: Vec<Feature>, path: &str) -> WorkspaceOutput {
    // Public SubsetFeatures export controls show per-output carry-forward for
    // NULL text/numeric attributes in dBASE; dates and FileGDB retain NULLs.
    if std::path::Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("shp") || e.eq_ignore_ascii_case("dbf"))
    {
        let mut previous: Vec<Option<Value>> = fc
            .fields()
            .iter()
            .map(|f| match f.field_type {
                FieldType::String => Some(Value::String(String::new())),
                FieldType::SmallInteger => Some(Value::SmallInteger(0)),
                FieldType::Integer => Some(Value::Integer(0)),
                FieldType::BigInteger => Some(Value::BigInteger(0)),
                FieldType::Single => Some(Value::Single(0.0)),
                FieldType::Double => Some(Value::Double(0.0)),
                _ => None,
            })
            .collect();
        for row in &mut features {
            for (value, previous) in row.attributes.iter_mut().zip(&mut previous) {
                if let Some(last) = previous {
                    if value.is_null() {
                        *value = last.clone();
                    } else {
                        *last = value.clone();
                    }
                }
            }
        }
    }
    if let Some(kind) = fc.geometry_kind() {
        WorkspaceOutput::Features(ToolOutput {
            kind,
            has_z: fc.has_z(),
            has_m: fc.has_m(),
            fields: fc.fields().to_vec(),
            spatial_reference_wkt: fc.spatial_reference_wkt().map(str::to_owned),
            features,
            messages: vec![],
        })
    } else {
        WorkspaceOutput::Table(TableOutput {
            fields: fc.fields().to_vec(),
            rows: features.into_iter().map(|f| f.attributes).collect(),
        })
    }
}
fn run(ctx: &dyn Context) -> Result<Report> {
    let fc = ctx.features("input")?;
    let size = value(ctx, "size_of_training_dataset", "50")
        .parse::<f64>()
        .map_err(|_| bad("ERROR 000854: Invalid subset size"))?;
    let percentage = value(ctx, "subset_size_units", "PERCENTAGE_OF_INPUT")
        .eq_ignore_ascii_case("PERCENTAGE_OF_INPUT");
    training_count(1, size, percentage)?;
    let train_path = ctx
        .get("out_training_feature_class")
        .ok_or_else(|| bad("ERROR 000735: Output Training Feature Class: Value is required"))?;
    let test_path = ctx.get("out_test_feature_class");
    let rows = fc.features().take(1_000_001).collect::<Result<Vec<_>>>()?;
    if rows.is_empty() {
        return Err(bad("ERROR 040303: The input features have no records."));
    }
    if rows.len() > 1_000_000 {
        return Err(bad("at most1000000 input records are supported"));
    }
    let n = training_count(rows.len(), size, percentage)?;
    let seed = value(ctx, "seed", "0")
        .parse::<u64>()
        .map_err(|_| bad("seed must be an unsigned integer"))?;
    let mut selected = vec![false; rows.len()];
    for i in crate::random_points::shuffled_indices(rows.len(), seed)
        .into_iter()
        .take(n)
    {
        selected[i] = true;
    }
    let mut training = Vec::with_capacity(n);
    let mut testing = Vec::with_capacity(rows.len() - n);
    for (i, row) in rows.into_iter().enumerate() {
        if selected[i] {
            training.push(row)
        } else {
            testing.push(row)
        }
    }
    let test_count = testing.len();
    let has_test = test_path.is_some();
    let training = output(fc.as_ref(), training, &train_path);
    let mut outputs = vec![(train_path, training)];
    if let Some(path) = test_path {
        let testing = output(fc.as_ref(), testing, &path);
        outputs.push((path, testing));
    }
    ctx.write_datasets("input", outputs)?;
    let mut report = Report {
        rows: vec![("training_count".into(), n as f64)],
        messages: vec![format!("Created {n} training records")],
    };
    if has_test {
        report.rows.push(("test_count".into(), test_count as f64));
        report
            .messages
            .push(format!("Created {test_count} test records"));
    }
    if percentage && (n == 0 || (has_test && test_count == 0)) {
        report.messages.push("WARNING 040303: The input features, the output training feature class, or the output test feature class have no records.".into());
    }
    Ok(report)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_host_cannot_claim_dataset_writes() {
        struct Host;
        impl Context for Host {
            fn get(&self, _: &str) -> Option<String> {
                None
            }
            fn features(&self, _: &str) -> Result<Box<dyn FeatureClass + '_>> {
                unreachable!()
            }
        }
        assert!(
            Host.write_datasets("input", vec![])
                .unwrap_err()
                .to_string()
                .contains("dataset outputs are not supported by this host")
        );
    }
    #[test]
    fn public_rounding_and_bounds() {
        assert_eq!(training_count(7, 50.0, true).unwrap(), 4);
        assert_eq!(training_count(5, 50.0, true).unwrap(), 3);
        assert_eq!(training_count(7, 33.3, true).unwrap(), 2);
        assert_eq!(training_count(7, 3.6, false).unwrap(), 3);
        assert_eq!(training_count(7, 1.0, true).unwrap(), 0);
        assert_eq!(training_count(7, 99.0, false).unwrap(), 7);
        for size in [0.0, -1.0, 101.0, f64::NAN, f64::INFINITY] {
            assert!(training_count(7, size, true).is_err());
        }
    }
    #[test]
    fn permutation_is_reproducible_complete_and_balanced() {
        let a = crate::random_points::shuffled_indices(1000, 0);
        assert_eq!(a, crate::random_points::shuffled_indices(1000, 0));
        assert_ne!(a, crate::random_points::shuffled_indices(1000, 1));
        let mut all = a.clone();
        all.sort();
        assert_eq!(all, (0..1000).collect::<Vec<_>>());
        let bins = a[..500].iter().fold([0; 4], |mut b, &i| {
            b[i / 250] += 1;
            b
        });
        assert!(bins.iter().all(|&n| (95..155).contains(&n)), "{bins:?}");
    }
}
