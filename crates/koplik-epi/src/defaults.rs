//! Explicit demonstration defaults, never a fit to outbreak or backtest outcomes.

use koplik_contracts::v1::{R0, SeirParameters};

pub fn default_parameters() -> SeirParameters {
    SeirParameters {
        // Guerra et al. (2017), Lancet Infect Dis, systematic review: "For
        // measles, R0 is often cited to be 12-18" and "R0 estimates vary more
        // than the often cited range of 12-18". A conventional range, NOT a
        // locally calibrated prior; the uniform density is a demonstration
        // modelling assumption. https://pubmed.ncbi.nlm.nih.gov/28757186/
        r0: R0::UniformPrior {
            min: 12.0,
            max: 18.0,
        },
        // CDC Pink Book ch. 13 (Measles): "Incubation period 11 to 12 days"
        // (exposure to prodrome); "exposure to rash onset averages 14 days
        // (range, 7 to 21 days)"; "transmissible from 4 days before through 4
        // days after rash onset". Latent (exposure to infectiousness) is taken
        // as 14 - 4 = 10 d, the low end of the spec's 10-12 d band; the 11-12 d
        // figure is incubation to symptoms, not to infectiousness.
        // https://www.cdc.gov/pinkbook/hcp/table-of-contents/chapter-13-measles.html
        latent_period_days: 10.0,
        // Same source: infectious 4 d before to 4 d after rash = 8 d. Treating
        // this window as an exponential mean is an explicit SEIR approximation.
        infectious_period_days: 8.0,
        // CDC: "One dose is 93% (range: 39% to 100%) effective at preventing
        // measles"; "Two doses ... are 97% (range: 67% to 100%) effective".
        // Point estimates, not sterilizing-immunity measurements. The all-or-none
        // susceptibility approximation is documented in seir.rs.
        // https://www.cdc.gov/measles/hcp/vaccine-considerations/index.html
        mmr_effectiveness_one_dose: 0.93,
        mmr_effectiveness_two_doses: 0.97,
        // Numerical/display choices for this demo, not biological estimates:
        // E4 of thoughts/shared/project/koplik-spec.md (daily tau-leaping).
        time_step_days: 1.0,
        // Six-month demo window; an explicit product choice, not a fitted value.
        horizon_days: 180,
        // No calibrated Texas mobility parameters exist in this engine. Callers
        // opt in with explicit scale and exponents; see gravity.rs and its source.
        gravity: None,
    }
}

/// One published citation for a [`default_parameters`] field (#1400): the scenario artifact's
/// companion provenance file lists these beside the values the scenario actually ran with.
/// Text only; it never changes a value. The authoritative comments stay on the defaults above.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParameterCitation {
    /// The `SeirParameters` field name.
    pub parameter: &'static str,
    /// The source, as the code comment above cites it.
    pub source: &'static str,
    /// Where to read it, when the source is a public page.
    pub url: Option<&'static str>,
    /// What the source supports and what it does not (assumptions stay visible).
    pub note: &'static str,
}

const PINK_BOOK: &str =
    "https://www.cdc.gov/pinkbook/hcp/table-of-contents/chapter-13-measles.html";
const CDC_VACCINE: &str = "https://www.cdc.gov/measles/hcp/vaccine-considerations/index.html";
const SPEC: &str = "Koplik spec E4 (thoughts/shared/project/koplik-spec.md)";

/// A citation for every field of [`SeirParameters`], in field order.
pub fn parameter_citations() -> Vec<ParameterCitation> {
    vec![
        ParameterCitation {
            parameter: "r0",
            source: "Guerra et al. (2017), Lancet Infect Dis, systematic review of measles R0",
            url: Some("https://pubmed.ncbi.nlm.nih.gov/28757186/"),
            note: "\"For measles, R0 is often cited to be 12-18\". A conventional range, not a locally calibrated prior; the uniform density is a demonstration modelling assumption. Each simulated run draws its own R0.",
        },
        ParameterCitation {
            parameter: "latent_period_days",
            source: "CDC Pink Book, chapter 13 (Measles)",
            url: Some(PINK_BOOK),
            note: "Exposure to rash onset averages 14 days and the case is transmissible from 4 days before rash onset, so latent = 14 - 4 = 10 days, the low end of the spec's 10-12 day band. The 11-12 day figure is incubation to symptoms, not to infectiousness.",
        },
        ParameterCitation {
            parameter: "infectious_period_days",
            source: "CDC Pink Book, chapter 13 (Measles)",
            url: Some(PINK_BOOK),
            note: "Transmissible from 4 days before through 4 days after rash onset, 8 days. Treating this window as an exponential mean is an explicit SEIR approximation.",
        },
        ParameterCitation {
            parameter: "mmr_effectiveness_one_dose",
            source: "CDC, MMR vaccine considerations",
            url: Some(CDC_VACCINE),
            note: "One dose is 93% (range 39% to 100%) effective at preventing measles. A point estimate, not a measured immunity. Retained in the scenario but unused by the engine: v1 has no one-dose-only coverage.",
        },
        ParameterCitation {
            parameter: "mmr_effectiveness_two_doses",
            source: "CDC, MMR vaccine considerations",
            url: Some(CDC_VACCINE),
            note: "Two doses are 97% (range 67% to 100%) effective. Applied to kindergarten MMR coverage as all-or-none protection and as a proxy for residents of every age (see seir.rs); a demonstration assumption, not measured population immunity.",
        },
        ParameterCitation {
            parameter: "time_step_days",
            source: SPEC,
            url: None,
            note: "A numerical choice (daily tau-leaping), not a biological estimate.",
        },
        ParameterCitation {
            parameter: "horizon_days",
            source: SPEC,
            url: None,
            note: "A six-month demonstration window, an explicit product choice, not a fitted value.",
        },
        ParameterCitation {
            parameter: "gravity",
            source: "Xia, Bjørnstad & Grenfell (2004), Am Nat 164(2):267-281; Bharti et al. (2008)",
            url: Some("https://doi.org/10.1086/422341"),
            note: "No coupling by default: no calibrated Texas mobility parameters exist in this engine and published fits from other settings are not transplanted to Texas (see gravity.rs). Counties are simulated in isolation unless a scenario supplies explicit coefficients.",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every parameter the scenario states has a citation, in field order, so a field added
    /// to `SeirParameters` cannot be published without one.
    #[test]
    fn every_default_parameter_has_a_citation() {
        let value = serde_json::to_value(default_parameters()).unwrap();
        let fields: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let cited: Vec<&str> = parameter_citations().iter().map(|c| c.parameter).collect();
        let mut a = fields.clone();
        let mut b = cited.clone();
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a, b);
        assert!(
            parameter_citations()
                .iter()
                .all(|c| !c.source.is_empty() && !c.note.is_empty())
        );
    }
}
