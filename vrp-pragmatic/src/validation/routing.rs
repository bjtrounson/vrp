#[cfg(test)]
#[path = "../../tests/unit/validation/routing_test.rs"]
mod routing_test;

use super::*;
use crate::utils::combine_error_results;
use std::collections::HashSet;
use vrp_core::prelude::Float;

/// Checks that no duplicated profile names specified.
fn check_e1500_duplicated_profiles(ctx: &ValidationContext) -> Result<(), FormatError> {
    get_duplicates(ctx.problem.fleet.profiles.iter().map(|p| &p.name)).map_or(Ok(()), |names| {
        Err(FormatError::new(
            "E1500".to_string(),
            "duplicated profile names".to_string(),
            format!("remove duplicates of profiles with the names: '{}'", names.join(", ")),
        ))
    })
}

/// Checks that profiles collection is not empty.
fn check_e1501_empty_profiles(ctx: &ValidationContext) -> Result<(), FormatError> {
    if ctx.problem.fleet.profiles.is_empty() {
        Err(FormatError::new(
            "E1501".to_string(),
            "empty profile collection".to_string(),
            "specify at least one profile".to_string(),
        ))
    } else {
        Ok(())
    }
}

/// Checks that only one type of location is used.
fn check_e1502_no_location_type_mix(_ctx: &ValidationContext, location_types: (bool, bool)) -> Result<(), FormatError> {
    let (has_coordinates, has_indices) = location_types;

    if has_coordinates && has_indices {
        Err(FormatError::new(
            "E1502".to_string(),
            "mixing different location types".to_string(),
            "use either coordinates or indices for all locations".to_string(),
        ))
    } else {
        Ok(())
    }
}

/// Checks that routing matrix is supplied when location indices are used.
fn check_e1503_no_matrix_when_indices_used(
    ctx: &ValidationContext,
    location_types: (bool, bool),
) -> Result<(), FormatError> {
    let (_, has_indices) = location_types;

    if has_indices && ctx.matrices.is_none_or(|matrices| matrices.is_empty()) {
        Err(FormatError::new(
            "E1503".to_string(),
            "location indices requires routing matrix to be specified".to_string(),
            "either use coordinates everywhere or specify routing matrix".to_string(),
        ))
    } else {
        Ok(())
    }
}

/// Checks that coord index has a proper maximum index for
fn check_e1504_index_size_mismatch(ctx: &ValidationContext) -> Result<(), FormatError> {
    let max_index = ctx.coord_index.max_matrix_index();

    let (matrix_size, is_correct_index) = ctx
        .matrices
        .and_then(|matrices| matrices.first())
        .map(|matrix| (matrix.distances.len() as Float).sqrt().round() as usize)
        .map_or((0_usize, true), |matrix_size| (matrix_size, max_index + 1 == matrix_size));

    if !is_correct_index {
        Err(FormatError::new(
            "E1504".to_string(),
            "amount of locations does not match matrix dimension".to_string(),
            format!(
                "check matrix size: max location index '{max_index}' + 1 should be equal to matrix size ('{matrix_size}')"
            ),
        ))
    } else {
        Ok(())
    }
}

/// Checks that no duplicated profile names specified.
fn check_e1505_profiles_exist(ctx: &ValidationContext) -> Result<(), FormatError> {
    let known_matrix_profiles = ctx.problem.fleet.profiles.iter().map(|p| p.name.clone()).collect::<HashSet<_>>();

    let unknown_vehicle_profiles = ctx
        .problem
        .fleet
        .vehicles
        .iter()
        .map(|vehicle| vehicle.profile.matrix.clone())
        .chain(ctx.problem.plan.clustering.iter().map(|clustering| match clustering {
            Clustering::Vicinity { profile, .. } => profile.matrix.clone(),
        }))
        .filter(|matrix| !known_matrix_profiles.contains(matrix))
        .collect::<HashSet<_>>();

    if unknown_vehicle_profiles.is_empty() {
        Ok(())
    } else {
        let unknown_profiles = unknown_vehicle_profiles.into_iter().collect::<Vec<_>>();
        Err(FormatError::new(
            "E1505".to_string(),
            "unknown matrix profile name in vehicle or vicinity clustering profile".to_string(),
            format!("ensure that matrix profiles '{}' are defined in profiles", unknown_profiles.join(", ")),
        ))
    }
}

