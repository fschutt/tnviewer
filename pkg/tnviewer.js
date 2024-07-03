import * as wasm from "./tnviewer_bg.wasm";
import { __wbg_set_wasm } from "./tnviewer_bg.js";
__wbg_set_wasm(wasm);
export * from "./tnviewer_bg.js";
