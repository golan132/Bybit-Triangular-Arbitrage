# Changelog

## 0.1.0 (2025-12-25)


### Features

* Add dynamic shape configuration for bot instance and create AMD environment variables file ([2804e0c](https://github.com/golan132/Bybit-Triangular-Arbitrage/commit/2804e0c3978702b126d7a2ac0057f95757f1e8f3))
* Add OCI SDK variables for backend initialization in action.yaml ([b2b75d6](https://github.com/golan132/Bybit-Triangular-Arbitrage/commit/b2b75d65644c152981611c70aef4372886024b6f))
* Create ArbitrageTrader for executing arbitrage opportunities ([58c73de](https://github.com/golan132/Bybit-Triangular-Arbitrage/commit/58c73de769f6d275d554a3312797683d7262b1f5))
* Enhance arbitrage engine and trading logic ([461ef3e](https://github.com/golan132/Bybit-Triangular-Arbitrage/commit/461ef3e487ccc9ec437b46e97e447a6fe34e3ab5))
* Enhance Arbitrage Engine with global best opportunity tracking ([e6b99e0](https://github.com/golan132/Bybit-Triangular-Arbitrage/commit/e6b99e08b2c4f493f3bb85c05c2736adeb437da6))
* Enhance arbitrage execution with slippage calculations and timeout handling ([95bf8b1](https://github.com/golan132/Bybit-Triangular-Arbitrage/commit/95bf8b15ea6ecf3f987ea4fdc700e0117c45c16e))
* Enhance PrecisionManager and ArbitrageTrader with symbol mapping and conversion logic ([bf400e0](https://github.com/golan132/Bybit-Triangular-Arbitrage/commit/bf400e06a27e6354c32b63751c8ed54fa1dabe4a))
* Implement OCI setup and deployment workflows with Terraform integration ([96b46ce](https://github.com/golan132/Bybit-Triangular-Arbitrage/commit/96b46ce8387321909d89126d9750dee35de96202))
* Implement PrecisionManager for handling trading pair precision data ([58c73de](https://github.com/golan132/Bybit-Triangular-Arbitrage/commit/58c73de769f6d275d554a3312797683d7262b1f5))
* Integrate WebSocket for real-time ticker updates ([e6b99e0](https://github.com/golan132/Bybit-Triangular-Arbitrage/commit/e6b99e08b2c4f493f3bb85c05c2736adeb437da6))
* Update OCI provider version and refactor variable definitions; add backend configuration ([2ae6578](https://github.com/golan132/Bybit-Triangular-Arbitrage/commit/2ae6578286550e17e3ff5cafbcdd1d590901d07b))
* Update permissions and add config file for release workflow ([93e38b6](https://github.com/golan132/Bybit-Triangular-Arbitrage/commit/93e38b69d987142023ec87a14e1de10d6b965970))
* Update release workflow for Rust support and add artifact upload step ([a7f5e8d](https://github.com/golan132/Bybit-Triangular-Arbitrage/commit/a7f5e8d89045fd0c9714ee00fbdcd560a30bcd16))


### Bug Fixes

* Add newline at the end of main.tf for proper formatting ([89488ed](https://github.com/golan132/Bybit-Triangular-Arbitrage/commit/89488ed14a87aa3918125fd6bed58f60029edb76))
* Improve error handling and logging in trader module ([e6b99e0](https://github.com/golan132/Bybit-Triangular-Arbitrage/commit/e6b99e08b2c4f493f3bb85c05c2736adeb437da6))
* Restore workflow triggers in deploy-project.yml and ensure proper formatting in variables.tf ([e9a2f02](https://github.com/golan132/Bybit-Triangular-Arbitrage/commit/e9a2f0284cb055d60a49da083b73784e2cdc4fc4))
* Update handling of OCI private key in action.yaml to manage multiline secrets correctly ([4749d07](https://github.com/golan132/Bybit-Triangular-Arbitrage/commit/4749d0795bc769d36bf6fe3a33b54b265854b99d))
* Update OCI key file path and permissions in action.yaml; comment out workflow triggers in deploy-project.yml ([9ee0ea0](https://github.com/golan132/Bybit-Triangular-Arbitrage/commit/9ee0ea05971658f6f51f806804c54ea65dfcf294))
* Update private key variable references in action.yaml and deploy-project.yml for consistency ([a91af60](https://github.com/golan132/Bybit-Triangular-Arbitrage/commit/a91af60d81b7e6a87472a1bd400665241a3750a5))
