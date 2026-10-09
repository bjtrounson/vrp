use crate::format::problem::{Matrix, PragmaticProblem, Problem};
use crate::helpers::*;
use serde_json::{Value, json};

fn pickup(id: &str, index: usize, mass: i64) -> Value {
    json!({"id": id, "pickups": [{"places": [{"location": {"index": index}, "duration": 0}], "demand": [1, mass]}]})
}

fn problem(jobs: Vec<Value>) -> Problem {
    serde_json::from_value(json!({
        "plan": {"jobs": jobs},
        "fleet": {
            "vehicles": [{
                "typeId": "truck", "vehicleIds": ["truck_1"],
                "profile": {"matrix": "light", "weightRouting": {
                    "tareWeightKg": 12000, "massDimensionIndex": 1,
                    "bands": [
                        {"maxGrossWeightKg": 31500, "matrix": "heavy"},
                        {"maxGrossWeightKg": 18000, "matrix": "light"},
                        {"maxGrossWeightKg": 23000, "matrix": "medium"}
                    ]
                }},
                "costs": {"distance": 1, "time": 1}, "capacity": [100, 100000],
                "shifts": [{
                    "start": {"earliest": "2020-01-01T00:00:00Z", "latest": "2020-01-01T00:00:00Z", "location": {"index": 0}},
                    "end": {"latest": "2020-01-02T00:00:00Z", "location": {"index": 0}}
                }]
            }],
            "profiles": [{"name": "light"}, {"name": "medium"}, {"name": "heavy"}]
        }
    })).unwrap()
}

fn matrices(size: usize) -> Vec<Matrix> {
    [("light", 10), ("medium", 30), ("heavy", 100)]
        .into_iter()
        .map(|(name, cost)| {
            let values = (0..size * size).map(|i| if i / size == i % size { 0 } else { cost }).collect::<Vec<_>>();
            Matrix {
                profile: Some(name.into()),
                timestamp: None,
                travel_times: values.clone(),
                distances: values,
                error_codes: None,
            }
        })
        .collect()
}

#[test]
fn uses_arbitrary_inclusive_thresholds_and_departure_mass() {
    for (mass, expected) in [(6000, 20), (6001, 40), (11000, 40), (11001, 110), (19500, 110)] {
        let solution = solve_with_cheapest_insertion(problem(vec![pickup("farm", 1, mass)]), Some(matrices(2)));
        assert!(solution.unassigned.is_none(), "mass: {mass}");
        assert_eq!(solution.statistic.distance, expected, "mass: {mass}");
        assert_eq!(solution.statistic.duration, expected, "mass: {mass}");
        assert_eq!(solution.statistic.cost, 2. * expected as f64);
    }
}

#[test]
fn rejects_weight_above_supplied_coverage() {
    let solution = solve_with_cheapest_insertion(problem(vec![pickup("farm", 1, 19501)]), Some(matrices(2)));
    assert!(solution.tours.is_empty());
    assert_eq!(solution.unassigned.unwrap()[0].job_id, "farm");
}

#[test]
fn does_not_use_a_lighter_matrix_for_an_unreachable_loaded_leg() {
    let mut data = matrices(2);
    data[1].error_codes = Some(vec![0, 0, -1, 0]);
    let solution = solve_with_cheapest_insertion(problem(vec![pickup("farm", 1, 7000)]), Some(data));
    assert!(solution.tours.is_empty());
    assert!(solution.unassigned.is_some());
}

#[test]
fn loads_deliveries_at_departure() {
    let delivery = json!({"id": "delivery", "deliveries": [{"places": [{"location": {"index": 1}, "duration": 0}], "demand": [1, 7000]}]});
    let solution = solve_with_cheapest_insertion(problem(vec![delivery]), Some(matrices(2)));
    assert!(solution.unassigned.is_none());
    assert_eq!(solution.statistic.distance, 40);
    assert_eq!(solution.tours[0].stops[0].load(), &vec![1, 7000]);
}

#[test]
fn evaluates_complete_pickup_delivery_pair() {
    let job = json!({"id": "shipment",
        "pickups": [{"places": [{"location": {"index": 1}, "duration": 0, "tag": "pickup"}], "demand": [1, 7000]}],
        "deliveries": [{"places": [{"location": {"index": 2}, "duration": 0, "tag": "delivery"}], "demand": [1, 7000]}]
    });
    let mut data = matrices(3);
    // A pickup without its delivery cannot return to the depot while loaded.
    data[1].error_codes = Some(vec![0, 0, 0, 1, 0, 0, 1, 0, 0]);
    let solution = solve_with_cheapest_insertion(problem(vec![job]), Some(data));
    assert!(solution.unassigned.is_none());
    assert_eq!(solution.statistic.distance, 50);
}

