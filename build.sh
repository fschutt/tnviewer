wasm-pack build --target web --no-pack --no-typescript --release && rm pkg/.gitignore
python3 ./postbuild.py