/// Validates routing rules.
pub fn validate_routing(ctx: &ValidationContext) -> Result<(), MultiFormatError> {
    let location_types = (ctx.coord_index.has_coordinates(), ctx.coord_index.has_indices());

    combine_error_results(&[
        check_e1500_duplicated_profiles(ctx),
        check_e1501_empty_profiles(ctx),
        check_e1502_no_location_type_mix(ctx, location_types),
        check_e1503_no_matrix_when_indices_used(ctx, location_types),
        check_e1504_index_size_mismatch(ctx),
        check_e1505_profiles_exist(ctx),
        check_weight_routing(ctx),
    ])
    .map_err(From::from)
}

fn check_weight_routing(ctx: &ValidationContext) -> Result<(), FormatError> {
    let fail = |message: String| FormatError::new("E1506".to_string(), "invalid weight routing".to_string(), message);
    if let Some(Clustering::Vicinity { profile, .. }) = &ctx.problem.plan.clustering
        && profile.weight_routing.is_some()
    {
        return Err(fail("weightRouting belongs on a vehicle profile, not a clustering profile".into()));
    }
    for vehicle in &ctx.problem.fleet.vehicles {
        let Some(config) = &vehicle.profile.weight_routing else {
            continue;
        };
        if ctx.problem.plan.clustering.is_some() || vehicle.shifts.iter().any(|s| s.recharges.is_some()) {
            return Err(fail(
                "weight routing cannot currently be combined with vicinity clustering or recharges".into(),
            ));
        }
        if config.tare_weight_kg < 0 || config.mass_dimension_index >= vehicle.capacity.len() || config.bands.is_empty()
        {
            return Err(fail(format!(
                "vehicle '{}': provide nonnegative tare, a valid massDimensionIndex and nonempty bands",
                vehicle.type_id
            )));
        }
        if config.tare_weight_kg.checked_add(i64::from(vehicle.capacity[config.mass_dimension_index])).is_none()
            || vehicle.profile.scale.is_some_and(|scale| !scale.is_finite() || scale <= 0.)
        {
            return Err(fail("gross weight must fit in i64 and profile scale must be positive and finite".into()));
        }
        let mut limits = HashSet::new();
        let matrices = ctx.matrices.ok_or_else(|| fail("supply explicit matrices for weight routing".into()))?;
        for band in &config.bands {
            if band.max_gross_weight_kg <= 0 || !limits.insert(band.max_gross_weight_kg) {
                return Err(fail("weight thresholds must be positive and unique".into()));
            }
            if !ctx.problem.fleet.profiles.iter().any(|p| p.name == band.matrix) {
                return Err(fail(format!("unknown weight-band profile '{}'", band.matrix)));
            }
            let matching = matrices.iter().filter(|m| m.profile.as_ref() == Some(&band.matrix)).collect::<Vec<_>>();
            if matching.len() != 1 || matching[0].timestamp.is_some() {
                return Err(fail(format!("supply exactly one non-timestamped matrix for '{}'", band.matrix)));
            }
            let matrix = matching[0];
            let size = ctx.coord_index.max_matrix_index() + 1;
            if matrix.distances.len() != size * size
                || matrix.travel_times.len() != size * size
                || matrix.error_codes.as_ref().is_some_and(|e| e.len() != size * size)
            {
                return Err(fail(format!(
                    "matrix '{}' must have matching square arrays for the problem's locations",
                    band.matrix
                )));
            }
            if matrix.distances.iter().chain(&matrix.travel_times).any(|v| *v < 0) {
                return Err(fail(
                    "use errorCodes for unreachable legs; distances and times must be nonnegative".into(),
                ));
            }
        }
        if config.bands.iter().all(|b| b.max_gross_weight_kg < config.tare_weight_kg) {
            return Err(fail(format!("vehicle '{}': no weight band covers tare weight", vehicle.type_id)));
        }
        for task in ctx.problem.plan.jobs.iter().flat_map(|job| job.all_tasks_iter()) {
            if task.demand.as_ref().is_some_and(|d| d.get(config.mass_dimension_index).is_none_or(|mass| *mass < 0)) {
                return Err(fail("every demand must include a nonnegative mass at massDimensionIndex".into()));
            }
        }
    }
    Ok(())
}
