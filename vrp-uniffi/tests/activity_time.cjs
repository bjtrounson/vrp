// Run against an installed, rebuilt package (or pass its absolute package directory):
// node vrp-uniffi/tests/activity_time.cjs [package-directory]
const assert = require("node:assert/strict");
const { resolve } = require("node:path");
const { pathToFileURL } = require("node:url");

async function main() {
  const packagePath = process.argv[2] ? resolve(process.argv[2]) : "@maptimy/vrp-uniffi";
  const { VrpSolver, Location, VrpError } = process.argv.includes("--esm")
    ? await import(pathToFileURL(require.resolve(packagePath).replace(/index\.js$/, "index.mjs")).href)
    : require(packagePath);

  const location = (index) => new Location.Coordinate({ lat: -40 - index, lng: 175 });
  const cases = [
    { name: "shared", locations: [1, 1] },
    { name: "combined", locations: [1], combined: true },
    { name: "separate", locations: [1, 2] },
    { name: "mixed", locations: [1, 1, 2] },
    { name: "unassigned", locations: [1, 1], unassigned: true },
  ];

  for (const { name, locations, combined, unassigned } of cases) {
    const jobs = locations.map((index, i) => ({
      id: `pickup-${i + 1}`,
      pickups: [{ places: [{ location: location(index), duration: 1 }], demand: [combined ? 2 : 1] }],
    }));
    if (unassigned) {
      jobs.push({ id: "too-large", pickups: [{ places: [{ location: location(1), duration: 1 }], demand: [11] }] });
    }
    const problem = {
      plan: { jobs },
      fleet: {
        profiles: [{ name: "truck" }],
        vehicles: [{
          typeId: "truck", vehicleIds: ["truck-1"], profile: { matrix: "truck" },
          costs: { distance: 1, time: 1 }, capacity: [10],
          shifts: [{ start: { earliest: "1970-01-01T00:00:00Z", location: location(0) },
            end: { latest: "1970-01-02T00:00:00Z", location: location(0) } }],
        }],
      },
    };
    const size = Math.max(...locations) + 1;
    const matrix = (value) => Array.from({ length: size * size }, (_, i) => Math.floor(i / size) === i % size ? 0n : value);
    const matrices = [{ profile: "truck", distances: matrix(100n), travelTimes: matrix(60n) }];
    const solver = new VrpSolver();
    try {
      solver.validate(problem, matrices);
      const solution = solver.solve(problem, matrices, JSON.stringify({ termination: { maxTime: 1 } }));
      if (name === "shared") {
        assert.throws(() => solver.solve(problem, matrices, "{invalid"), (error) => {
          assert.ok(error instanceof Error);
          assert.ok(VrpError.FormatError.instanceOf(error));
          assert.ok(error.inner.message.length > 0);
          return true;
        });
      }
      assert.equal(solution.tours.length, 1, name);
      const stops = solution.tours[0].stops;
      const pickups = stops.flatMap((stop) => stop.activities).filter((activity) => activity.typeField === "pickup");
      assert.deepEqual(pickups.map((activity) => activity.jobId).sort(), locations.map((_, i) => `pickup-${i + 1}`));
      assert.equal(Math.max(...stops.map((stop) => stop.load[0])), combined ? 2 : locations.length);
      assert.deepEqual((solution.unassigned || []).map((job) => job.jobId), unassigned ? ["too-large"] : []);
      assert.equal(Number(solution.statistic.times.serving), locations.length);
      let intervals = 0;
      for (const stop of stops) {
        const arrival = Date.parse(stop.time.arrival);
        const departure = Date.parse(stop.time.departure);
        assert.ok(Number.isFinite(arrival) && arrival <= departure);
        for (const activity of stop.activities) {
          if (activity.time) {
            const start = Date.parse(activity.time.start);
            const end = Date.parse(activity.time.end);
            assert.ok(arrival <= start && start <= end && end <= departure);
            assert.equal(end - start, 1000);
            assert.equal(activity.time.arrival, undefined);
            intervals++;
          }
        }
      }
      assert.equal(intervals, locations.length > new Set(locations).size ? 2 : 0);
      console.log(`${name}: passed (${process.version}, ${process.platform}/${process.arch})`);
    } finally {
      solver.uniffiDestroy();
    }
  }
}

main().catch((error) => {
  console.error(error.inner?.message ?? error);
  process.exitCode = 1;
});
