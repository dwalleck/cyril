//! Normalized verdict state shared by the pipeline and its application readers.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Verdict {
    Confirmed,
    Plausible,
    Refuted,
    Unverified,
}

impl Verdict {
    pub const REPORT_ORDER: [Self; 4] = [
        Self::Confirmed,
        Self::Plausible,
        Self::Refuted,
        Self::Unverified,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Confirmed => "CONFIRMED",
            Self::Plausible => "PLAUSIBLE",
            Self::Refuted => "REFUTED",
            Self::Unverified => "UNVERIFIED",
        }
    }

    /// Untrusted agent spelling is normalized once; unknown or missing is unverified.
    pub(crate) fn from_agent(value: Option<&Value>) -> Self {
        match value
            .and_then(Value::as_str)
            .map(str::trim)
            .map(str::to_uppercase)
            .as_deref()
        {
            Some("CONFIRMED") => Self::Confirmed,
            Some("PLAUSIBLE") => Self::Plausible,
            Some("REFUTED") => Self::Refuted,
            _ => Self::Unverified,
        }
    }
}

impl std::fmt::Display for Verdict {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialization_preserves_artifact_spellings() -> Result<(), serde_json::Error> {
        for verdict in Verdict::REPORT_ORDER {
            let value = serde_json::to_value(verdict)?;
            assert_eq!(value, json!(verdict.as_str()));
            assert_eq!(serde_json::from_value::<Verdict>(value)?, verdict);
            assert_eq!(verdict.to_string(), verdict.as_str());
        }
        assert!(serde_json::from_value::<Verdict>(json!("unknown")).is_err());
        Ok(())
    }

    #[test]
    fn agent_normalization_handles_case_absence_and_odd_values() {
        assert_eq!(
            Verdict::from_agent(Some(&json!(" confirmed "))),
            Verdict::Confirmed
        );
        for value in [json!(null), json!(42), json!({}), json!("unknown")] {
            assert_eq!(Verdict::from_agent(Some(&value)), Verdict::Unverified);
        }
        assert_eq!(Verdict::from_agent(None), Verdict::Unverified);
    }
}
