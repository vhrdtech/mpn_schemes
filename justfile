# Bare `just` only lists the recipes
default:
    @just --list

# The tests. `just test`: all of them (test-all), the same run the landing gate does. `just test --changed`: only
# the workspace crates changed against main and those that depend on them, before marking work ready.
test *flags:
    if [ "{{flags}}" = "--changed" ]; then if command -v tpm >/dev/null 2>&1; then tpm test changed -- cargo nextest run --workspace; else cargo nextest run --workspace; fi; else {{just_executable()}} test-all {{flags}}; fi

# Every test: nextest for unit and integration tests, cargo test for doctests
test-all:
    cargo nextest run --workspace --no-tests=pass
    cargo test --workspace --doc

# Clippy with warnings as errors, and the format check
lint:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --all --check

# Format all code
fmt:
    cargo fmt --all

# Build mpns in the deploy profile and install it on this PC (kept-releases stack when tpm is on PATH)
install:
    cargo build --profile deploy --locked -p mpn_schemes_cli
    if command -v tpm >/dev/null; then tpm install target/deploy/mpns; else cargo install --locked --force --path mpn_schemes_cli; fi

# Bring this PC up to main
deploy: install
