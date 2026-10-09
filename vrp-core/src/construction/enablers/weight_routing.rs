//! Load-dependent selection among caller-supplied matrix profiles.

use super::get_route_intervals;
use crate::construction::features::JobDemandDimension;
use crate::construction::heuristics::{ActivityContext, RouteContext, RouteState, SolutionContext};
use crate::models::common::*;
use crate::models::problem::Job;
use crate::models::problem::{ActivityCost, TransportCost, TravelTime};
use crate::models::solution::{Activity, Route};
use crate::models::{Feature, FeatureBuilder, FeatureState};
use rosomaxa::prelude::{Float, GenericResult};
use rosomaxa::utils::UnwrapValue;
use std::collections::HashMap;
use std::sync::Arc;

/// The weight thresholds and mass dimension used by one vehicle.
#[derive(Clone)]
pub struct WeightRouting {
    /// Empty vehicle/combination mass in kilograms.
    pub tare_weight_kg: i64,
    /// Zero-based demand dimension containing kilograms.
    pub mass_dimension_index: usize,
    /// Increasing inclusive gross-weight limits and their matrix profiles.
    pub bands: Vec<(i64, Profile)>,
    /// Identifies activities that unload static pickups and load static deliveries.
    pub is_reload: fn(&Activity) -> bool,
    /// Existing vehicle distance limit, used when checking routes after removal moves.
    pub max_distance: Option<Distance>,
    /// Existing vehicle duration limit.
    pub max_duration: Option<Duration>,
}

custom_dimension!(pub VehicleWeightRouting typeof WeightRouting);
custom_tour_state!(pub(crate) PartialWeightInsertion typeof bool);

impl WeightRouting {
    /// Selects the smallest supplied limit covering this gross weight.
    pub fn profile(&self, gross_weight_kg: i64) -> Option<&Profile> {
        let index = self.bands.partition_point(|(limit, _)| *limit < gross_weight_kg);
        (gross_weight_kg >= self.tare_weight_kg).then(|| self.bands.get(index)).flatten().map(|(_, p)| p)
    }
}

/// Returns whether the vehicle uses weight-dependent routing.
pub fn has_weight_routing(route: &Route) -> bool {
    route.actor.vehicle.dimens.get_vehicle_weight_routing().is_some()
}

/// Recomputes departure weights by activity index, including initial loads and reloads.
pub fn route_weights(route: &Route) -> Option<Vec<i64>> {
    let config = route.actor.vehicle.dimens.get_vehicle_weight_routing()?;
    let demand = |activity: &Activity| {
        activity.job.as_ref().map_or((0, 0, 0), |job| {
            if let Some(d) = job.dimens.get_job_demand::<MultiDimLoad>() {
                let i = config.mass_dimension_index;
                let value = |v: MultiDimLoad| i64::from(v.load.get(i).copied().unwrap_or_default());
                (
                    value(d.delivery.0),
                    value(d.pickup.0),
                    value(d.pickup.0) + value(d.pickup.1) - value(d.delivery.0) - value(d.delivery.1),
                )
            } else if let Some(d) = job.dimens.get_job_demand::<SingleDimLoad>() {
                (
                    i64::from(d.delivery.0.value),
                    i64::from(d.pickup.0.value),
                    i64::from(d.pickup.0.value) + i64::from(d.pickup.1.value)
                        - i64::from(d.delivery.0.value)
                        - i64::from(d.delivery.1.value),
                )
            } else {
                (0, 0, 0)
            }
        })
    };
    let mut weights = vec![config.tare_weight_kg; route.tour.total()];
    let mut carried = 0_i64;
    for (start, end) in get_route_intervals(route, config.is_reload) {
        let activities = route.tour.activities_slice(start, end);
        let (delivery, pickup) = activities
            .iter()
            .map(demand)
            .fold((0_i64, 0_i64), |(d, p), (sd, sp, _)| (d.saturating_add(sd), p.saturating_add(sp)));
        carried = carried.saturating_add(delivery);
        for (offset, activity) in activities.iter().enumerate() {
            carried = carried.saturating_add(demand(activity).2);
            weights[start + offset] = config.tare_weight_kg.saturating_add(carried);
        }
        carried = carried.saturating_sub(pickup);
    }
    Some(weights)
}

