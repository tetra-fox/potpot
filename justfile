default:
    @just --list

# open an example window: demo, equalizer, compressor, synth or flat
example name:
    cargo run --example {{ name }}

# draw an example to a png at a scale: `just render demo out.png 1.5`.
# `gallery` takes the name of a panel of parts first, then a whole-number scale:
# `just render gallery out.png knobs 2`
render name file *args:
    cargo run --example {{ name }} -- {{ file }} {{ args }}

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
