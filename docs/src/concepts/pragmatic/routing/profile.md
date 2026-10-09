# Routing matrix profiles

In order to solve VRP, you need to specify at least one routing matrix profile.


## Usage

Routing matrix profiles are defined in `fleet.profiles`:

```json
{{#include ../../../../../examples/data/pragmatic/simple.basic.problem.json:135:139}}
```

The `name` must be unique for each matrix profile and it should referenced by `profile.matrix` property defined on vehicle:

```json
{{#include ../../../../../examples/data/pragmatic/simple.basic.problem.json:104:106}}
```

Use `-m` option to pass the matrix:

    vrp-cli solve pragmatic problem.json -m routing_matrix.json -o solution.json

If you don't pass any routing matrix, then [haversine formula](https://en.wikipedia.org/wiki/Haversine_formula) is used to
calculate distances between geo locations. Durations are calculated using speed value defined via `speed` property in
each profile. It is optional, default value is `10` which corresponds to `10m/s`.


## Multiple profiles

In general, you're not limited to one single routing profile. You can define multiple ones and pass their matrices
to the solver:

    vrp-cli solve pragmatic problem.json -m routing_matrix_car.json -m routing_matrix_truck.json

Make sure that for all profile names in `fleet.profiles` you have the corresponding matrix specified.

See [multiple profiles example](../../../examples/pragmatic/basics/profiles.md).

## Weight-dependent routing

A vehicle type can select a different matrix for each leg using its gross weight after the preceding
activity. Add the optional `weightRouting` object to the vehicle's `profile`:

```json
{
  "matrix": "truck-light",
  "weightRouting": {
    "tareWeightKg": 12000,
    "massDimensionIndex": 1,
    "bands": [
      { "maxGrossWeightKg": 18000, "matrix": "truck-light" },
      { "maxGrossWeightKg": 23000, "matrix": "truck-medium" },
      { "maxGrossWeightKg": 31500, "matrix": "truck-heavy" }
    ]
  }
}
```

Declare every referenced profile in `fleet.profiles` and supply its matrix through the existing matrix
inputs. Matrix files do not contain `weightRouting`. Vehicles may share matrices while using different
tare weights. The application owns matrix generation, units conversion, caching, and threshold spacing;
there is no interval-size setting in the solver.

`massDimensionIndex` is a zero-based index into job demand and vehicle capacity arrays, not product
density. For example, `demand: [1000, 1030]` with index `1` represents 1,030 kg of product; the other
dimension may represent litres. Supply integer kilograms, including in `tareWeightKg` and thresholds.
The application calculates product mass before calling the solver. Capacity continues to limit payload
independently of routing thresholds.

Gross weight is tare plus onboard mass. The solver accounts for initial deliveries, pickups,
deliveries, and unloading/loading at reload stops. It selects the smallest supplied threshold greater
than or equal to the departure weight. Thresholds can be uneven and may be supplied in any order;
duplicates are invalid. A weight of exactly 23,000 kg uses the 23,000 kg matrix. Weights above all
thresholds, or nonzero `errorCodes` in the selected matrix, make a leg infeasible. The solver does not
interpolate across weight bands or fall back to a lighter matrix.

Weight-aware insertion evaluates the candidate route's full load sequence, schedules, reachability,
costs, and travel limits. Paired pickups and deliveries are evaluated together. Solution statistics and
the solution checker use the same thresholds. Search moves that invalidate an existing weighted route
return its jobs for reassignment. This evaluation is more expensive than ordinary insertion, especially
for jobs with many paired activities; benchmark representative workloads before increasing problem size.

The initial implementation requires explicit, non-timestamped matrices and does not support combining
weight routing with vicinity clustering or recharge stations. These combinations return a validation
error. Reload stops and vehicle breaks remain available. Without `weightRouting`, existing inputs keep
their previous behaviour. Rust struct literals must set the new optional field to `None`; consumers of
generated bindings must rebuild against the updated library.


## Time dependent routing

In order to use this feature, specify more than one routing matrix for each profile with timestamp property set.

