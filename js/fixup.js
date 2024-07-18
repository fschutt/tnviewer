

function base64ToArrayBuffer(base64) {
    var binaryString = atob(base64);
    var bytes = new Uint8Array(binaryString.length);
    for (var i = 0; i < binaryString.length; i++) {
        bytes[i] = binaryString.charCodeAt(i);
    }
    return bytes.buffer;
}

async function __wbg_init(input) {
    if (wasm !== undefined) return wasm;
    const imports = __wbg_get_imports();
    __wbg_init_memory(imports);
    var v = base64ToArrayBuffer(window.GLOBAL_WASM);
    const { instance, module } = await WebAssembly.instantiate(v, imports);
    return __wbg_finalize_init(instance, module);
}