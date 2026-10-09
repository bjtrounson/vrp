use serde_json::{Value, json};
use std::io::BufReader;
use std::sync::Arc;
use vrp_cli::extensions::solve::config::read_config;
use vrp_pragmatic::format::problem::PragmaticProblem;
use vrp_uniffi::models::solution::{Activity, Solution, Stop};
use vrp_uniffi::solver::VrpSolver;

#[test]
fn activity_intervals_preserve_exact_values_without_changing_stop_times() {
    let stop: Stop = serde_json::from_value(json!({
        "location": {"lat": -41, "lng": 175},
        "time": {"arrival": "1970-01-01T00:01:00Z", "departure": "1970-01-01T00:01:02Z"},
        "distance": 100, "load": [2],
        "activities": [{"jobId": "pickup-1", "type": "pickup",
            "time": {"start": "1970-01-01T00:01:00Z", "end": "1970-01-01T00:01:01Z"}},
            {"jobId": "pickup-2", "type": "pickup",
            "time": {"start": "1970-01-01T00:01:01Z", "end": "1970-01-01T00:01:02Z"}}]
    }))
    .unwrap();
    assert_eq!(stop.time.arrival, "1970-01-01T00:01:00Z");
    assert_eq!(stop.time.departure, "1970-01-01T00:01:02Z");
    for (activity, (start, end)) in stop
        .activities
        .iter()
        .zip([("1970-01-01T00:01:00Z", "1970-01-01T00:01:01Z"), ("1970-01-01T00:01:01Z", "1970-01-01T00:01:02Z")])
    {
        let time = activity.time.as_ref().unwrap();
        assert_eq!(time.start, start);
        assert_eq!(time.end, end);
        assert_eq!(serde_json::to_value(activity).unwrap()["time"], json!({"start": start, "end": end}));
    }
}

#[test]
fn optional_activity_times_remain_optional() {
    for value in [json!({"jobId": "a", "type": "pickup"}), json!({"jobId": "a", "type": "pickup", "time": null})] {
        let activity: Activity = serde_json::from_value(value).unwrap();
        assert!(activity.time.is_none());
        assert!(serde_json::to_value(activity).unwrap().get("time").is_none());
    }
}

#[test]
fn malformed_activity_intervals_and_stop_schedules_are_rejected() {
    for time in [
        json!({}),
        json!({"start": "a"}),
        json!({"end": "b"}),
        json!({"start": null, "end": "b"}),
        json!({"start": "a", "end": 1}),
        json!({"arrival": "a", "departure": "b"}),
        json!([]),
    ] {
        assert!(serde_json::from_value::<Activity>(json!({"jobId": "a", "type": "pickup", "time": time})).is_err());
    }
    assert!(
        serde_json::from_value::<Stop>(json!({
            "location": {"index": 0}, "time": {"start": "a", "end": "b"},
            "distance": 0, "load": [0], "activities": []
        }))
        .is_err()
    );
}

fn problem(locations: &[usize], combined: bool, unassigned: bool) -> (Value, Value) {
    let mut jobs: Vec<_> = locations.iter().enumerate().map(|(i, location)| json!({
        "id": format!("pickup-{}", i + 1),
        "pickups": [{"places": [{"location": {"index": location}, "duration": 1}], "demand": [if combined {2} else {1}]}]
    })).collect();
    if unassigned {
        jobs.push(json!({"id": "too-large", "pickups": [{"places": [{"location": {"index": 1}, "duration": 1}], "demand": [11]}]}));
    }
    let size = locations.iter().max().unwrap() + 1;
    let distances: Vec<_> = (0..size * size).map(|i| if i / size == i % size { 0 } else { 100 }).collect();
    let travel_times: Vec<_> = distances.iter().map(|d| if *d == 0 { 0 } else { 60 }).collect();
    (
        json!({
            "plan": {"jobs": jobs},
            "fleet": {"profiles": [{"name": "truck"}], "vehicles": [{
                "typeId": "truck", "vehicleIds": ["truck-1"], "profile": {"matrix": "truck"},
                "costs": {"distance": 1, "time": 1}, "capacity": [10],
                "shifts": [{"start": {"earliest": "1970-01-01T00:00:00Z", "location": {"index": 0}},
                    "end": {"latest": "1970-01-02T00:00:00Z", "location": {"index": 0}}}]
            }]}
        }),
        json!({"profile": "truck", "distances": distances, "travelTimes": travel_times}),
    )
}

