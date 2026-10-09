import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";

const packageDir = dirname(fileURLToPath(import.meta.url));
const workspace = resolve(packageDir, "../..");
const generated = join(packageDir, "generated");
const dist = join(packageDir, "dist");
const platform = {
  win32: { target: "x86_64-pc-windows-msvc", library: "vrp_uniffi.dll" },
  linux: { target: "x86_64-unknown-linux-gnu", library: "libvrp_uniffi.so" },
}[process.platform];
if (!platform || process.arch !== "x64") throw new Error("Build on Windows x64 or Linux x64 (glibc).");

function run(command, args, cwd = workspace) {
  const result = spawnSync(command, args, { cwd, stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} exited with ${result.status}`);
}

// --native-dir packages CI-built libraries; without it, build the host library locally.
const args = process.argv.slice(2);
if (args.length && (args.length !== 2 || args[0] !== "--native-dir")) {
  throw new Error("Usage: node build.mjs [--native-dir DIRECTORY]");
}
let nativeDir;
if (args.length) {
  nativeDir = resolve(args[1]);
} else {
  run("cargo", ["build", "--locked", "--release", "-p", "vrp-uniffi", "--lib", "--target", platform.target,
    "--target-dir", join(workspace, "target")]);
  nativeDir = join(workspace, "target", platform.target, "release");
}
const library = join(nativeDir, platform.library);
if (!existsSync(library)) throw new Error(`Missing native library: ${library}`);

// These exact directories are generated outputs underneath this package, never caller-supplied paths.
for (const directory of [generated, dist]) {
  if (dirname(directory) !== packageDir) throw new Error("Invalid output directory");
  rmSync(directory, { recursive: true, force: true });
  mkdirSync(directory, { recursive: true });
}
run("cargo", [
  "run", "--locked", "--manifest-path",
  join(packageDir, "node_modules/uniffi-bindgen-react-native/crates/ubrn_cli/Cargo.toml"),
  "--", "generate", "napi", "bindings", "--library", library,
  "--ts-dir", generated, "--lib-colocated", "--no-format",
]);

const common = {
  entryPoints: [join(generated, "index.ts")],
  bundle: true, platform: "node", target: "node18", packages: "external", sourcemap: false,
};
await build({ ...common, format: "esm", outfile: join(dist, "index.mjs") });
// The generated loader uses import.meta.url. Supply the equivalent in CommonJS.
await build({
  ...common, format: "cjs", outfile: join(dist, "index.js"),
  define: { "import.meta.url": "__uniffiModuleUrl" },
  banner: { js: 'const __uniffiModuleUrl = require("node:url").pathToFileURL(__filename).href;' },
});
run(process.execPath, [join(packageDir, "node_modules/typescript/bin/tsc"), "-p", join(packageDir, "tsconfig.json")]);
for (const name of ["vrp_uniffi.dll", "libvrp_uniffi.so"]) {
  if (existsSync(join(nativeDir, name))) copyFileSync(join(nativeDir, name), join(dist, name));
}
copyFileSync(join(workspace, "LICENSE"), join(dist, "LICENSE"));
const pkg = JSON.parse(readFileSync(join(packageDir, "package.json"), "utf8"));
writeFileSync(join(dist, "build-info.json"), JSON.stringify({
  package: `${pkg.name}@${pkg.version}`,
  generator: pkg.devDependencies["uniffi-bindgen-react-native"],
  runtime: pkg.dependencies["@ubjs/node"],
}, null, 2) + "\n");
console.log(`Built ${pkg.name}@${pkg.version} in ${dist}`);