#[test]
fn reorders_pickups_to_avoid_a_downstream_weight_restriction() {
    let mut data = matrices(3);
    // Farm B can only be exited while light, so it must precede the heavy pickup at A.
    data[1].error_codes = Some(vec![0, 0, 0, 0, 0, 0, 1, 1, 0]);
    data[2].error_codes = data[1].error_codes.clone();
    let solution = solve_with_metaheuristic_and_iterations(
        problem(vec![pickup("A", 1, 7000), pickup("B", 2, 1000)]),
        Some(data),
        20,
    );
    assert!(solution.unassigned.is_none());
    let ids =
        solution.tours[0].stops.iter().flat_map(|s| s.activities()).map(|a| a.job_id.as_str()).collect::<Vec<_>>();
    assert_eq!(ids, vec!["departure", "B", "A", "arrival"]);
    assert_eq!(solution.statistic.distance, 50);
}

#[test]
fn rejects_invalid_weight_configuration() {
    let base = problem(vec![pickup("farm", 1, 7000)]);
    for mutation in 0..6 {
        let mut problem = base.clone();
        let config = problem.fleet.vehicles[0].profile.weight_routing.as_mut().unwrap();
        match mutation {
            0 => config.bands.clear(),
            1 => config.bands[0].max_gross_weight_kg = config.bands[1].max_gross_weight_kg,
            2 => config.bands[0].matrix = "missing".into(),
            3 => config.mass_dimension_index = 2,
            4 => config.tare_weight_kg = -1,
            _ => config.tare_weight_kg = 40000,
        }
        assert!((problem, matrices(2)).read_pragmatic().is_err(), "mutation: {mutation}");
    }
    assert!(base.read_pragmatic().is_err());
}

#[test]
fn checks_weighted_route_limits_and_downstream_time_windows() {
    for limit in ["maxDistance", "maxDuration"] {
        let mut value = serde_json::to_value(problem(vec![pickup("farm", 1, 7000)])).unwrap();
        value["fleet"]["vehicles"][0]["limits"] = json!({limit: 39});
        let solution = solve_with_cheapest_insertion(serde_json::from_value(value).unwrap(), Some(matrices(2)));
        assert!(solution.tours.is_empty(), "{limit}");
    }
    let mut value = serde_json::to_value(problem(vec![pickup("farm", 1, 7000)])).unwrap();
    value["fleet"]["vehicles"][0]["shifts"][0]["end"]["latest"] = json!("2020-01-01T00:00:39Z");
    let solution = solve_with_cheapest_insertion(serde_json::from_value(value).unwrap(), Some(matrices(2)));
    assert!(solution.tours.is_empty());
}

#[test]
fn unloads_at_a_repeated_depot_before_the_next_trip() {
    let mut value = serde_json::to_value(problem(vec![pickup("A", 1, 7000), pickup("B", 2, 7000)])).unwrap();
    value["fleet"]["vehicles"][0]["capacity"] = json!([1, 7000]);
    value["fleet"]["vehicles"][0]["shifts"][0]["reloads"] = json!([{"location": {"index": 0}, "duration": 0}]);
    let solution =
        solve_with_metaheuristic_and_iterations(serde_json::from_value(value).unwrap(), Some(matrices(3)), 20);
    assert!(solution.unassigned.is_none());
    assert_eq!(solution.statistic.distance, 80);
    let reload =
        solution.tours[0].stops.iter().find(|s| s.activities().iter().any(|a| a.activity_type == "reload")).unwrap();
    assert_eq!(reload.load(), &vec![0, 0]);
}

#[test]
fn supports_single_mass_dimension_and_existing_duration_scale() {
    let mut value = serde_json::to_value(problem(vec![pickup("farm", 1, 7000)])).unwrap();
    value["fleet"]["vehicles"][0]["profile"]["weightRouting"]["massDimensionIndex"] = json!(0);
    value["fleet"]["vehicles"][0]["profile"]["scale"] = json!(2);
    value["fleet"]["vehicles"][0]["capacity"] = json!([10000]);
    value["plan"]["jobs"][0]["pickups"][0]["demand"] = json!([7000]);
    let solution = solve_with_cheapest_insertion(serde_json::from_value(value).unwrap(), Some(matrices(2)));
    assert!(solution.unassigned.is_none());
    assert_eq!(solution.statistic.distance, 40);
    assert_eq!(solution.statistic.duration, 80);
}

#[test]
fn leaves_ordinary_routing_unchanged_when_configuration_is_absent() {
    let mut problem = problem(vec![pickup("farm", 1, 7000)]);
    problem.fleet.vehicles[0].profile.weight_routing = None;
    let solution = solve_with_cheapest_insertion(problem, Some(matrices(2)));
    assert!(solution.unassigned.is_none());
    assert_eq!(solution.statistic.distance, 20);
    assert_eq!(solution.statistic.duration, 20);
}

