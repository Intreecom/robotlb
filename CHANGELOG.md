# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0](https://github.com/forkline/robotlb/tree/0.5.0) - 2026-03-04

### Added

- helm: Add Prometheus metrics support ([2e6eb91](https://github.com/forkline/robotlb/commit/2e6eb916b09f9e65e1da67891a42bea3d7b6913f))
- Add Prometheus metrics endpoint ([c69e7ab](https://github.com/forkline/robotlb/commit/c69e7abc4c9b1c44000b0d5a8ef63108ff1afe6c))
- Implement OpenTelemetry distributed tracing integration ([dbcfa8e](https://github.com/forkline/robotlb/commit/dbcfa8eb35bd62da0fadc2332c54ccc17dba3c9a))

### Fixed

- ci: Resolve pre-commit hook failures ([94f6254](https://github.com/forkline/robotlb/commit/94f62541370363fd4b9901eb0dae4991550f690a))
- ci: Correct action field order in auto-tag workflow ([7a4f3d8](https://github.com/forkline/robotlb/commit/7a4f3d8b59a63573ef00d3d382fa85b79f656b6f))
- ci: Downgrade invalid action versions ([77ec84f](https://github.com/forkline/robotlb/commit/77ec84faab22af4a883e3be9b685db010e6640b6))
- Address clippy warnings in metrics module ([1bbe64a](https://github.com/forkline/robotlb/commit/1bbe64a4cc17c6dcc6d3de194e83f80f9964b7bd))
- Resolve clippy warnings in metrics code ([a6f3aef](https://github.com/forkline/robotlb/commit/a6f3aef1f1b38ad6c8916202496dfa34d9d068b8))
- Remove trailing whitespace in docs ([f824f6b](https://github.com/forkline/robotlb/commit/f824f6ba692100370fc53df8d337b590f42edc2d))
- Remove trailing whitespace in docs ([d80b96e](https://github.com/forkline/robotlb/commit/d80b96eaedcc4063c85c79ea85a8c0fe61126160))
- Resolve clippy warnings and formatting issues ([32012da](https://github.com/forkline/robotlb/commit/32012da6daf055f05185b4c9ddca1e781349d7f5))

### Documentation

- Add OpenTelemetry tracing integration trade-off analysis ([b02a625](https://github.com/forkline/robotlb/commit/b02a62511a72912e4ae6292630c55a2b7e3c14e9))

### Refactor

- metrics: Use prometheus crate for metrics ([ae5e42b](https://github.com/forkline/robotlb/commit/ae5e42b49f541667be30c1595957073cb63f7041))
- Modularize codebase and add health checks ([b61c35a](https://github.com/forkline/robotlb/commit/b61c35a38178df79d7c1aefb2f7beea90565a819))
- Use OpenTelemetry for metrics instead of prometheus crate ([9bf163b](https://github.com/forkline/robotlb/commit/9bf163b99f157a8a2ac9672e515235258bbbc08a))

### Helm

- Add OpenTelemetry tracing configuration support ([4876530](https://github.com/forkline/robotlb/commit/48765307cc7c6a54eb196c4dff89e189f287de48))

## [0.4.3](https://github.com/forkline/robotlb/tree/0.4.3) - 2026-02-27

### Added

- Add graceful shutdown handling ([a0b8bed](https://github.com/forkline/robotlb/commit/a0b8bedb9e17eb3f8648b4ded091a36c38605a1d))

### Fixed

- Use correct ipmode field name in service status json ([c0df415](https://github.com/forkline/robotlb/commit/c0df4157007489ce326bbc63d4ed849d198ff28b))

### Release

- Version 0.4.2 ([8fd114f](https://github.com/forkline/robotlb/commit/8fd114fda7e1e22a18259fa4e96f12cdf607b301))

## [0.4.2](https://github.com/forkline/robotlb/tree/0.4.2) - 2026-02-27

### Fixed

- Use correct ipmode field name in service status json ([c0df415](https://github.com/forkline/robotlb/commit/c0df4157007489ce326bbc63d4ed849d198ff28b))

## [0.4.1](https://github.com/forkline/robotlb/tree/0.4.1) - 2026-02-27

### Added

- Set ipMode to Proxy when proxy mode is enabled ([be9cbc3](https://github.com/forkline/robotlb/commit/be9cbc3111a270b7dc20f7960dda5b4846177692))

### Documentation

- Add ha deployment and leader election guidance ([a204747](https://github.com/forkline/robotlb/commit/a204747fe8abeb711c57745656a7327e48b270f4))

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
