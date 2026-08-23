# Publishing releases

MiniAudio Node uses GitHub Actions to build and publish the native package. The maintained workflow is [`.github/workflows/release.yml`](../.github/workflows/release.yml).

## Release sequence

The workflow has three gates:

1. **Quality** — Rust formatting, Clippy, Rust tests, and TypeScript checks run on Ubuntu.
2. **Build** — the six published native targets are built in parallel.
3. **Publish** — artifacts are collected into a package directory, a GitHub release is created, and npm publication is attempted.

The build matrix is:

| Target | Runner | Native tests |
| --- | --- | --- |
| `x86_64-pc-windows-msvc` | Windows | Yes |
| `i686-pc-windows-msvc` | Windows | Build only |
| `x86_64-apple-darwin` | macOS Intel | Yes |
| `aarch64-apple-darwin` | macOS Apple Silicon | Yes |
| `x86_64-unknown-linux-gnu` | Ubuntu | Yes |
| `aarch64-unknown-linux-gnu` | Ubuntu cross-build | Build only |

The build-only entries are cross-compiled or otherwise cannot be executed by their runner. They still must compile and produce the expected artifact.

## Create a tag release

Keep the npm and Cargo versions aligned before creating a release. Then build and run the local checks:

```bash
bun install --frozen-lockfile
bun run build
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
bun test
bun run tsc --noEmit
```

Create and push a semantic-version tag:

```bash
git tag v1.6.3
git push origin v1.6.3
```

A tag matching `v<major>.<minor>.<patch>` starts the release workflow. Pre-release suffixes such as `-beta.1` are also accepted. Stable tags are published to npm with the `latest` dist-tag; pre-release tags are marked as GitHub prereleases and published with npm's `next` dist-tag.

## Run a manual release

The workflow supports `workflow_dispatch` with a required `version` input naming an existing tag:

```bash
gh workflow run release.yml -f version=v1.6.3
```

The input must be a valid `v`-prefixed semantic version and the tag must already exist. Every job checks out that tag, verifies that Cargo and npm metadata match it, and builds/publishes only that checked-out source. A manual run cannot relabel arbitrary `main` code as the requested version.

## Required repository configuration

The release job uses:

- `CANONICAL_RELEASE_TOKEN` to create the GitHub release in `nglmercer/miniaudio-node`. This must be a narrowly scoped fine-grained PAT or GitHub App token with contents write access to that repository; the workflow may execute in the `hernan-lc/miniaudio-node` Actions mirror, so its default `GITHUB_TOKEN` is not used for the canonical release;
- `NPM_TOKEN` to publish to npm when npm publication is enabled.

If `NPM_TOKEN` is absent, the workflow creates the GitHub release and emits a warning while skipping npm publication. Both secrets should be configured in the repository or organization settings before a release that must publish the GitHub release and npm package.

## Package contents

The publish job copies these files into a temporary package directory:

```text
package/
├── package.json
├── index.js
├── index.d.ts
├── README.md
├── LICENSE
└── *.node
```

The native files are flattened into the package root with the platform-specific names expected by the generated N-API loader. Documentation articles remain in the repository and are linked from the README; they are not required in the runtime package.

## Verify a package locally

Before pushing a release tag, inspect the npm package file list:

```bash
npm pack --dry-run
```

Confirm that the package contains the loader, declarations, license, README, and the native artifact for the current host. A local build uses the N-API CLI directly; no custom loader patch is required after `bun run build`.

## Release security

The workflows pin third-party GitHub Actions to commit SHAs. The default workflow permissions are read-only, and write permissions are scoped to the publishing job. Keep those properties when changing release automation.

Do not put npm tokens in source files, workflow arguments, or build logs. Rotate a token immediately if it is exposed.

## Troubleshooting

### A target is not found after the build

Check the target name and artifact name in the matrix. The expected artifacts are listed in [Platform support](PLATFORM_SUPPORT.md). Build output must be copied into `dist/` before upload.

### The package version is wrong

Check the tag or manual `version` input. It must be an existing `vX.Y.Z` tag (optionally with a pre-release suffix), and both Cargo and npm versions must match it.

### Native tests fail on a runner

Confirm that the test is running against the artifact created by that matrix entry and that the runner has the required audio runtime. Hardware-dependent tests may be skipped when no audio system is available; deterministic failures must still fail the job. For a self-hosted release runner with real devices, set `MINIAUDIO_REQUIRE_AUDIO_HARDWARE=1` so unavailable input/output paths fail rather than skip. See [Testing](TESTING.md).

## Related documentation

- [Platform support](PLATFORM_SUPPORT.md)
- [Development](DEVELOPMENT.md)
- [Testing](TESTING.md)
- [Changelog](CHANGELOG.md)