#[test]
fn shares_matrices_across_vehicles_with_different_tare_weights_and_ordinary_routing() {
    let mut value = serde_json::to_value(problem(vec![pickup("farm", 1, 7000)])).unwrap();
    let template = value["fleet"]["vehicles"][0].clone();
    let mut vehicles = vec![];
    let mut jobs = vec![];
    for (id, tare) in [("light_truck", Some(12000)), ("heavy_truck", Some(19000)), ("ordinary", None)] {
        let mut vehicle = template.clone();
        vehicle["typeId"] = json!(id);
        vehicle["vehicleIds"] = json!([id]);
        vehicle["skills"] = json!([id]);
        if let Some(tare) = tare {
            vehicle["profile"]["weightRouting"]["tareWeightKg"] = json!(tare);
        } else {
            vehicle["profile"].as_object_mut().unwrap().remove("weightRouting");
        }
        vehicles.push(vehicle);
        let mut job = pickup(id, 1, 7000);
        job["skills"] = json!({"allOf": [id]});
        jobs.push(job);
    }
    value["fleet"]["vehicles"] = json!(vehicles);
    value["plan"]["jobs"] = json!(jobs);
    let solution = solve_with_cheapest_insertion(serde_json::from_value(value).unwrap(), Some(matrices(2)));
    assert!(solution.unassigned.is_none());
    assert_eq!(solution.tours.len(), 3);
    for (id, distance) in [("light_truck", 40), ("heavy_truck", 130), ("ordinary", 20)] {
        let tour = solution.tours.iter().find(|tour| tour.vehicle_id == id).unwrap();
        assert_eq!(tour.statistic.distance, distance, "{id}");
    }
}

#[test]
fn validates_matrix_shape_and_mass_input() {
    let base = problem(vec![pickup("farm", 1, 7000)]);
    for mutation in 0..4 {
        let mut data = matrices(2);
        match mutation {
            0 => {
                data[1].distances.pop();
            }
            1 => data[1].error_codes = Some(vec![0]),
            2 => data[1].timestamp = Some("2020-01-01T00:00:00Z".into()),
            _ => data[1].travel_times[1] = -1,
        }
        assert!((base.clone(), data).read_pragmatic().is_err());
    }
    for demand in [vec![1], vec![1, -1]] {
        let mut problem = base.clone();
        problem.plan.jobs[0].pickups.as_mut().unwrap()[0].demand = Some(demand);
        assert!((problem, matrices(2)).read_pragmatic().is_err());
    }
}

#[test]
fn revalidates_remaining_route_after_a_removal_changes_its_matrix() {
    use std::sync::Arc;
    use vrp_core::construction::heuristics::InsertionContext;
    use vrp_core::rosomaxa::evolution::TelemetryMode;
    use vrp_core::solver::search::{Recreate, RecreateWithCheapest};
    use vrp_core::solver::{RefinementContext, create_elitism_population};
    use vrp_core::utils::Environment;

    let mut data = matrices(3);
    data[0].error_codes = Some(vec![0, 0, 0, 0, 0, 0, 1, 1, 0]);
    let core = Arc::new((problem(vec![pickup("A", 1, 6000), pickup("B", 2, 1000)]), data).read_pragmatic().unwrap());
    let environment = Arc::new(Environment::default());
    let population = create_elitism_population(core.goal.clone(), environment.clone());
    let refinement =
        RefinementContext::new(core.clone(), Box::new(population), TelemetryMode::None, environment.clone());
    let mut ctx = RecreateWithCheapest::new(environment.random.clone())
        .run(&refinement, InsertionContext::new(core.clone(), environment));
    assert_eq!(ctx.solution.routes[0].route().tour.job_count(), 2);
    let job = core.jobs.all()[0].clone();
    assert!(ctx.solution.routes[0].route_mut().tour.remove(&job));
    ctx.solution.required.push(job);
    ctx.restore();
    assert!(ctx.solution.routes.is_empty());
    assert_eq!(ctx.solution.required.len(), 2);
}

#[test]
fn accounts_for_a_required_break_on_a_loaded_leg() {
    let mut value = serde_json::to_value(problem(vec![pickup("farm", 1, 7000)])).unwrap();
    value["fleet"]["vehicles"][0]["shifts"][0]["breaks"] = json!([{
        "time": {"earliest": "2020-01-01T00:00:20Z", "latest": "2020-01-01T00:00:20Z"}, "duration": 5
    }]);
    let solution =
        solve_with_metaheuristic_and_iterations(serde_json::from_value(value).unwrap(), Some(matrices(2)), 10);
    assert!(solution.unassigned.is_none());
    assert_eq!(solution.statistic.distance, 40);
    assert_eq!(solution.statistic.duration, 45);
}
