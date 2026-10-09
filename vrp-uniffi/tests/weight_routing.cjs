const assert = require("node:assert/strict");
const { resolve } = require("node:path");
const { pathToFileURL } = require("node:url");

async function main() {
  const packagePath = process.argv[2] ? resolve(process.argv[2]) : "@maptimy/vrp-uniffi";
  const { VrpSolver, Location } = process.argv.includes("--esm")
    ? await import(pathToFileURL(require.resolve(packagePath).replace(/index\.js$/, "index.mjs")).href)
    : require(packagePath);
  const location = (index) => new Location.Reference({ index });
  const problem = {
    plan: { jobs: [{ id: "farm", pickups: [{ places: [{ location: location(1), duration: 0 }], demand: [1000, 1030] }] }] },
    fleet: {
      profiles: [{ name: "light" }, { name: "loaded" }],
      vehicles: [{
        typeId: "truck", vehicleIds: ["truck-1"],
        profile: { matrix: "light", weightRouting: {
          tareWeightKg: 12000n, massDimensionIndex: 1,
          bands: [
            { maxGrossWeightKg: 12500n, matrix: "light" },
            { maxGrossWeightKg: 15000n, matrix: "loaded" },
          ],
        } },
        capacity: [10000, 10000], costs: { distance: 1, time: 1 },
        shifts: [{ start: { earliest: "2020-01-01T00:00:00Z", location: location(0) },
          end: { latest: "2020-01-02T00:00:00Z", location: location(0) } }],
      }],
    },
  };
  const matrices = [
    { profile: "light", distances: [0n, 10n, 10n, 0n], travelTimes: [0n, 10n, 10n, 0n] },
    { profile: "loaded", distances: [0n, 30n, 30n, 0n], travelTimes: [0n, 30n, 30n, 0n] },
  ];
  const solver = new VrpSolver();
  try {
    solver.validate(problem, matrices);
    const solution = solver.solve(problem, matrices, JSON.stringify({ termination: { maxGenerations: 10 } }));
    assert.equal(solution.statistic.distance, 40n);
    assert.equal(solution.statistic.duration, 40n);
    assert.equal(solution.unassigned?.length ?? 0, 0);
    console.log(`weight routing: passed (${process.version}, ${process.platform}/${process.arch})`);
  } finally {
    solver.uniffiDestroy();
  }
}

main().catch((error) => {
  console.error(error.inner?.message ?? error);
  process.exitCode = 1;
});
