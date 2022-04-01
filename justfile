serve package: (build package)
    miniserve --index index.html ./target/{{package}}/

build package:
    mkdir -p ./target/{{package}}/
    cp ./index.html ./target/{{package}}/
    RUSTFLAGS=--cfg=web_sys_unstable_apis cargo build --release --package {{package}} --target wasm32-unknown-unknown --features web
    wasm-bindgen --target web --no-typescript --out-dir ./target/{{package}}/ ./target/wasm32-unknown-unknown/release/{{package}}.wasm

clean package:
    rm -rf ./target/{{package}}/
