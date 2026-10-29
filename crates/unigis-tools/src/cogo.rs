//! COGO capabilities are supplied by the host; line validation is shared.
use crate::registry::{Context, Kind, Param, Report, Runner, Tool};
use unigis_core::{Error, GeometryKind, Result};

pub const DISABLE: Tool = Tool {
    id: "disable_cogo",
    name: "Disable COGO",
    toolbox: "Data Management › COGO",
    summary: "Disables COGO on line features while retaining their dimension fields and values.",
    params: &[Param {
        name: "input",
        label: "Input Line Features",
        kind: Kind::Features,
        required: true,
        default: "",
        help: "Line feature class to update in place.",
    }],
    output_suffix: "cogo_disabled",
    measures: false,
    run: Runner::Report(disable),
};
fn disable(ctx: &dyn Context) -> Result<Report> {
    let input = ctx.features("input")?;
    match input.geometry_kind() {
        None => {
            return Err(Error::Unsupported(
                "ERROR 000732: Input Line Features: dataset does not exist or is not supported"
                    .into(),
            ));
        }
        Some(GeometryKind::Polyline) => {}
        _ => {
            return Err(Error::Unsupported(
                "ERROR 003917: The feature class geometry is not of type polyline".into(),
            ));
        }
    }
    drop(input);
    ctx.disable_cogo("input")?;
    Ok(Report {
        rows: vec![],
        messages: vec!["COGO disabled".into()],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use unigis_core::FeatureClass;
    #[test]
    fn unsupported_host_does_not_claim_mutation() {
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
            Host.disable_cogo("input")
                .unwrap_err()
                .to_string()
                .contains("COGO edits are not supported by this host")
        );
    }
}
