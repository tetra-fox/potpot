default:
    @just --list

# open the example panel that is drawn with flat shapes alone
flat:
    cargo run -p controls-window --example flat

# draw one of skeuo's example panels to a png: `just picture out.png synth`
picture *args:
    cargo run -p skeuo --example picture -- {{ args }}

test:
    cargo test --workspace

lint:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings

# the window's code as Windows hosts build it
check-windows:
    cargo check --workspace --target x86_64-pc-windows-gnu

# the gate for every push
ci: lint test check-windows
