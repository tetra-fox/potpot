default:
    @just --list

# open an example window: demo, equalizer, compressor, synth or flat
example name:
    cargo run --example {{ name }}

# draw one of potpot's example panels to a png: `just render synth out.png`
render name file *args:
    cargo run --example picture -- {{ file }} {{ name }} {{ args }}

test:
    cargo test --workspace

# run the tests that draw with a graphics device, which `test` and `ci` leave out
gpu-test:
    cargo test --workspace -- --ignored

lint:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings

# check the workspace as it builds for Windows
check-windows:
    cargo check --workspace --target x86_64-pc-windows-gnu

# the gate for every push
ci: lint test check-windows
