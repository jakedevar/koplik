//! Gravity contact weights after Xia, Bjørnstad & Grenfell (2004), "Measles
//! metapopulation dynamics: a gravity model for epidemiological coupling and
//! dynamics", Am. Nat. 164(2):267-281, https://doi.org/10.1086/422341. Their
//! coupling term is theta * N_k^tau1 * sum_j I_j^tau2 / d_kj^rho (recipient k,
//! donor j); secondary sources report their England & Wales fit as tau1 = 1,
//! tau2 = 1 (Jandarov & Haran, arXiv:1110.6451, sec. 4) or tau2 = 1.5 (Bharti et
//! al. 2008, PMC2275791), rho = 1. The primary PDF was not accessible to verify
//! which; neither fit is transplanted to Texas. All four coefficients come from
//! ScenarioInput; the engine's defaults carry no gravity (`defaults.rs`).
//!
//! F_ij = scale N_i^a N_j^b / d_ij^c, with d in km (haversine on the GRS80 mean
//! sphere). F is a contact-equivalent flow of residents of i into j, not
//! migration: nobody changes node. Set w_ii = 1, w_ij = F_ij / N_i, then normalize
//! each row. The force of infection on i is beta * sum_j w_ij I_j / N_j, so with
//! a = b = 1 the cross term is scale * I_j / d^c, the Xia form with tau1 = tau2 = 1
//! up to the row normalization. Normalizing keeps beta = R0 / D exactly at
//! uniform prevalence. Scale has units persons^(1-a-b) km^c relative to local
//! contacts; it is a scenario choice, not a fitted constant.

use crate::seir::EngineError;
use koplik_contracts::v1::{Centroid, ScenarioInput};

fn distance_km(a: Centroid, b: Centroid) -> f64 {
    let radians = core::f64::consts::PI / 180.0;
    let lat = libm::sin((b.latitude - a.latitude) * radians / 2.0);
    let lon = libm::sin((b.longitude - a.longitude) * radians / 2.0);
    let h =
        lat * lat + libm::cos(a.latitude * radians) * libm::cos(b.latitude * radians) * lon * lon;
    // GRS80 mean radius R1 = (2a+b)/3 = 6 371 008.7714 m, rounded to 0.1 m
    // (Moritz, "Geodetic Reference System 1980", J. Geod. 74:128-162, 2000,
    // https://doi.org/10.1007/s001900050278). A geometric conversion constant,
    // not an epidemiological calibration parameter.
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