/// Result of evaluating all legs of a weighted route.
pub struct WeightedRouteEvaluation {
    /// Recomputed activity schedules.
    pub schedules: Vec<Schedule>,
    /// Total distance.
    pub distance: Distance,
    /// Elapsed route duration.
    pub duration: Duration,
    /// Variable travel, service and waiting cost.
    pub cost: Cost,
    /// All legs have a covering matrix and are reachable.
    pub reachable: bool,
    /// All activities and the vehicle shift fit their time windows.
    pub time_feasible: bool,
}

/// Evaluates a complete route using explicit departure weights.
pub fn evaluate_weighted_route(
    route: &Route,
    activity_cost: &dyn ActivityCost,
    transport: &dyn TransportCost,
) -> WeightedRouteEvaluation {
    let config = route.actor.vehicle.dimens.get_vehicle_weight_routing().unwrap();
    let weights = route_weights(route).unwrap();
    let start = route.tour.start().unwrap();
    let mut result = WeightedRouteEvaluation {
        schedules: vec![start.schedule.clone()],
        distance: 0.,
        duration: 0.,
        cost: 0.,
        reachable: true,
        time_feasible: true,
    };
    let actor = &route.actor;
    for index in 1..route.tour.total() {
        let from = route.tour.get(index - 1).unwrap();
        let to = route.tour.get(index).unwrap();
        let departure = result.schedules[index - 1].departure;
        let profile = config.profile(weights[index - 1]);
        result.reachable &= profile.is_some();
        // Incomplete paired insertions still need finite estimates; feasibility is checked when complete.
        let profile = profile.unwrap_or(&config.bands[0].1);
        let time = TravelTime::Departure(departure);
        let duration = transport.duration_with_profile(route, profile, from.place.location, to.place.location, time);
        let distance = transport.distance_with_profile(route, profile, from.place.location, to.place.location, time);
        result.reachable &= duration >= 0. && distance >= 0. && duration.is_finite() && distance.is_finite();
        let duration = if duration.is_finite() { duration.max(0.) } else { 0. };
        let distance = if distance.is_finite() { distance.max(0.) } else { 0. };
        let arrival = departure + duration;
        let end = activity_cost.estimate_departure(route, to, arrival);
        result.time_feasible &= matches!(end, std::ops::ControlFlow::Continue(_)) && arrival <= to.place.time.end;
        let end = end.unwrap_value();
        result.distance += distance;
        result.cost += distance * (actor.vehicle.costs.per_distance + actor.driver.costs.per_distance)
            + duration * (actor.vehicle.costs.per_driving_time + actor.driver.costs.per_driving_time)
            + activity_cost.cost(route, to, arrival);
        result.schedules.push(Schedule::new(arrival, end));
    }
    let end = result.schedules.last().unwrap().departure;
    result.duration = end - start.schedule.departure;
    result.time_feasible &= end <= actor.detail.time.end;
    result
}

/// Evaluates an insertion without changing the original route or using stale load state.
pub fn evaluate_weighted_insertion(
    route_ctx: &RouteContext,
    activity_ctx: &ActivityContext,
    activity: &dyn ActivityCost,
    transport: &dyn TransportCost,
) -> (WeightedRouteEvaluation, WeightedRouteEvaluation) {
    let before = evaluate_weighted_route(route_ctx.route(), activity, transport);
    let mut candidate = route_ctx.route().deep_copy();
    candidate.tour.insert_at(activity_ctx.target.deep_copy(), activity_ctx.index + 1);
    let after = evaluate_weighted_route(&candidate, activity, transport);
    (before, after)
}

