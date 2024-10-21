let wasm;

let cachedUint8ArrayMemory0 = null;

function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

const heap = new Array(128).fill(undefined);

heap.push(undefined, null, true, false);

let heap_next = heap.length;

function addHeapObject(obj) {
    if (heap_next === heap.length) heap.push(heap.length + 1);
    const idx = heap_next;
    heap_next = heap[idx];

    heap[idx] = obj;
    return idx;
}

function getObject(idx) { return heap[idx]; }

function dropObject(idx) {
    if (idx < 132) return;
    heap[idx] = heap_next;
    heap_next = idx;
}

function takeObject(idx) {
    const ret = getObject(idx);
    dropObject(idx);
    return ret;
}

const cachedTextDecoder = (typeof TextDecoder !== 'undefined' ? new TextDecoder('utf-8', { ignoreBOM: true, fatal: true }) : { decode: () => { throw Error('TextDecoder not available') } } );

if (typeof TextDecoder !== 'undefined') { cachedTextDecoder.decode(); };

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

let WASM_VECTOR_LEN = 0;

const cachedTextEncoder = (typeof TextEncoder !== 'undefined' ? new TextEncoder('utf-8') : { encode: () => { throw Error('TextEncoder not available') } } );

const encodeString = (typeof cachedTextEncoder.encodeInto === 'function'
    ? function (arg, view) {
    return cachedTextEncoder.encodeInto(arg, view);
}
    : function (arg, view) {
    const buf = cachedTextEncoder.encode(arg);
    view.set(buf);
    return {
        read: arg.length,
        written: buf.length
    };
});

function passStringToWasm0(arg, malloc, realloc) {

    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }

    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = encodeString(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

let cachedDataViewMemory0 = null;

function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

function debugString(val) {
    // primitive types
    const type = typeof val;
    if (type == 'number' || type == 'boolean' || val == null) {
        return  `${val}`;
    }
    if (type == 'string') {
        return `"${val}"`;
    }
    if (type == 'symbol') {
        const description = val.description;
        if (description == null) {
            return 'Symbol';
        } else {
            return `Symbol(${description})`;
        }
    }
    if (type == 'function') {
        const name = val.name;
        if (typeof name == 'string' && name.length > 0) {
            return `Function(${name})`;
        } else {
            return 'Function';
        }
    }
    // objects
    if (Array.isArray(val)) {
        const length = val.length;
        let debug = '[';
        if (length > 0) {
            debug += debugString(val[0]);
        }
        for(let i = 1; i < length; i++) {
            debug += ', ' + debugString(val[i]);
        }
        debug += ']';
        return debug;
    }
    // Test for built-in
    const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
    let className;
    if (builtInMatches.length > 1) {
        className = builtInMatches[1];
    } else {
        // Failed to match the standard '[object ClassName]'
        return toString.call(val);
    }
    if (className == 'Object') {
        // we're a user defined class or Object
        // JSON.stringify avoids problems with cycles, and is generally much
        // easier than looping through ownProperties of `val`.
        try {
            return 'Object(' + JSON.stringify(val) + ')';
        } catch (_) {
            return 'Object';
        }
    }
    // errors
    if (val instanceof Error) {
        return `${val.name}: ${val.message}\n${val.stack}`;
    }
    // TODO we could test for more things here, like `Set`s and `Map`s.
    return className;
}

const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(state => {
    wasm.__wbindgen_export_3.get(state.dtor)(state.a, state.b)
});

function makeMutClosure(arg0, arg1, dtor, f) {
    const state = { a: arg0, b: arg1, cnt: 1, dtor };
    const real = (...args) => {
        // First up with a closure we increment the internal reference
        // count. This ensures that the Rust closure environment won't
        // be deallocated while we're invoking it.
        state.cnt++;
        const a = state.a;
        state.a = 0;
        try {
            return f(a, state.b, ...args);
        } finally {
            if (--state.cnt === 0) {
                wasm.__wbindgen_export_3.get(state.dtor)(a, state.b);
                CLOSURE_DTORS.unregister(state);
            } else {
                state.a = a;
            }
        }
    };
    real.original = state;
    CLOSURE_DTORS.register(real, state, state);
    return real;
}
function __wbg_adapter_32(arg0, arg1, arg2) {
    wasm.__wbindgen_export_4(arg0, arg1, addHeapObject(arg2));
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        wasm.__wbindgen_export_5(addHeapObject(e));
    }
}
function __wbg_adapter_105(arg0, arg1, arg2, arg3) {
    wasm.__wbindgen_export_6(arg0, arg1, addHeapObject(arg2), addHeapObject(arg3));
}

/**
*/
export function main() {
    wasm.main();
}

function _assertClass(instance, klass) {
    if (!(instance instanceof klass)) {
        throw new Error(`expected instance of ${klass.name}`);
    }
    return instance.ptr;
}
/**
* @param {Projection} src
* @param {Projection} dst
* @param {Point} point
*/
export function transform(src, dst, point) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        _assertClass(src, Projection);
        _assertClass(dst, Projection);
        _assertClass(point, Point);
        wasm.transform(retptr, src.__wbg_ptr, dst.__wbg_ptr, point.__wbg_ptr);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        if (r1) {
            throw takeObject(r0);
        }
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
    }
}

let stack_pointer = 128;

function addBorrowedObject(obj) {
    if (stack_pointer == 1) throw new Error('out of js stack');
    heap[--stack_pointer] = obj;
    return stack_pointer;
}
/**
* Read a binary NTv2 from Dataview.
*
* Note: only NTv2 file format are supported.
* @param {string} key
* @param {DataView} view
*/
export function add_nadgrid(key, view) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(key, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        wasm.add_nadgrid(retptr, ptr0, len0, addBorrowedObject(view));
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        if (r1) {
            throw takeObject(r0);
        }
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        heap[stack_pointer++] = undefined;
    }
}

function notDefined(what) { return () => { throw new Error(`${what} is not defined`); }; }
/**
* @returns {string}
*/
export function get_new_poly_id() {
    let deferred1_0;
    let deferred1_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        wasm.get_new_poly_id(retptr);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred1_0 = r0;
        deferred1_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred1_0, deferred1_1, 1);
    }
}

