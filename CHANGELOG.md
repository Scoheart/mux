# Changelog

## [1.8.13](https://github.com/Scoheart/mux/compare/v1.8.12...v1.8.13) (2026-07-21)

### Changes

* feat(desktop): refine asset management UX ([c63223d](https://github.com/Scoheart/mux/commit/c63223d87b2a9562cf3acbe08b573542d6739030))

## [1.8.12](https://github.com/Scoheart/mux/compare/v1.8.11...v1.8.12) (2026-07-21)

### Changes

* style(desktop): strengthen workspace region hierarchy ([3f5bb93](https://github.com/Scoheart/mux/commit/3f5bb9372236eb71c8a9a62005b9d78fcce13a8c))

## [1.8.11](https://github.com/Scoheart/mux/compare/v1.8.10...v1.8.11) (2026-07-21)

### Changes

* feat(ui): adopt soft workbench layout ([eddcec0](https://github.com/Scoheart/mux/commit/eddcec0f485c0b1bcd5e2185bd86b2e5caba9748))

## [1.8.10](https://github.com/Scoheart/mux/compare/v1.8.9...v1.8.10) (2026-07-21)

### Changes

* feat(models): add profile migration and agent config import ([ec656a2](https://github.com/Scoheart/mux/commit/ec656a2285f03915b440af4a41b7caec89629b9c))

## [1.8.9](https://github.com/Scoheart/mux/compare/v1.8.8...v1.8.9) (2026-07-20)

### Changes

* feat(models): manage multiple profiles per agent ([994a8a8](https://github.com/Scoheart/mux/commit/994a8a8e979b816eab9524f7eee4acd82e7e9c19))

## [1.8.8](https://github.com/Scoheart/mux/compare/v1.8.7...v1.8.8) (2026-07-20)

### Changes

* fix(release): tolerate Draft lookup latency ([3bb5eef](https://github.com/Scoheart/mux/commit/3bb5eef03ecab37b1624b9a5e9edeb3d182b7631))

## [1.8.7](https://github.com/Scoheart/mux/compare/v1.8.6...v1.8.7) (2026-07-20)

### Changes

* fix(release): verify Draft tag binding ([08b8614](https://github.com/Scoheart/mux/commit/08b8614b35e275fe4a878e72272bcfa6af31ff79))

## [1.8.6](https://github.com/Scoheart/mux/compare/v1.8.5...v1.8.6) (2026-07-20)

### Changes

* ci(release): turn main pushes into stable packages ([8966338](https://github.com/Scoheart/mux/commit/89663389e3267c066bbfa6f35703022c66dbf5fe))

## [1.8.5](https://github.com/Scoheart/mux/compare/v1.8.4...v1.8.5) (2026-07-20)


### Bug Fixes

* **models:** make Grok Build credentials atomic ([a0c2aa3](https://github.com/Scoheart/mux/commit/a0c2aa309321d6fe20b94def99388a3bbff3b470))

## [1.8.4](https://github.com/Scoheart/mux/compare/v1.8.3...v1.8.4) (2026-07-20)


### Bug Fixes

* **release:** bound fast lane restoration retries ([c115e72](https://github.com/Scoheart/mux/commit/c115e72a546f3d4e21e5a383e6c513e3f189d5b1))

## [1.8.3](https://github.com/Scoheart/mux/compare/v1.8.2...v1.8.3) (2026-07-20)


### Bug Fixes

* **release:** ignore generated prerelease tag events ([9f40300](https://github.com/Scoheart/mux/commit/9f40300522b28faafb3d890c5ac5c03078d190d1))

## [1.8.2](https://github.com/Scoheart/mux/compare/v1.8.1...v1.8.2) (2026-07-20)


### Bug Fixes

* **release:** authorize prerelease publishing ([ea1d085](https://github.com/Scoheart/mux/commit/ea1d0853cf82b9c864c1ac798da994607c037b2c))

## [1.8.1](https://github.com/Scoheart/mux/compare/v1.8.0...v1.8.1) (2026-07-20)


### Bug Fixes

* **release:** invoke verification script with bash ([3fc4b86](https://github.com/Scoheart/mux/commit/3fc4b864900bc4e75e4e9da04b7fa8eb87c21972))
* **release:** isolate ruleset restoration token ([0e6ea63](https://github.com/Scoheart/mux/commit/0e6ea63837d51849317cd8d5b98b53579693db05))

## [1.8.0](https://github.com/Scoheart/mux/compare/v1.7.0...v1.8.0) (2026-07-20)


### Features

* **migration:** adopt historical MCPs and skills ([#67](https://github.com/Scoheart/mux/issues/67)) ([eda902a](https://github.com/Scoheart/mux/commit/eda902a4816dd60a37a89c4199a4114fe7b1c198))

## [1.7.0](https://github.com/Scoheart/mux/compare/v1.6.3...v1.7.0) (2026-07-20)


### Features

* **agents:** simplify skills and manage Grok models ([#65](https://github.com/Scoheart/mux/issues/65)) ([3705e78](https://github.com/Scoheart/mux/commit/3705e78e7c11e7f286d26febcce18d8e3abde030))

## [1.6.3](https://github.com/Scoheart/mux/compare/v1.6.2...v1.6.3) (2026-07-19)


### Bug Fixes

* **release:** recognize squashed release commits ([#56](https://github.com/Scoheart/mux/issues/56)) ([56547c5](https://github.com/Scoheart/mux/commit/56547c53dd274ca8e8b9526ab0d2d50e47c03265))
* **release:** use verified setup-node action pin ([#58](https://github.com/Scoheart/mux/issues/58)) ([0d59ba5](https://github.com/Scoheart/mux/commit/0d59ba5fa1762cf70efb5621074aef1ed60a6727))

## [1.6.2](https://github.com/Scoheart/mux/compare/v1.6.1...v1.6.2) (2026-07-19)


### Bug Fixes

* **agent:** reconcile model state and agent identity ([#54](https://github.com/Scoheart/mux/issues/54)) ([b95792f](https://github.com/Scoheart/mux/commit/b95792f8109180b4d1519e0b94fd5e90df1d5635))

## [1.6.1](https://github.com/Scoheart/mux/compare/v1.6.0...v1.6.1) (2026-07-19)


### Bug Fixes

* **assets:** keep consumption controls in Agent pages ([#52](https://github.com/Scoheart/mux/issues/52)) ([42973f8](https://github.com/Scoheart/mux/commit/42973f8f885445e65524c304d7b1c5970cf99ef7))

## [1.6.0](https://github.com/Scoheart/mux/compare/v1.5.0...v1.6.0) (2026-07-19)


### Features

* **assets:** streamline Agent and Skill workflows ([#50](https://github.com/Scoheart/mux/issues/50)) ([a0f2566](https://github.com/Scoheart/mux/commit/a0f25662d86e9e17481cbf0e3f14ab135aafa868))

## [1.5.0](https://github.com/Scoheart/mux/compare/v1.4.0...v1.5.0) (2026-07-19)


### Features

* **agent:** simplify asset configuration ([713a2ab](https://github.com/Scoheart/mux/commit/713a2ab5ccf67f884b432175a0fb28297fe3695b))

## [1.4.0](https://github.com/Scoheart/mux/compare/v1.3.0...v1.4.0) (2026-07-18)


### Features

* **consumption:** centralize agent asset consumption ([d7723e8](https://github.com/Scoheart/mux/commit/d7723e89ea3a841fb79eef4b6232a07ea8066a09))


### Bug Fixes

* **core:** migrate TOML document parsing ([c81a6da](https://github.com/Scoheart/mux/commit/c81a6da636fb96e133f5d8bae2bf6d960f7ad089))

## [1.3.0](https://github.com/Scoheart/mux/compare/v1.2.20...v1.3.0) (2026-07-18)


### Features

* **ui:** unify MCP, Model, and Skill resource views ([#40](https://github.com/Scoheart/mux/issues/40)) ([8b38a76](https://github.com/Scoheart/mux/commit/8b38a76595f1a3bfb6c439c1c4b78880cd854f00))

## [1.2.20](https://github.com/Scoheart/mux/compare/v1.2.19...v1.2.20) (2026-07-17)


### Bug Fixes

* **release:** harden stable publication ([#38](https://github.com/Scoheart/mux/issues/38)) ([cb5df0e](https://github.com/Scoheart/mux/commit/cb5df0eb856a82d5be2883fbd3d79e1b669d36f2))

## [1.2.19](https://github.com/Scoheart/mux/compare/v1.2.18...v1.2.19) (2026-07-17)


### Bug Fixes

* **release:** automate MUX delivery ([#20](https://github.com/Scoheart/mux/issues/20)) ([6b18122](https://github.com/Scoheart/mux/commit/6b1812213a8b07452d68b12a4746b07dfe73d65f))
* **release:** preserve versioned Release PR title ([#34](https://github.com/Scoheart/mux/issues/34)) ([d12df81](https://github.com/Scoheart/mux/commit/d12df81a311a2ff69d9688533fda18b1ff31ef64))
* **release:** refresh Cargo package versions ([#35](https://github.com/Scoheart/mux/issues/35)) ([5fe02f1](https://github.com/Scoheart/mux/commit/5fe02f16acca6210b2c39d7a276b34fb247449d4))
* **release:** refresh local Cargo lock entries ([#36](https://github.com/Scoheart/mux/issues/36)) ([e571ceb](https://github.com/Scoheart/mux/commit/e571ceb313ba587c2a4bb2fe06bf017bfa4b60cc))
* **test:** serialize path environment checks ([#37](https://github.com/Scoheart/mux/issues/37)) ([ed6f94c](https://github.com/Scoheart/mux/commit/ed6f94c976e4ad796bdb3a7a737e226dddb9ae90))

## Changelog

All notable stable MUX changes are recorded here by Release Please.
