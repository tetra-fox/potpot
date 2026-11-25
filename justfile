default:
    @just --list

# open the example panel that is drawn with flat shapes alone
flat:
    cargo run -p potpot-window --example flat

# draw one of potpot's example panels to a png: `just picture out.png synth`
picture *args:
    cargo run -p potpot --example picture -- {{ args }}

# open a window with every kind of part on it, lamps and meters following the controls
demo:
    cargo run -p potpot --example demo

test:
    cargo test --workspace

lint:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings

# check the workspace as it builds for Windows
check-windows:
    cargo check --workspace --target x86_64-pc-windows-gnu

# the gate for every push
ci: lint test check-windows
