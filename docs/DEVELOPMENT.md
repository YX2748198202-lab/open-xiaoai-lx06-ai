# Development

## Local checks

```sh
cargo fmt --all -- --check
cargo check --bin standalone-ai
sh -n install-standalone-ai.sh
```

The GitHub workflow additionally builds an ARMv7 hard-float release binary with `cross`, verifies the ELF and packages an artifact.

## Source layout

- `src/bin/standalone-ai.rs`: standalone application, routing, API and TTS flow
- `src/services/monitor/`: LX06 input/log monitoring
- `src/services/speaker.rs`: native XiaoAI/TTS integration
- `src/services/`: shared Open-XiaoAI protocol and utility code
- `install-standalone-ai.sh`: device installation and `/data/init.sh` registration
- `ai.conf.example`: public configuration template without secrets

## Testing on hardware

Record the exact device model, firmware, configuration type and sanitized log lines. Do not use real API keys in tests or commit test configurations.
