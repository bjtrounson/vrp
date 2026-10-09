use serde_json::json;
use vrp_uniffi::models::problem::{Matrix, Problem, VehicleProfile};
use vrp_uniffi::solver::VrpSolver;

#[test]
fn optional_weight_configuration_round_trips_through_the_binding() {
    let profile: VehicleProfile = serde_json::from_value(json!({"matrix": "light"})).unwrap();
    assert!(profile.weight_routing.is_none());
    assert_eq!(serde_json::to_value(profile).unwrap(), json!({"matrix": "light"}));

    let problem: Problem = serde_json::from_value(json!({
        "plan": {"jobs": [{"id": "farm", "pickups": [{
            "places": [{"location": {"index": 1}, "duration": 0}], "demand": [1000, 1030]
        }]}]},
        "fleet": {
            "profiles": [{"name": "light"}, {"name": "loaded"}],
            "vehicles": [{
                "typeId": "truck", "vehicleIds": ["truck-1"],
                "profile": {"matrix": "light", "weightRouting": {
                    "tareWeightKg": 12000, "massDimensionIndex": 1,
                    "bands": [
                        {"maxGrossWeightKg": 12500, "matrix": "light"},
                        {"maxGrossWeightKg": 15000, "matrix": "loaded"}
                    ]
                }},
                "costs": {"distance": 1, "time": 1}, "capacity": [10000, 10000],
                "shifts": [{"start": {"earliest": "2020-01-01T00:00:00Z", "location": {"index": 0}},
                    "end": {"latest": "2020-01-02T00:00:00Z", "location": {"index": 0}}}]
            }]
        }
    }))
    .unwrap();
    let matrices: Vec<Matrix> = serde_json::from_value(json!([
        {"profile": "light", "travelTimes": [0, 10, 10, 0], "distances": [0, 10, 10, 0]},
        {"profile": "loaded", "travelTimes": [0, 30, 30, 0], "distances": [0, 30, 30, 0]}
    ]))
    .unwrap();
    let solver = VrpSolver::new();
    solver.validate(problem.clone(), matrices.clone()).unwrap();
    let solution = solver.solve(problem, matrices, Some(r#"{"termination":{"maxGenerations":10}}"#.into())).unwrap();
    assert!(solution.unassigned.is_none());
    assert_eq!(solution.statistic.distance, 40);
    assert_eq!(solution.statistic.duration, 40);
}
