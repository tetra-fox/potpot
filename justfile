default:
    @just --list

# open the example panel that is drawn with flat shapes alone
flat:
    cargo run -p potpot-window --example flat

# draw one of potpot's example panels to a png: `just picture out.png synth`
picture *args:
    cargo run -p potpot --example picture -- {{ args }}

# open a window with every kind of part on it, lamps following the controls and meters a feed
demo:
    cargo run -p potpot --example demo

# open a four-band equalizer whose dots, knobs and faders turn the same parameters
equalizer:
    cargo run -p potpot --example equalizer

# open a compressor with its curve on a screen and meters from a feed
compressor:
    cargo run -p potpot --example compressor

# open a synthesizer's front with a wave, a filter and an envelope on screens
synth:
    cargo run -p potpot --example synth

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