/**
* @param {string} savefile
* @returns {string}
*/
export function lib_parse_savefile(savefile) {
    let deferred2_0;
    let deferred2_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(savefile, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        wasm.lib_parse_savefile(retptr, ptr0, len0);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred2_0 = r0;
        deferred2_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred2_0, deferred2_1, 1);
    }
}

/**
* @param {string} info
* @param {string | undefined} [risse]
* @param {string | undefined} [csv]
* @param {string | undefined} [aenderungen]
* @param {string | undefined} [target_crs]
* @returns {string}
*/
export function format_savefile(info, risse, csv, aenderungen, target_crs) {
    let deferred6_0;
    let deferred6_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(info, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(risse) ? 0 : passStringToWasm0(risse, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        var len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(csv) ? 0 : passStringToWasm0(csv, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        var len2 = WASM_VECTOR_LEN;
        var ptr3 = isLikeNone(aenderungen) ? 0 : passStringToWasm0(aenderungen, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        var len3 = WASM_VECTOR_LEN;
        var ptr4 = isLikeNone(target_crs) ? 0 : passStringToWasm0(target_crs, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        var len4 = WASM_VECTOR_LEN;
        wasm.format_savefile(retptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred6_0 = r0;
        deferred6_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred6_0, deferred6_1, 1);
    }
}

/**
* @param {string} poly
* @param {string} target_crs
* @returns {string}
*/
export function get_rissgebiet_geojson(poly, target_crs) {
    let deferred3_0;
    let deferred3_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(poly, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(target_crs, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        wasm.get_rissgebiet_geojson(retptr, ptr0, len0, ptr1, len1);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred3_0 = r0;
        deferred3_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred3_0, deferred3_1, 1);
    }
}

/**
* @returns {string}
*/
export function get_problem_geojson() {
    let deferred1_0;
    let deferred1_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        wasm.get_problem_geojson(retptr);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred1_0 = r0;
        deferred1_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred1_0, deferred1_1, 1);
    }
}

/**
* @param {string} id
* @param {string | undefined} [nas_projected]
* @returns {string | undefined}
*/
export function search_flst_internal(id, nas_projected) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(nas_projected) ? 0 : passStringToWasm0(nas_projected, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        var len1 = WASM_VECTOR_LEN;
        wasm.search_flst_internal(retptr, ptr0, len0, ptr1, len1);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        let v3;
        if (r0 !== 0) {
            v3 = getStringFromWasm0(r0, r1).slice();
            wasm.__wbindgen_export_0(r0, r1 * 1, 1);
        }
        return v3;
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
    }
}

/**
* @param {string} id
* @param {string | undefined} [split_nas_projected]
* @returns {string | undefined}
*/
export function search_flst_part_internal(id, split_nas_projected) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(split_nas_projected) ? 0 : passStringToWasm0(split_nas_projected, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        var len1 = WASM_VECTOR_LEN;
        wasm.search_flst_part_internal(retptr, ptr0, len0, ptr1, len1);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        let v3;
        if (r0 !== 0) {
            v3 = getStringFromWasm0(r0, r1).slice();
            wasm.__wbindgen_export_0(r0, r1 * 1, 1);
        }
        return v3;
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
    }
}

/**
* @param {string} id
* @returns {string}
*/
export function validate_format_flst_id(id) {
    let deferred2_0;
    let deferred2_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        wasm.validate_format_flst_id(retptr, ptr0, len0);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred2_0 = r0;
        deferred2_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred2_0, deferred2_1, 1);
    }
}

