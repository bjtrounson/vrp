import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const info = JSON.parse(readFileSync(join(root, "dist/build-info.json"), "utf8"));
if (info.package !== `${pkg.name}@${pkg.version}` ||
    info.generator !== pkg.devDependencies["uniffi-bindgen-react-native"] ||
    info.runtime !== pkg.dependencies["@ubjs/node"]) {
  throw new Error("Build output is stale. Run npm run build again.");
}
const libraries = ["vrp_uniffi.dll", "libvrp_uniffi.so"];
const required = ["index.js", "index.mjs", "index.d.ts", "vrp_uniffi.d.ts", "LICENSE"];
if (process.argv.includes("--release")) required.push(...libraries);
else if (!libraries.some((name) => existsSync(join(root, "dist", name)))) {
  throw new Error("The package has no native library. Run npm run build first.");
}
for (const name of required) {
  if (!existsSync(join(root, "dist", name))) throw new Error(`Missing dist/${name}`);
}
