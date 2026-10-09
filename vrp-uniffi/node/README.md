# Native Node package

This directory builds `@maptimy/vrp-uniffi` from the Rust source in this repository.
It uses `uniffi-bindgen-react-native`'s Node/N-API backend with `@ubjs/node` and
`@ubjs/core`. It does not use WASM, `ffi-rs`, the discontinued Node generator,
or a copy of previously published bindings.

## Local build

Install Rust 1.92.0, Node.js 18 or later, and the host C/C++ linker (Visual Studio
Build Tools with Desktop development with C++ on Windows; GCC on Linux).
The supported native targets are Windows x64 MSVC and Linux x64 glibc.
macOS, ARM64 and Alpine/musl are not included in this package.

From this directory:

```sh
npm ci --ignore-scripts
npm run build
npm test
npm run test:package
```

The build compiles the host native library, reads its UniFFI metadata, generates
TypeScript, and emits CommonJS, ESM and declarations in `dist/`. The Cargo and
npm lockfiles pin the native dependencies and generation tools. No global
bindgen installation or manual generated-code edits are needed.

`test:package` writes a tarball to `../../target/node-package/`, installs it into
a fresh temporary consumer, runs all five native solve scenarios through both
module formats, and checks the installed TypeScript API. It prints the consumer
directory for inspection. A local build contains only the host native library;
it is for local testing, not a cross-platform release.

To package prebuilt libraries from the **same source revision and Cargo.lock**,
put `vrp_uniffi.dll` and `libvrp_uniffi.so` together in a directory and run:

```sh
npm run build -- --native-dir /absolute/path/to/native
node check-package.mjs --release
npm run test:package
```

The host library is used to generate shared bindings. The release check requires
both native files. `prepublishOnly` also checks them when publishing this directory.
Do not publish a locally packed host-only tarball directly: npm publishing an
existing tarball does not run this directory's release guard.

## Weight-dependent matrices

The optional `vehicle.profile.weightRouting` selects among separately supplied matrices. Set
`tareWeightKg`, `massDimensionIndex` (zero-based kilograms dimension in demands), and `bands` with
`maxGrossWeightKg` and `matrix` profile references. Thresholds are arbitrary inclusive upper bounds;
the application generates the matrices. In the generated Node types, tare and threshold kilograms
use `bigint`, while the dimension index and demand values use `number`.

Rebuild the native library and generated bindings together after updating. See the
[routing profile documentation](../../docs/src/concepts/pragmatic/routing/profile.md) for the input
contract and supported combinations, and `../tests/weight_routing.cjs` for a complete Node example.

## CI and publishing

`.github/workflows/node-package.yaml` builds both libraries, runs the Rust
regressions, generates the package once, and tests the **same tarball** on Windows
and Linux using Node 18, 20, 22 and 24. Linux CI uses Ubuntu 22.04 (glibc 2.35);
older glibc and musl environments need separate builds. The workflow runs on
relevant pull requests and can be dispatched manually from the desired branch.

For a release, update the version in `package.json` and its lockfile, configure
the `npm` GitHub environment and its `NPM_TOKEN` secret with publish access to
`@maptimy/vrp-uniffi`, then dispatch **Native Node package** with `publish: true`.
The default is build/test only. Publishing uses the tested tarball and is gated
on every platform/runtime check succeeding. No npm credentials are needed for
local builds or build-only workflow runs.

## Migrating from 0.3.0 to 0.4.0

This is an intentional API change; regenerate and publish the native package
before updating the scheduler dependency to `^0.4.0`.

- Activity time is `{ start, end }`; stop time remains `{ arrival, departure }`.
  Update pickup timing and Excel export to use `activity.time?.start/end`, with
  the existing stop-time fallbacks.
- Construct input locations using `new Location.Coordinate({ lat, lng })` or
  `new Location.Reference({ index })`. Enum tags are now `Coordinate` and
  `Reference`. Update comparisons and fixtures accordingly. Frontend code should
  continue using type-only imports so it does not load the native runtime.
- `VrpError` variants are `FormatError`, `ValidationError` and `SolvingError`.
  Their native message is `error.inner.message`. Generated converters are now
  private: remove the scheduler's `FfiConverterTypeVrpError` monkey-patch, and
  handle the structured error before falling back to `Error.message`.
- `VrpSolver`, its methods, and `uniffiDestroy()` remain available. The standalone
  regression in `../tests/activity_time.cjs` demonstrates the new API.

After migrating, run the scheduler's TypeScript and planner tests, then verify
the selected-transports case and its Excel export with real application data.
