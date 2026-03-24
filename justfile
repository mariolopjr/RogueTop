bootstrap *args:
    {{ if os() == "windows" { "cmd /c bootstrap_pokerogue.cmd" } else { "bash bootstrap_pokerogue.sh" } }} {{args}}

clean:
    cargo clean --manifest-path src-tauri/Cargo.toml
    rm -rf dist ext/index_min.js game.dat src-ext

build-ext:
    pnpm build:ext

build: build-ext
    pnpm tauri build

build-offline: bootstrap build-ext
    pnpm tauri build -- --features offline

dev:
    pnpm tauri dev

dev-offline: bootstrap build-ext
    pnpm tauri dev -- --features offline
