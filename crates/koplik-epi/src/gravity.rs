//! Gravity contact weights inspired by Xia, Bjørnstad & Grenfell (2004),
//! https://doi.org/10.1086/422341. Their fitted England/Wales TSIR coefficients
//! are NOT transplanted to Texas. All four coefficients come from ScenarioInput.
//!
//! F_ij = scale N_i^a N_j^b / d_ij^c, with d in km. F is a contact-equivalent
//! flow, not migration. Set w_ii=1, w_ij=F_ij/N_i, then normalize each row.
//! This explicit mixing assumption retains beta=R0/D at uniform prevalence.
//! Scale has units persons^(1-a-b) km^c; it is relative to local contacts.

use crate::seir::EngineError;
use koplik_contracts::v1::{Centroid, ScenarioInput};

fn distance_km(a: Centroid, b: Centroid) -> f64 {
    let radians = core::f64::consts::PI / 180.0;
    let lat = libm::sin((b.latitude - a.latitude) * radians / 2.0);
    let lon = libm::sin((b.longitude - a.longitude) * radians / 2.0);
    let h =
        lat * lat + libm::cos(a.latitude * radians) * libm::cos(b.latitude * radians) * lon * lon;
    // IUGG mean radius (2a+b)/3 for WGS84, rounded to 0.1 m. A geometric
    // conversion constant, not an epidemiological calibration parameter.
    // https://doi.org/10.1007/s001900050278 (Moritz, Geodetic Reference System 1980).
    2.0 * 6371.0088 * libm::asin(libm::sqrt(h.clamp(0.0, 1.0)))
}

pub(crate) fn weights(input: &ScenarioInput) -> Result<Vec<Vec<f64>>, EngineError> {
    let mut rows = Vec::with_capacity(input.nodes.len());
    for (i, origin) in input.nodes.iter().enumerate() {
        let mut row = vec![0.0; input.nodes.len()];
        row[i] = 1.0;
        if let Some(g) = input.parameters.gravity.filter(|g| g.scale > 0.0) {
            for (j, destination) in input.nodes.iter().enumerate() {
                if i == j {
                    continue;
                }
                let distance = distance_km(origin.centroid, destination.centroid);
                if distance == 0.0 && g.distance_exponent > 0.0 {
                    return Err(EngineError::Invalid(
                        "gravity requires distinct centroids when distance exponent is positive"
                            .into(),
                    ));
                }
                let flow = g.scale
                    * libm::pow(origin.population as f64, g.origin_exponent)
                    * libm::pow(destination.population as f64, g.destination_exponent)
                    / libm::pow(distance, g.distance_exponent);
                row[j] = flow / origin.population as f64;
                if !row[j].is_finite() {
                    return Err(EngineError::Invalid(
                        "gravity weights overflow; reduce scale or exponents".into(),
                    ));
                }
            }
        }
        let sum: f64 = row.iter().sum();
        if !sum.is_finite() {
            return Err(EngineError::Invalid("gravity row sum overflow".into()));
        }
        for weight in &mut row {
            *weight /= sum;
        }
        rows.push(row);
    }
    Ok(rows)
}
