wasm-pack build --target web --no-pack --no-typescript --debug && rm pkg/.gitignore
python3 ./postbuild.py