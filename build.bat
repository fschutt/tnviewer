wasm-pack build --target web --no-pack --no-typescript --release && del pkg/.gitignore
python ./postbuild.py