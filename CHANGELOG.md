# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0](https://github.com/forkline/robotlb/tree/0.4.0) - 2026-02-25

### Added

- ha: Enable leader election for safe multi-replica runs ([9c71b96](https://github.com/forkline/robotlb/commit/9c71b9635ce3f24ae7a718f3c67a5f1eabf01559))

## [0.3.2](https://github.com/forkline/robotlb/tree/0.3.2) - 2026-02-25

### Fixed

- changelog: Align tag pattern and regenerate changelog ([9ffe8d9](https://github.com/forkline/robotlb/commit/9ffe8d99f53955be11eea68129ff33b45f458f75))

### Chore

- logging: Quiet reconcile debug noise for production ([2255956](https://github.com/forkline/robotlb/commit/22559568b937fa9b66d13d59e1bcddb719a48605))

## [0.3.1](https://github.com/forkline/robotlb/tree/0.3.1) - 2026-02-25

### Fixed

- Trigger reconcile on endpoints updates with resilient fallback ([743159d](https://github.com/forkline/robotlb/commit/743159db222066a7818eea773884666ee186eca9))

## [0.3.0](https://github.com/forkline/robotlb/tree/0.3.0) - 2026-02-25

### Fixed

- deps: Align transitive deps for kube v3 ([3afdc09](https://github.com/forkline/robotlb/commit/3afdc092b8e3da4daca3ecacff8325bfe18b6e9d))
- Satisfy clippy item order in lb tests ([fcfce3d](https://github.com/forkline/robotlb/commit/fcfce3db5703f467087b2cd577cbab504e5faf3b))
- Satisfy clippy send and default-init lints ([d036703](https://github.com/forkline/robotlb/commit/d036703a6162b770e253f6392e4fff05b452da67))
- Resolve clippy lints and format code ([c00007a](https://github.com/forkline/robotlb/commit/c00007a4385082e95e0c99fe324ade843734c0c1))

### Build

- deps: Update Rust crate kube to v3 ([1baef50](https://github.com/forkline/robotlb/commit/1baef506398d601b2b6044f54e9259a891fe0478))
- deps: Update hcloud to v0.25.0 ([004c2f0](https://github.com/forkline/robotlb/commit/004c2f0aebe55677e3f66c4bbeae802a7b0f5c8b))

### Refactor

- Split load balancer reconciliation planning from execution ([47adbbf](https://github.com/forkline/robotlb/commit/47adbbf7af5b86b16689f7551efed085eaf041e8))
- Make finalizer reconciliation idempotent and retry-safe ([4d9ce2b](https://github.com/forkline/robotlb/commit/4d9ce2b850d4a373cadd6fbdacac5832f583aefd))
- Decompose reconcile flow into explicit steps ([7770f48](https://github.com/forkline/robotlb/commit/7770f48a5d23bd827dbba5a518d1f5aca086c59a))

### Styling

- Format lb module with cargo fmt ([5f09e90](https://github.com/forkline/robotlb/commit/5f09e90df7dc8f5207edd95ba09eeed586d4ee04))

### Testing

- Add unit coverage for config and label filters ([9f29b1d](https://github.com/forkline/robotlb/commit/9f29b1d99c1b03242cc9c131d264c2f87172fc48))
- Cover load balancer annotation parsing ([ec9698a](https://github.com/forkline/robotlb/commit/ec9698a8010a5649efa913178259b722670aaec4))
- Add mocked reconcile coverage for lb sync ([b20f389](https://github.com/forkline/robotlb/commit/b20f389435e1e9739d340018f574ba232ed0eea2))
- Cover controller reconcile filtering and derivation ([cafbd0b](https://github.com/forkline/robotlb/commit/cafbd0bc43001595b9c0eddc1c17335cb8e0af77))

## [0.2.1](https://github.com/forkline/robotlb/tree/0.2.1) - 2026-02-25

### Fixed

- ci: Add helm package to create chart ([cda33cf](https://github.com/forkline/robotlb/commit/cda33cf42c1fcca711691ce663184227e4faeeae))

### Build

- deps: Update actions/setup-python action to v6 ([c5b8434](https://github.com/forkline/robotlb/commit/c5b8434d760fa400f8d9efaf69dbf23b06eef053))

## [0.2.0](https://github.com/forkline/robotlb/tree/0.2.0) - 2026-02-25

### Release

- Sync helm chart version with app version ([7000ee5](https://github.com/forkline/robotlb/commit/7000ee56ed7d25159be7192e46a4889e2ee7e964))

## [0.1.1](https://github.com/forkline/robotlb/tree/0.1.1) - 2026-02-25

### Fixed

- chart: Image repository renamed to forkline ([7a32c84](https://github.com/forkline/robotlb/commit/7a32c842ebefc2a81cc7edf5711b74e2e67236d5))
- Resolve all pre-commit CI errors ([8602655](https://github.com/forkline/robotlb/commit/86026559866a0fbbfe24db4ba8d619afa399cb52))

### Build

- deps: Update pre-commit/action action to v3.0.1 ([504dc49](https://github.com/forkline/robotlb/commit/504dc4906b1acfb44773b41bb1c5bcd9690e3b50))
- deps: Update Rust crate clap to v4.5.60 ([42ed67d](https://github.com/forkline/robotlb/commit/42ed67d28efc8f34c8bb39f23b9fafb572e4d642))
- deps: Update Rust crate thiserror to v2.0.18 ([1ce38f1](https://github.com/forkline/robotlb/commit/1ce38f1eee5892c0ea7d871f905c326fe97dbb52))
- deps: Update azure/setup-helm action to v4.3.1 ([12e3d44](https://github.com/forkline/robotlb/commit/12e3d448af9956ad688d4b7629eb9e8f90eac4ea))
- deps: Update Rust crate tokio to v1.49.0 ([b681931](https://github.com/forkline/robotlb/commit/b68193138a667383eef57c30a8fbf5f9428a0986))
- deps: Update docker/login-action action to v3 ([5e3eefa](https://github.com/forkline/robotlb/commit/5e3eefaf722615ff370c3e48cc0627a5de8bbc73))
- deps: Update docker/build-push-action action to v6 ([e1d707e](https://github.com/forkline/robotlb/commit/e1d707e776706e259faeb1aa90aee71b4bb3fe50))
- deps: Update actions/checkout action to v6 ([611c297](https://github.com/forkline/robotlb/commit/611c297e85fb37144844b80b4b6f9b2502bd6f12))
- deps: Update Rust crate tikv-jemallocator to v0.6.1 ([25a057b](https://github.com/forkline/robotlb/commit/25a057b27cfd6934c84c633271bf3a5ea15138d8))
- deps: Update tokio-tracing monorepo ([c9255c5](https://github.com/forkline/robotlb/commit/c9255c5d5c5ac10b2ad08eaf0cd9b4784db55305))
- deps: Update Rust crate futures to v0.3.32 ([f4ab84f](https://github.com/forkline/robotlb/commit/f4ab84fe9baefadd6321b6af45743e15cd5cbf5e))
- renovate: Enable fork processing and onboarding dashboard ([3a57b44](https://github.com/forkline/robotlb/commit/3a57b4472da87fd19d1a7d903aaedd15ab32324e))
- renovate: Remove invalid onboarding repo option ([98696f9](https://github.com/forkline/robotlb/commit/98696f9199ea1b52eb769d311842af1bbc78ebac))

### Release

- Version 0.1.1 ([61246e0](https://github.com/forkline/robotlb/commit/61246e0dea9a12c8871ad388f3a2631e180b01a2))

## [0.1.0](https://github.com/forkline/robotlb/tree/0.1.0) - 2026-02-25

### Build

- Add static checks and devops machinery ([d20ba06](https://github.com/forkline/robotlb/commit/d20ba06b748d9ec636e3e2ef054e49a7d4ccd985))

### Release

- Version 0.1.0 ([6a70265](https://github.com/forkline/robotlb/commit/6a70265af41ac9ac9ce338d0fc8225285dd05b3f))

## [0.0.1](https://github.com/forkline/robotlb/tree/0.0.1) - 2024-11-18