/// Rechecks weighted routes after solution-level edits, including removals and reload changes.
pub fn create_weight_routing_feature(
    transport: Arc<dyn TransportCost>,
    activity: Arc<dyn ActivityCost>,
) -> GenericResult<Feature> {
    struct State {
        transport: Arc<dyn TransportCost>,
        activity: Arc<dyn ActivityCost>,
    }
    impl FeatureState for State {
        fn accept_insertion(&self, _: &mut SolutionContext, _: usize, _: &Job) {}
        fn accept_route_state(&self, _: &mut RouteContext) {}
        fn accept_solution_state(&self, solution: &mut SolutionContext) {
            let invalid = solution
                .routes
                .iter()
                .filter_map(|ctx| {
                    let route = ctx.route();
                    let config = route.actor.vehicle.dimens.get_vehicle_weight_routing()?;
                    if !route.tour.has_jobs() {
                        return None;
                    }
                    let value = evaluate_weighted_route(route, self.activity.as_ref(), self.transport.as_ref());
                    let valid = value.reachable
                        && value.time_feasible
                        && config.max_distance.is_none_or(|limit| value.distance <= limit)
                        && config.max_duration.is_none_or(|limit| value.duration <= limit);
                    (!valid).then(|| (route.actor.clone(), route.tour.jobs().cloned().collect::<Vec<_>>()))
                })
                .collect::<Vec<_>>();
            for (actor, jobs) in invalid {
                solution.required.extend(jobs);
                solution.keep_routes(&|ctx| ctx.route().actor != actor);
            }
        }
    }
    FeatureBuilder::default().with_name("weight_routing").with_state(State { transport, activity }).build()
}

/// Uses optimistic estimates for profile-only search heuristics, while preserving exact matrix lookups.
pub fn with_weight_routing_estimates(
    inner: Arc<dyn TransportCost>,
    profiles: HashMap<usize, Vec<usize>>,
) -> Arc<dyn TransportCost> {
    struct Estimates {
        inner: Arc<dyn TransportCost>,
        profiles: HashMap<usize, Vec<usize>>,
    }
    impl Estimates {
        fn estimate(&self, profile: &Profile, estimate: impl Fn(&Profile) -> Float) -> Float {
            self.profiles.get(&profile.index).map_or_else(
                || estimate(profile),
                |indices| {
                    indices
                        .iter()
                        .map(|index| estimate(&Profile::new(*index, Some(profile.scale))))
                        .filter(|v| *v >= 0.)
                        .min_by(Float::total_cmp)
                        .unwrap_or(-1.)
                },
            )
        }
    }
    impl TransportCost for Estimates {
        fn duration_approx(&self, p: &Profile, from: Location, to: Location) -> Duration {
            self.estimate(p, |p| self.inner.duration_approx(p, from, to))
        }
        fn distance_approx(&self, p: &Profile, from: Location, to: Location) -> Distance {
            self.estimate(p, |p| self.inner.distance_approx(p, from, to))
        }
        fn duration(&self, route: &Route, from: Location, to: Location, t: TravelTime) -> Duration {
            self.inner.duration(route, from, to, t)
        }
        fn distance(&self, route: &Route, from: Location, to: Location, t: TravelTime) -> Distance {
            self.inner.distance(route, from, to, t)
        }
        fn duration_with_profile(
            &self,
            route: &Route,
            p: &Profile,
            from: Location,
            to: Location,
            t: TravelTime,
        ) -> Duration {
            self.inner.duration_with_profile(route, p, from, to, t)
        }
        fn distance_with_profile(
            &self,
            route: &Route,
            p: &Profile,
            from: Location,
            to: Location,
            t: TravelTime,
        ) -> Distance {
            self.inner.distance_with_profile(route, p, from, to, t)
        }
        fn size(&self) -> usize {
            self.inner.size()
        }
    }
    if profiles.is_empty() { inner } else { Arc::new(Estimates { inner, profiles }) }
}
