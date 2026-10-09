import { spawnSync } from "node:child_process";
import { copyFileSync, mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const npm = process.env.npm_execpath;
if (!npm) throw new Error("Run this check using npm run test:package.");
function run(command, args, cwd, capture = false) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8", stdio: capture ? "pipe" : "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(result.stderr || `${command} exited with ${result.status}`);
  return result.stdout;
}
// Install the tarball into a clean directory so workspace dependencies cannot mask missing package files.
const consumer = mkdtempSync(join(tmpdir(), "vrp-package-"));
const output = resolve(root, "../../target/node-package");
mkdirSync(output, { recursive: true });
const packed = JSON.parse(run(process.execPath, [npm, "pack", "--json", "--pack-destination", output], root, true));
const tarball = join(output, packed[0].filename);
writeFileSync(join(consumer, "package.json"), '{"name":"vrp-package-check","private":true}\n');
run(process.execPath, [npm, "install", "--ignore-scripts", "--no-audit", "--no-fund", tarball], consumer);
const installed = join(consumer, "node_modules/@maptimy/vrp-uniffi");
for (const name of ["activity_time.cjs", "weight_routing.cjs"]) {
  const regression = resolve(root, "../tests", name);
  run(process.execPath, [regression, installed], consumer);
  run(process.execPath, [regression, installed, "--esm"], consumer);
}
// Check real package-name resolution for both module systems.
writeFileSync(join(consumer, "check.cjs"), 'const { VrpSolver } = require("@maptimy/vrp-uniffi"); new VrpSolver().uniffiDestroy();\n');
writeFileSync(join(consumer, "check.mjs"), 'import { VrpSolver } from "@maptimy/vrp-uniffi"; new VrpSolver().uniffiDestroy();\n');
run(process.execPath, ["check.cjs"], consumer);
run(process.execPath, ["check.mjs"], consumer);
writeFileSync(join(consumer, "check.ts"), `
import { Location, VrpSolver, type Activity, type Stop, type VehicleProfile } from "@maptimy/vrp-uniffi";
const profile: VehicleProfile = { matrix: "light", weightRouting: {
  tareWeightKg: 12000n, massDimensionIndex: 1,
  bands: [{ maxGrossWeightKg: 18000n, matrix: "light" }],
} };
const location: Location = new Location.Coordinate({ lat: -41, lng: 175 });
const activity: Activity = { jobId: "pickup", typeField: "pickup", time: { start: "a", end: "b" } };
const stop: Stop = { location, time: { arrival: "a", departure: "b" }, distance: 0n, load: [0], activities: [activity] };
const solver = new VrpSolver();
solver.uniffiDestroy();
`);
copyFileSync(join(consumer, "check.ts"), join(consumer, "check.mts"));
run(process.execPath, [join(root, "node_modules/typescript/bin/tsc"), "--noEmit", "--strict", "--skipLibCheck",
  "--target", "ES2022", "--module", "NodeNext", "--moduleResolution", "NodeNext", "check.ts", "check.mts"], consumer);
console.log(`Verified packed package: ${tarball}\nClean consumer: ${consumer}`);