/**
* @param {string | undefined} konfiguration
* @param {string | undefined} nas_original
* @param {string | undefined} split_nas_xml
* @param {string | undefined} aenderungen
* @param {string | undefined} csv
* @param {boolean} use_dgm
* @param {boolean} use_background
* @returns {Promise<Uint8Array>}
*/
export function export_pdf_overview(konfiguration, nas_original, split_nas_xml, aenderungen, csv, use_dgm, use_background) {
    var ptr0 = isLikeNone(konfiguration) ? 0 : passStringToWasm0(konfiguration, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
    var len0 = WASM_VECTOR_LEN;
    var ptr1 = isLikeNone(nas_original) ? 0 : passStringToWasm0(nas_original, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
    var len1 = WASM_VECTOR_LEN;
    var ptr2 = isLikeNone(split_nas_xml) ? 0 : passStringToWasm0(split_nas_xml, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
    var len2 = WASM_VECTOR_LEN;
    var ptr3 = isLikeNone(aenderungen) ? 0 : passStringToWasm0(aenderungen, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
    var len3 = WASM_VECTOR_LEN;
    var ptr4 = isLikeNone(csv) ? 0 : passStringToWasm0(csv, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
    var len4 = WASM_VECTOR_LEN;
    const ret = wasm.export_pdf_overview(ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, use_dgm, use_background);
    return takeObject(ret);
}

/**
* @param {string} rc
* @param {string | undefined} [utm_crs]
* @returns {string}
*/
export function get_header_coords(rc, utm_crs) {
    let deferred3_0;
    let deferred3_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(rc, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(utm_crs) ? 0 : passStringToWasm0(utm_crs, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        var len1 = WASM_VECTOR_LEN;
        wasm.get_header_coords(retptr, ptr0, len0, ptr1, len1);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred3_0 = r0;
        deferred3_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred3_0, deferred3_1, 1);
    }
}

/**
* @param {string | undefined} id
* @param {string} aenderungen
* @returns {string}
*/
export function lock_unlock_poly(id, aenderungen) {
    let deferred3_0;
    let deferred3_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        var ptr0 = isLikeNone(id) ? 0 : passStringToWasm0(id, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        var len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(aenderungen, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        wasm.lock_unlock_poly(retptr, ptr0, len0, ptr1, len1);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred3_0 = r0;
        deferred3_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred3_0, deferred3_1, 1);
    }
}

/**
* @param {string | undefined} id
* @param {string} aenderungen
* @param {string} split_nas_xml
* @param {string} nas_original
* @param {string} konfiguration
* @returns {string}
*/
export function lib_nutzungen_saeubern(id, aenderungen, split_nas_xml, nas_original, konfiguration) {
    let deferred6_0;
    let deferred6_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        var ptr0 = isLikeNone(id) ? 0 : passStringToWasm0(id, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        var len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(aenderungen, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(split_nas_xml, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passStringToWasm0(nas_original, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len3 = WASM_VECTOR_LEN;
        const ptr4 = passStringToWasm0(konfiguration, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len4 = WASM_VECTOR_LEN;
        wasm.lib_nutzungen_saeubern(retptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred6_0 = r0;
        deferred6_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred6_0, deferred6_1, 1);
    }
}

/**
* @param {string | undefined} [id]
* @param {string | undefined} [aenderungen]
* @param {string | undefined} [split_nas_xml]
* @param {string | undefined} [nas_original]
* @param {string | undefined} [konfiguration]
* @param {string | undefined} [csv]
* @returns {string}
*/
export function lib_get_aenderungen_clean(id, aenderungen, split_nas_xml, nas_original, konfiguration, csv) {
    let deferred7_0;
    let deferred7_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        var ptr0 = isLikeNone(id) ? 0 : passStringToWasm0(id, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        var len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(aenderungen) ? 0 : passStringToWasm0(aenderungen, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        var len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(split_nas_xml) ? 0 : passStringToWasm0(split_nas_xml, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        var len2 = WASM_VECTOR_LEN;
        var ptr3 = isLikeNone(nas_original) ? 0 : passStringToWasm0(nas_original, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        var len3 = WASM_VECTOR_LEN;
        var ptr4 = isLikeNone(konfiguration) ? 0 : passStringToWasm0(konfiguration, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        var len4 = WASM_VECTOR_LEN;
        var ptr5 = isLikeNone(csv) ? 0 : passStringToWasm0(csv, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        var len5 = WASM_VECTOR_LEN;
        wasm.lib_get_aenderungen_clean(retptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred7_0 = r0;
        deferred7_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred7_0, deferred7_1, 1);
    }
}

/**
* @param {string} split_nas_xml
* @param {string} nas_xml
* @param {string} projekt_info
* @param {string} konfiguration
* @param {string} aenderungen
* @param {string} risse
* @param {string} csv_data
* @param {boolean} render_hintergrund_vorschau
* @param {boolean} use_dgm
* @returns {Promise<Uint8Array>}
*/
export function aenderungen_zu_geograf(split_nas_xml, nas_xml, projekt_info, konfiguration, aenderungen, risse, csv_data, render_hintergrund_vorschau, use_dgm) {
    const ptr0 = passStringToWasm0(split_nas_xml, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passStringToWasm0(nas_xml, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
    const len1 = WASM_VECTOR_LEN;
    const ptr2 = passStringToWasm0(projekt_info, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
    const len2 = WASM_VECTOR_LEN;
    const ptr3 = passStringToWasm0(konfiguration, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
    const len3 = WASM_VECTOR_LEN;
    const ptr4 = passStringToWasm0(aenderungen, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
    const len4 = WASM_VECTOR_LEN;
    const ptr5 = passStringToWasm0(risse, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
    const len5 = WASM_VECTOR_LEN;
    const ptr6 = passStringToWasm0(csv_data, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
    const len6 = WASM_VECTOR_LEN;
    const ret = wasm.aenderungen_zu_geograf(ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5, ptr6, len6, render_hintergrund_vorschau, use_dgm);
    return takeObject(ret);
}

/**
* @param {string} aenderungen
* @param {string} nas_xml
* @param {string} split_nas
* @param {string} xml_objects
* @param {string} csv_data
* @returns {string}
*/
export function aenderungen_zu_nas_xml(aenderungen, nas_xml, split_nas, xml_objects, csv_data) {
    let deferred6_0;
    let deferred6_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(aenderungen, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(nas_xml, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(split_nas, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passStringToWasm0(xml_objects, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len3 = WASM_VECTOR_LEN;
        const ptr4 = passStringToWasm0(csv_data, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len4 = WASM_VECTOR_LEN;
        wasm.aenderungen_zu_nas_xml(retptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred6_0 = r0;
        deferred6_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred6_0, deferred6_1, 1);
    }
}

/**
* @param {string} datum
* @param {string} aenderungen
* @param {string} nas_xml
* @param {string} split_nas
* @param {string} xml_objects
* @param {string} csv_data
* @returns {string}
*/
export function aenderungen_zu_david(datum, aenderungen, nas_xml, split_nas, xml_objects, csv_data) {
    let deferred7_0;
    let deferred7_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(datum, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(aenderungen, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(nas_xml, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passStringToWasm0(split_nas, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len3 = WASM_VECTOR_LEN;
        const ptr4 = passStringToWasm0(xml_objects, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len4 = WASM_VECTOR_LEN;
        const ptr5 = passStringToWasm0(csv_data, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len5 = WASM_VECTOR_LEN;
        wasm.aenderungen_zu_david(retptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred7_0 = r0;
        deferred7_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred7_0, deferred7_1, 1);
    }
}

/**
* @param {string} aenderungen
* @param {string} target_crs
* @returns {string}
*/
export function get_geojson_fuer_neue_polygone(aenderungen, target_crs) {
    let deferred3_0;
    let deferred3_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(aenderungen, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(target_crs, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        wasm.get_geojson_fuer_neue_polygone(retptr, ptr0, len0, ptr1, len1);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred3_0 = r0;
        deferred3_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred3_0, deferred3_1, 1);
    }
}

/**
* @param {string} map_bounds
* @param {string} nas_xml
* @returns {string}
*/
export function get_flurstuecke_in_extent(map_bounds, nas_xml) {
    let deferred3_0;
    let deferred3_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(map_bounds, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(nas_xml, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        wasm.get_flurstuecke_in_extent(retptr, ptr0, len0, ptr1, len1);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred3_0 = r0;
        deferred3_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred3_0, deferred3_1, 1);
    }
}

/**
* @param {string} split_flurstuecke
* @param {string | undefined} crs
* @param {string} aenderungen
* @param {string} map_bounds
* @returns {string}
*/
export function get_polyline_guides_in_current_bounds(split_flurstuecke, crs, aenderungen, map_bounds) {
    let deferred5_0;
    let deferred5_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(split_flurstuecke, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(crs) ? 0 : passStringToWasm0(crs, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        var len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(aenderungen, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passStringToWasm0(map_bounds, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len3 = WASM_VECTOR_LEN;
        wasm.get_polyline_guides_in_current_bounds(retptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred5_0 = r0;
        deferred5_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred5_0, deferred5_1, 1);
    }
}

/**
* @param {string} points
* @param {string} crs
* @returns {string}
*/
export function fixup_polyline_rissgebiet(points, crs) {
    let deferred3_0;
    let deferred3_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(points, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(crs, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        wasm.fixup_polyline_rissgebiet(retptr, ptr0, len0, ptr1, len1);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred3_0 = r0;
        deferred3_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred3_0, deferred3_1, 1);
    }
}

/**
* @param {string} aenderungen
* @param {string} target_crs
* @returns {string}
*/
export function reproject_aenderungen_for_view(aenderungen, target_crs) {
    let deferred3_0;
    let deferred3_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(aenderungen, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(target_crs, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        wasm.reproject_aenderungen_for_view(retptr, ptr0, len0, ptr1, len1);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred3_0 = r0;
        deferred3_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred3_0, deferred3_1, 1);
    }
}

/**
* @param {string} xml
* @param {string} split_flurstuecke
* @param {string} points
* @param {string} id
* @param {string} aenderungen
* @param {string} config
* @returns {string}
*/
export function fixup_polyline(xml, split_flurstuecke, points, id, aenderungen, config) {
    let deferred7_0;
    let deferred7_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(xml, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(split_flurstuecke, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(points, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passStringToWasm0(id, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len3 = WASM_VECTOR_LEN;
        const ptr4 = passStringToWasm0(aenderungen, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len4 = WASM_VECTOR_LEN;
        const ptr5 = passStringToWasm0(config, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len5 = WASM_VECTOR_LEN;
        wasm.fixup_polyline(retptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred7_0 = r0;
        deferred7_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred7_0, deferred7_1, 1);
    }
}

/**
* @param {string} projektinfo
* @param {string} risse
* @param {string} uidata
* @param {string} csv
* @param {string} aenderungen
* @param {string} konfiguration
* @returns {string}
*/
export function ui_render_entire_screen(projektinfo, risse, uidata, csv, aenderungen, konfiguration) {
    let deferred7_0;
    let deferred7_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(projektinfo, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(risse, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(uidata, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passStringToWasm0(csv, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len3 = WASM_VECTOR_LEN;
        const ptr4 = passStringToWasm0(aenderungen, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len4 = WASM_VECTOR_LEN;
        const ptr5 = passStringToWasm0(konfiguration, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len5 = WASM_VECTOR_LEN;
        wasm.ui_render_entire_screen(retptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred7_0 = r0;
        deferred7_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred7_0, deferred7_1, 1);
    }
}

/**
* @param {string} decoded
* @returns {string}
*/
export function ui_render_ribbon(decoded) {
    let deferred2_0;
    let deferred2_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(decoded, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        wasm.ui_render_ribbon(retptr, ptr0, len0);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred2_0 = r0;
        deferred2_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred2_0, deferred2_1, 1);
    }
}

/**
* @param {string} search_term
* @returns {string}
*/
export function ui_render_search_popover_content(search_term) {
    let deferred2_0;
    let deferred2_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(search_term, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        wasm.ui_render_search_popover_content(retptr, ptr0, len0);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred2_0 = r0;
        deferred2_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred2_0, deferred2_1, 1);
    }
}

/**
* @param {string} decoded
* @param {string} konfiguration
* @returns {string}
*/
export function ui_render_popover_content(decoded, konfiguration) {
    let deferred3_0;
    let deferred3_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(decoded, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(konfiguration, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        wasm.ui_render_popover_content(retptr, ptr0, len0, ptr1, len1);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred3_0 = r0;
        deferred3_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred3_0, deferred3_1, 1);
    }
}

/**
* @param {string} uidata
* @returns {string}
*/
export function ui_render_switch_content(uidata) {
    let deferred2_0;
    let deferred2_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(uidata, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        wasm.ui_render_switch_content(retptr, ptr0, len0);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred2_0 = r0;
        deferred2_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred2_0, deferred2_1, 1);
    }
}

/**
* @param {string} projektinfo
* @param {string} risse
* @param {string} uidata
* @param {string} csv_data
* @param {string} aenderungen
* @param {string | undefined} [split_flurstuecke]
* @returns {string}
*/
export function ui_render_project_content(projektinfo, risse, uidata, csv_data, aenderungen, split_flurstuecke) {
    let deferred7_0;
    let deferred7_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(projektinfo, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(risse, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(uidata, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passStringToWasm0(csv_data, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len3 = WASM_VECTOR_LEN;
        const ptr4 = passStringToWasm0(aenderungen, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len4 = WASM_VECTOR_LEN;
        var ptr5 = isLikeNone(split_flurstuecke) ? 0 : passStringToWasm0(split_flurstuecke, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        var len5 = WASM_VECTOR_LEN;
        wasm.ui_render_project_content(retptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred7_0 = r0;
        deferred7_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred7_0, deferred7_1, 1);
    }
}

/**
* @param {string} s
* @returns {string}
*/
export function get_geojson_polygon(s) {
    let deferred2_0;
    let deferred2_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(s, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        wasm.get_geojson_polygon(retptr, ptr0, len0);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred2_0 = r0;
        deferred2_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred2_0, deferred2_1, 1);
    }
}

/**
* @param {string} s
* @returns {string}
*/
export function get_fit_bounds(s) {
    let deferred2_0;
    let deferred2_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(s, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        wasm.get_fit_bounds(retptr, ptr0, len0);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred2_0 = r0;
        deferred2_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred2_0, deferred2_1, 1);
    }
}

/**
* @param {string} nas_xml
* @param {string} id
* @returns {string}
*/
export function search_for_id(nas_xml, id) {
    let deferred3_0;
    let deferred3_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(nas_xml, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(id, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        wasm.search_for_id(retptr, ptr0, len0, ptr1, len1);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred3_0 = r0;
        deferred3_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred3_0, deferred3_1, 1);
    }
}

/**
* @param {string} aenderungen
* @param {string} poly_id
* @returns {string}
*/
export function search_for_polyneu(aenderungen, poly_id) {
    let deferred3_0;
    let deferred3_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(aenderungen, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(poly_id, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        wasm.search_for_polyneu(retptr, ptr0, len0, ptr1, len1);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred3_0 = r0;
        deferred3_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred3_0, deferred3_1, 1);
    }
}

/**
* @param {string} s
* @param {string} gebaeude_id
* @returns {string}
*/
export function search_for_gebauede(s, gebaeude_id) {
    let deferred3_0;
    let deferred3_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(s, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(gebaeude_id, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        wasm.search_for_gebauede(retptr, ptr0, len0, ptr1, len1);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred3_0 = r0;
        deferred3_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred3_0, deferred3_1, 1);
    }
}

/**
* @param {string} konfiguration
* @returns {string}
*/
export function get_ebenen_darstellung(konfiguration) {
    let deferred2_0;
    let deferred2_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(konfiguration, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        wasm.get_ebenen_darstellung(retptr, ptr0, len0);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred2_0 = r0;
        deferred2_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred2_0, deferred2_1, 1);
    }
}

/**
* @param {string} s
* @param {string} style
* @returns {string}
*/
export function load_nas_xml(s, style) {
    let deferred3_0;
    let deferred3_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(s, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(style, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        wasm.load_nas_xml(retptr, ptr0, len0, ptr1, len1);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred3_0 = r0;
        deferred3_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred3_0, deferred3_1, 1);
    }
}

/**
* @param {string} json
* @param {string} layer
* @returns {string}
*/
export function get_geojson_fuer_ebene(json, layer) {
    let deferred3_0;
    let deferred3_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(json, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(layer, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        wasm.get_geojson_fuer_ebene(retptr, ptr0, len0, ptr1, len1);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred3_0 = r0;
        deferred3_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred3_0, deferred3_1, 1);
    }
}

/**
* @param {string} konfiguration
* @param {string} layer_name
* @returns {string}
*/
export function get_layer_style(konfiguration, layer_name) {
    let deferred3_0;
    let deferred3_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(konfiguration, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(layer_name, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        wasm.get_layer_style(retptr, ptr0, len0, ptr1, len1);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred3_0 = r0;
        deferred3_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred3_0, deferred3_1, 1);
    }
}

/**
* @param {string} json
* @param {string} csv
* @param {string} aenderungen
* @returns {string}
*/
export function get_gebaeude_geojson_fuer_aktive_flst(json, csv, aenderungen) {
    let deferred4_0;
    let deferred4_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(json, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(csv, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(aenderungen, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len2 = WASM_VECTOR_LEN;
        wasm.get_gebaeude_geojson_fuer_aktive_flst(retptr, ptr0, len0, ptr1, len1, ptr2, len2);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred4_0 = r0;
        deferred4_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred4_0, deferred4_1, 1);
    }
}

/**
* @param {string} json
* @param {string} layer
* @returns {string}
*/
export function get_labels_fuer_ebene(json, layer) {
    let deferred3_0;
    let deferred3_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(json, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(layer, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        wasm.get_labels_fuer_ebene(retptr, ptr0, len0, ptr1, len1);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred3_0 = r0;
        deferred3_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred3_0, deferred3_1, 1);
    }
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}
/**
* @param {Uint8Array} csv
* @param {string} id_col
* @param {string} nutzung_col
* @param {string} eigentuemer_col
* @param {string} delimiter
* @param {string} ignore_firstline
* @returns {string}
*/
export function parse_csv_dataset_to_json(csv, id_col, nutzung_col, eigentuemer_col, delimiter, ignore_firstline) {
    let deferred7_0;
    let deferred7_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passArray8ToWasm0(csv, wasm.__wbindgen_export_1);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(id_col, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(nutzung_col, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passStringToWasm0(eigentuemer_col, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len3 = WASM_VECTOR_LEN;
        const ptr4 = passStringToWasm0(delimiter, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len4 = WASM_VECTOR_LEN;
        const ptr5 = passStringToWasm0(ignore_firstline, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len5 = WASM_VECTOR_LEN;
        wasm.parse_csv_dataset_to_json(retptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred7_0 = r0;
        deferred7_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred7_0, deferred7_1, 1);
    }
}

/**
* @param {string} s
* @returns {string}
*/
export function export_alle_flst(s) {
    let deferred2_0;
    let deferred2_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(s, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        wasm.export_alle_flst(retptr, ptr0, len0);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred2_0 = r0;
        deferred2_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred2_0, deferred2_1, 1);
    }
}

/**
* @param {string} konfiguration
* @param {string} xml_nas
* @returns {string}
*/
export function edit_konfiguration_layer_alle(konfiguration, xml_nas) {
    let deferred3_0;
    let deferred3_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(konfiguration, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(xml_nas, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        wasm.edit_konfiguration_layer_alle(retptr, ptr0, len0, ptr1, len1);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred3_0 = r0;
        deferred3_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred3_0, deferred3_1, 1);
    }
}

/**
* @param {string} konfiguration
* @param {string} layer_type
* @returns {string}
*/
export function edit_konfiguration_layer_neu(konfiguration, layer_type) {
    let deferred3_0;
    let deferred3_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(konfiguration, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(layer_type, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        wasm.edit_konfiguration_layer_neu(retptr, ptr0, len0, ptr1, len1);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred3_0 = r0;
        deferred3_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred3_0, deferred3_1, 1);
    }
}

/**
* @param {string} konfiguration
* @param {string} layer_type
* @param {string} ebene_id
* @param {string} move_type
* @returns {string}
*/
export function edit_konfiguration_move_layer(konfiguration, layer_type, ebene_id, move_type) {
    let deferred5_0;
    let deferred5_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(konfiguration, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(layer_type, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(ebene_id, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passStringToWasm0(move_type, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len3 = WASM_VECTOR_LEN;
        wasm.edit_konfiguration_move_layer(retptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred5_0 = r0;
        deferred5_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export_0(deferred5_0, deferred5_1, 1);
    }
}

const PointFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_point_free(ptr >>> 0, 1));
/**
*/
export class Point {

    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        PointFinalization.unregister(this);
        return ptr;
    }

    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_point_free(ptr, 0);
    }
    /**
    * @param {number} x
    * @param {number} y
    * @param {number} z
    */
    constructor(x, y, z) {
        const ret = wasm.point_new(x, y, z);
        this.__wbg_ptr = ret >>> 0;
        PointFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
    * @returns {number}
    */
    get x() {
        const ret = wasm.point_x(this.__wbg_ptr);
        return ret;
    }
    /**
    * @param {number} x
    */
    set x(x) {
        wasm.point_set_x(this.__wbg_ptr, x);
    }
    /**
    * @returns {number}
    */
    get y() {
        const ret = wasm.point_y(this.__wbg_ptr);
        return ret;
    }
    /**
    * @param {number} y
    */
    set y(y) {
        wasm.point_set_y(this.__wbg_ptr, y);
    }
    /**
    * @returns {number}
    */
    get z() {
        const ret = wasm.point_z(this.__wbg_ptr);
        return ret;
    }
    /**
    * @param {number} z
    */
    set z(z) {
        wasm.point_set_z(this.__wbg_ptr, z);
    }
}

const ProjectionFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_projection_free(ptr >>> 0, 1));
/**
*/
export class Projection {

    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ProjectionFinalization.unregister(this);
        return ptr;
    }

    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_projection_free(ptr, 0);
    }
    /**
    * @param {string} defn
    */
    constructor(defn) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(defn, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
            const len0 = WASM_VECTOR_LEN;
            wasm.projection_new(retptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            this.__wbg_ptr = r0 >>> 0;
            ProjectionFinalization.register(this, this.__wbg_ptr, this);
            return this;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
    * @returns {string}
    */
    get projName() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.projection_projName(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export_0(deferred1_0, deferred1_1, 1);
        }
    }
    /**
    * @returns {boolean}
    */
    get isLatlon() {
        const ret = wasm.projection_isLatlon(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
    * @returns {boolean}
    */
    get isGeocentric() {
        const ret = wasm.projection_isGeocentric(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
    * @returns {string}
    */
    get axis() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.projection_axis(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export_0(deferred1_0, deferred1_1, 1);
        }
    }
    /**
    * @returns {boolean}
    */
    get isNormalizedAxis() {
        const ret = wasm.projection_isNormalizedAxis(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
    * @returns {number}
    */
    get to_meter() {
        const ret = wasm.projection_to_meter(this.__wbg_ptr);
        return ret;
    }
    /**
    * @returns {string}
    */
    get units() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.projection_units(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export_0(deferred1_0, deferred1_1, 1);
        }
    }
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);

            } catch (e) {
                if (module.headers.get('Content-Type') != 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else {
                    throw e;
                }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);

    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };

        } else {
            return instance;
        }
    }
}

function __wbg_get_imports() {
    const imports = {};
    imports.wbg = {};
    imports.wbg.__wbindgen_uint8_array_new = function(arg0, arg1) {
        var v0 = getArrayU8FromWasm0(arg0, arg1).slice();
        wasm.__wbindgen_export_0(arg0, arg1 * 1, 1);
        const ret = v0;
        return addHeapObject(ret);
    };
    imports.wbg.__wbindgen_number_new = function(arg0) {
        const ret = arg0;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_new_7982fb43cfca37ae = function(arg0) {
        const ret = new Date(getObject(arg0));
        return addHeapObject(ret);
    };
    imports.wbg.__wbindgen_object_drop_ref = function(arg0) {
        takeObject(arg0);
    };
    imports.wbg.__wbg_getTimezoneOffset_c9929a3cc94500fe = function(arg0) {
        const ret = getObject(arg0).getTimezoneOffset();
        return ret;
    };
    imports.wbg.__wbg_new0_65387337a95cf44d = function() {
        const ret = new Date();
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_getTime_91058879093a1589 = function(arg0) {
        const ret = getObject(arg0).getTime();
        return ret;
    };
    imports.wbg.__wbg_newwithyearmonthdayhrminsec_d95c77209c89164d = function(arg0, arg1, arg2, arg3, arg4, arg5) {
        const ret = new Date(arg0 >>> 0, arg1, arg2, arg3, arg4, arg5);
        return addHeapObject(ret);
    };
    imports.wbg.__wbindgen_is_object = function(arg0) {
        const val = getObject(arg0);
        const ret = typeof(val) === 'object' && val !== null;
        return ret;
    };
    imports.wbg.__wbg_crypto_1d1f22824a6a080c = function(arg0) {
        const ret = getObject(arg0).crypto;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_process_4a72847cc503995b = function(arg0) {
        const ret = getObject(arg0).process;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_versions_f686565e586dd935 = function(arg0) {
        const ret = getObject(arg0).versions;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_node_104a2ff8d6ea03a2 = function(arg0) {
        const ret = getObject(arg0).node;
        return addHeapObject(ret);
    };
    imports.wbg.__wbindgen_is_string = function(arg0) {
        const ret = typeof(getObject(arg0)) === 'string';
        return ret;
    };
    imports.wbg.__wbg_require_cca90b1a94a0255b = function() { return handleError(function () {
        const ret = module.require;
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbindgen_string_new = function(arg0, arg1) {
        const ret = getStringFromWasm0(arg0, arg1);
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_msCrypto_eb05e62b530a1508 = function(arg0) {
        const ret = getObject(arg0).msCrypto;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_newwithlength_ec548f448387c968 = function(arg0) {
        const ret = new Uint8Array(arg0 >>> 0);
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_call_1084a111329e68ce = function() { return handleError(function (arg0, arg1) {
        const ret = getObject(arg0).call(getObject(arg1));
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbindgen_object_clone_ref = function(arg0) {
        const ret = getObject(arg0);
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_self_3093d5d1f7bcb682 = function() { return handleError(function () {
        const ret = self.self;
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbg_window_3bcfc4d31bc012f8 = function() { return handleError(function () {
        const ret = window.window;
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbg_globalThis_86b222e13bdf32ed = function() { return handleError(function () {
        const ret = globalThis.globalThis;
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbg_global_e5a3fe56f8be9485 = function() { return handleError(function () {
        const ret = global.global;
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbindgen_is_undefined = function(arg0) {
        const ret = getObject(arg0) === undefined;
        return ret;
    };
    imports.wbg.__wbg_newnoargs_76313bd6ff35d0f2 = function(arg0, arg1) {
        const ret = new Function(getStringFromWasm0(arg0, arg1));
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_getInt32_15561582072bf5e9 = function(arg0, arg1, arg2) {
        const ret = getObject(arg0).getInt32(arg1 >>> 0, arg2 !== 0);
        return ret;
    };
    imports.wbg.__wbg_getUint32_0aebf6b991eb16be = function(arg0, arg1, arg2) {
        const ret = getObject(arg0).getUint32(arg1 >>> 0, arg2 !== 0);
        return ret;
    };
    imports.wbg.__wbg_getFloat32_3028134421611c5d = function(arg0, arg1, arg2) {
        const ret = getObject(arg0).getFloat32(arg1 >>> 0, arg2 !== 0);
        return ret;
    };
    imports.wbg.__wbg_getFloat64_41ed54ab83a47d2b = function(arg0, arg1, arg2) {
        const ret = getObject(arg0).getFloat64(arg1 >>> 0, arg2 !== 0);
        return ret;
    };
    imports.wbg.__wbg_call_89af060b4e1523f2 = function() { return handleError(function (arg0, arg1, arg2) {
        const ret = getObject(arg0).call(getObject(arg1), getObject(arg2));
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbg_getDate_52f8a749afd35eda = function(arg0) {
        const ret = getObject(arg0).getDate();
        return ret;
    };
    imports.wbg.__wbg_getFullYear_9649dddf8a157b21 = function(arg0) {
        const ret = getObject(arg0).getFullYear();
        return ret;
    };
    imports.wbg.__wbg_getHours_3758479fff0cc553 = function(arg0) {
        const ret = getObject(arg0).getHours();
        return ret;
    };
    imports.wbg.__wbg_getMinutes_9a73b3e678d5346f = function(arg0) {
        const ret = getObject(arg0).getMinutes();
        return ret;
    };
    imports.wbg.__wbg_getMonth_e3a0afc07cbc1328 = function(arg0) {
        const ret = getObject(arg0).getMonth();
        return ret;
    };
    imports.wbg.__wbg_getSeconds_7d2d158963b9702c = function(arg0) {
        const ret = getObject(arg0).getSeconds();
        return ret;
    };
    imports.wbg.__wbindgen_memory = function() {
        const ret = wasm.memory;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_buffer_b7b08af79b0b0974 = function(arg0) {
        const ret = getObject(arg0).buffer;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_new_ea1883e1e5e86686 = function(arg0) {
        const ret = new Uint8Array(getObject(arg0));
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_set_d1e79e2388520f18 = function(arg0, arg1, arg2) {
        getObject(arg0).set(getObject(arg1), arg2 >>> 0);
    };
    imports.wbg.__wbindgen_is_function = function(arg0) {
        const ret = typeof(getObject(arg0)) === 'function';
        return ret;
    };
    imports.wbg.__wbg_parseInt_d764958c60f6d3fb = function(arg0, arg1, arg2) {
        const ret = parseInt(getStringFromWasm0(arg0, arg1), arg2);
        return ret;
    };
    imports.wbg.__wbg_buffer_cb1b140b7c25c485 = function(arg0) {
        const ret = getObject(arg0).buffer;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_slice_f2eec3f5b3eba38d = function(arg0, arg1, arg2) {
        const ret = getObject(arg0).slice(arg1 >>> 0, arg2 >>> 0);
        return addHeapObject(ret);
    };
    imports.wbg.__wbindgen_string_get = function(arg0, arg1) {
        const obj = getObject(arg1);
        const ret = typeof(obj) === 'string' ? obj : undefined;
        var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        var len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbindgen_error_new = function(arg0, arg1) {
        const ret = new Error(getStringFromWasm0(arg0, arg1));
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_parseFloat_b1b04fcaf22971b0 = function(arg0, arg1) {
        const ret = parseFloat(getStringFromWasm0(arg0, arg1));
        return ret;
    };
    imports.wbg.__wbg_abort_8659d889a7877ae3 = function(arg0) {
        getObject(arg0).abort();
    };
    imports.wbg.__wbg_random_4bc01a1f182e92dc = typeof Math.random == 'function' ? Math.random : notDefined('Math.random');
    imports.wbg.__wbg_log_b103404cc5920657 = function(arg0) {
        console.log(getObject(arg0));
    };
    imports.wbg.__wbg_updateexportstatus_dff9b10d495f2e73 = function(arg0, arg1) {
        let deferred0_0;
        let deferred0_1;
        try {
            deferred0_0 = arg0;
            deferred0_1 = arg1;
            update_export_status(getStringFromWasm0(arg0, arg1));
        } finally {
            wasm.__wbindgen_export_0(deferred0_0, deferred0_1, 1);
        }
    };
    imports.wbg.__wbg_getnak_d42c4edc9d6e4205 = function(arg0) {
        const ret = get_nak();
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbg_new_b85e72ed1bfd57f9 = function(arg0, arg1) {
        try {
            var state0 = {a: arg0, b: arg1};
            var cb0 = (arg0, arg1) => {
                const a = state0.a;
                state0.a = 0;
                try {
                    return __wbg_adapter_105(a, state0.b, arg0, arg1);
                } finally {
                    state0.a = a;
                }
            };
            const ret = new Promise(cb0);
            return addHeapObject(ret);
        } finally {
            state0.a = state0.b = 0;
        }
    };
    imports.wbg.__wbg_exportstatusclear_696b2edfbc03c672 = typeof export_status_clear == 'function' ? export_status_clear : notDefined('export_status_clear');
    imports.wbg.__wbg_new_525245e2b9901204 = function() {
        const ret = new Object();
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_setmethod_dc68a742c2db5c6a = function(arg0, arg1, arg2) {
        getObject(arg0).method = getStringFromWasm0(arg1, arg2);
    };
    imports.wbg.__wbg_new_e27c93803e1acc42 = function() { return handleError(function () {
        const ret = new Headers();
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbg_setheaders_be10a5ab566fd06f = function(arg0, arg1) {
        getObject(arg0).headers = getObject(arg1);
    };
    imports.wbg.__wbg_setmode_a781aae2bd3df202 = function(arg0, arg1) {
        getObject(arg0).mode = ["same-origin","no-cors","cors","navigate",][arg1];
    };
    imports.wbg.__wbg_setcredentials_2b67800db3f7b621 = function(arg0, arg1) {
        getObject(arg0).credentials = ["omit","same-origin","include",][arg1];
    };
    imports.wbg.__wbg_newwithbyteoffsetandlength_8a2cb9ca96b27ec9 = function(arg0, arg1, arg2) {
        const ret = new Uint8Array(getObject(arg0), arg1 >>> 0, arg2 >>> 0);
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_setbody_734cb3d7ee8e6e96 = function(arg0, arg1) {
        getObject(arg0).body = getObject(arg1);
    };
    imports.wbg.__wbg_append_f3a4426bb50622c5 = function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
        getObject(arg0).append(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
    }, arguments) };
    imports.wbg.__wbg_new_ebf2727385ee825c = function() { return handleError(function () {
        const ret = new AbortController();
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbg_signal_41e46ccad44bb5e2 = function(arg0) {
        const ret = getObject(arg0).signal;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_setsignal_91c4e8ebd04eb935 = function(arg0, arg1) {
        getObject(arg0).signal = getObject(arg1);
    };
    imports.wbg.__wbg_newwithstrandinit_a31c69e4cc337183 = function() { return handleError(function (arg0, arg1, arg2) {
        const ret = new Request(getStringFromWasm0(arg0, arg1), getObject(arg2));
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbg_has_4bfbc01db38743f7 = function() { return handleError(function (arg0, arg1) {
        const ret = Reflect.has(getObject(arg0), getObject(arg1));
        return ret;
    }, arguments) };
    imports.wbg.__wbg_fetch_ba7fe179e527d942 = function(arg0, arg1) {
        const ret = getObject(arg0).fetch(getObject(arg1));
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_fetch_25e3a297f7b04639 = function(arg0) {
        const ret = fetch(getObject(arg0));
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_instanceof_Response_e91b7eb7c611a9ae = function(arg0) {
        let result;
        try {
            result = getObject(arg0) instanceof Response;
        } catch (_) {
            result = false;
        }
        const ret = result;
        return ret;
    };
    imports.wbg.__wbg_status_ae8de515694c5c7c = function(arg0) {
        const ret = getObject(arg0).status;
        return ret;
    };
    imports.wbg.__wbg_url_1bf85c8abeb8c92d = function(arg0, arg1) {
        const ret = getObject(arg1).url;
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbg_headers_5e283e8345689121 = function(arg0) {
        const ret = getObject(arg0).headers;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_iterator_888179a48810a9fe = function() {
        const ret = Symbol.iterator;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_get_224d16597dbbfd96 = function() { return handleError(function (arg0, arg1) {
        const ret = Reflect.get(getObject(arg0), getObject(arg1));
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbg_next_de3e9db4440638b2 = function(arg0) {
        const ret = getObject(arg0).next;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_next_f9cb570345655b9a = function() { return handleError(function (arg0) {
        const ret = getObject(arg0).next();
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbg_done_bfda7aa8f252b39f = function(arg0) {
        const ret = getObject(arg0).done;
        return ret;
    };
    imports.wbg.__wbg_value_6d39332ab4788d86 = function(arg0) {
        const ret = getObject(arg0).value;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_stringify_bbf45426c92a6bf5 = function() { return handleError(function (arg0) {
        const ret = JSON.stringify(getObject(arg0));
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbg_arrayBuffer_a5fbad63cc7e663b = function() { return handleError(function (arg0) {
        const ret = getObject(arg0).arrayBuffer();
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbg_length_8339fcf5d8ecd12e = function(arg0) {
        const ret = getObject(arg0).length;
        return ret;
    };
    imports.wbg.__wbg_randomFillSync_5c9c955aa56b6049 = function() { return handleError(function (arg0, arg1) {
        getObject(arg0).randomFillSync(takeObject(arg1));
    }, arguments) };
    imports.wbg.__wbg_subarray_7c2e3576afe181d1 = function(arg0, arg1, arg2) {
        const ret = getObject(arg0).subarray(arg1 >>> 0, arg2 >>> 0);
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_getRandomValues_3aa56aa6edec874c = function() { return handleError(function (arg0, arg1) {
        getObject(arg0).getRandomValues(getObject(arg1));
    }, arguments) };
    imports.wbg.__wbindgen_throw = function(arg0, arg1) {
        throw new Error(getStringFromWasm0(arg0, arg1));
    };
    imports.wbg.__wbindgen_debug_string = function(arg0, arg1) {
        const ret = debugString(getObject(arg1));
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export_1, wasm.__wbindgen_export_2);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbg_then_876bb3c633745cc6 = function(arg0, arg1, arg2) {
        const ret = getObject(arg0).then(getObject(arg1), getObject(arg2));
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_queueMicrotask_48421b3cc9052b68 = function(arg0) {
        const ret = getObject(arg0).queueMicrotask;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_resolve_570458cb99d56a43 = function(arg0) {
        const ret = Promise.resolve(getObject(arg0));
        return addHeapObject(ret);
    };
    imports.wbg.__wbindgen_cb_drop = function(arg0) {
        const obj = takeObject(arg0).original;
        if (obj.cnt-- == 1) {
            obj.a = 0;
            return true;
        }
        const ret = false;
        return ret;
    };
    imports.wbg.__wbg_then_95e6edc0f89b73b1 = function(arg0, arg1) {
        const ret = getObject(arg0).then(getObject(arg1));
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_queueMicrotask_12a30234db4045d3 = function(arg0) {
        queueMicrotask(getObject(arg0));
    };
    imports.wbg.__wbindgen_closure_wrapper8327 = function(arg0, arg1, arg2) {
        const ret = makeMutClosure(arg0, arg1, 455, __wbg_adapter_32);
        return addHeapObject(ret);
    };

    return imports;
}

function __wbg_init_memory(imports, memory) {

}

function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    __wbg_init.__wbindgen_wasm_module = module;
    cachedDataViewMemory0 = null;
    cachedUint8ArrayMemory0 = null;


    wasm.__wbindgen_start();
    return wasm;
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (typeof module !== 'undefined' && Object.getPrototypeOf(module) === Object.prototype)
    ({module} = module)
    else
    console.warn('using deprecated parameters for `initSync()`; pass a single object instead')

    const imports = __wbg_get_imports();

    __wbg_init_memory(imports);

    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }

    const instance = new WebAssembly.Instance(module, imports);

    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (typeof module_or_path !== 'undefined' && Object.getPrototypeOf(module_or_path) === Object.prototype)
    ({module_or_path} = module_or_path)
    else
    console.warn('using deprecated parameters for the initialization function; pass a single object instead')

    if (typeof module_or_path === 'undefined') {
        module_or_path = new URL('tnviewer_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    __wbg_init_memory(imports);

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync };
export default __wbg_init;