#[test]
fn solver_preserves_pickups_loads_and_unassigned_jobs() {
    for (name, locations, combined, unassigned) in [
        ("shared", vec![1, 1], false, false),
        ("combined", vec![1], true, false),
        ("separate", vec![1, 2], false, false),
        ("mixed", vec![1, 1, 2], false, false),
        ("unassigned", vec![1, 1], false, true),
    ] {
        let (problem, matrix) = problem(&locations, combined, unassigned);
        let solver = VrpSolver::new();
        let problem: vrp_uniffi::models::problem::Problem = serde_json::from_value(problem).unwrap();
        let matrices: Vec<vrp_uniffi::models::problem::Matrix> = vec![serde_json::from_value(matrix).unwrap()];
        solver.validate(problem.clone(), matrices.clone()).unwrap();
        let solution =
            solver.solve(problem, matrices, Some(r#"{"termination":{"maxGenerations":10}}"#.into())).unwrap();
        assert_eq!(solution.tours.len(), 1, "{name}");
        let tour = &solution.tours[0];
        let mut ids: Vec<_> = tour
            .stops
            .iter()
            .flat_map(|stop| &stop.activities)
            .filter(|activity| activity.type_field == "pickup")
            .map(|activity| activity.job_id.clone())
            .collect();
        ids.sort();
        assert_eq!(ids, (1..=locations.len()).map(|i| format!("pickup-{i}")).collect::<Vec<_>>(), "{name}");
        assert_eq!(
            tour.stops.iter().map(|stop| stop.load[0]).max(),
            Some(if combined { 2 } else { locations.len() as i32 }),
            "{name}"
        );
        for stop in &tour.stops {
            assert!(stop.time.arrival <= stop.time.departure);
            for activity in &stop.activities {
                if let Some(time) = &activity.time {
                    assert!(stop.time.arrival <= time.start);
                    assert!(time.start <= time.end);
                    assert!(time.end <= stop.time.departure);
                }
            }
        }
        let unassigned_ids: Vec<_> = solution.unassigned.iter().flatten().map(|job| job.job_id.as_str()).collect();
        assert_eq!(unassigned_ids, if unassigned { vec!["too-large"] } else { vec![] });
    }
}

#[test]
fn capture_actual_pragmatic_intervals_before_wrapper_deserialization() {
    let (problem, matrix) = problem(&[1, 1], false, false);
    let problem: vrp_pragmatic::format::problem::Problem = serde_json::from_value(problem).unwrap();
    let matrix: vrp_pragmatic::format::problem::Matrix = serde_json::from_value(matrix).unwrap();
    let core_problem = (problem, vec![matrix]).read_pragmatic().unwrap();
    let config = read_config(BufReader::new(r#"{"termination":{"maxGenerations":10}}"#.as_bytes())).unwrap();
    let raw = vrp_cli::get_solution_serialized(Arc::new(core_problem), config).unwrap();
    // Synthetic input only: keep the exact upstream output visible with --nocapture.
    println!("Raw pragmatic solution before UniFFI deserialization:\n{raw}");
    let json: Value = serde_json::from_str(&raw).unwrap();
    let solution: Solution = serde_json::from_str(&raw).unwrap();
    let mut intervals = 0;
    for (raw_stop, stop) in json["tours"][0]["stops"].as_array().unwrap().iter().zip(&solution.tours[0].stops) {
        for (raw_activity, activity) in raw_stop["activities"].as_array().unwrap().iter().zip(&stop.activities) {
            if let Some(time) = &activity.time {
                assert_eq!(raw_activity["time"]["start"], time.start);
                assert_eq!(raw_activity["time"]["end"], time.end);
                assert!(raw_activity["time"].get("arrival").is_none());
                intervals += 1;
            }
        }
    }
    assert_eq!(intervals, 2);
}
