wasm-pack build --target web --no-pack --no-typescript && del pkg/.gitignore
python ./postbuild.py