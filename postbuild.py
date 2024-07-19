import base64

def read_file(path):
    text_file = open(path, 'r')
    text_file_contents = text_file.read()
    text_file.close()
    return text_file_contents

def read_file_base64(path):
    encoded_string = ""
    with open(path, "rb") as image_file:
        encoded_string = base64.b64encode(image_file.read()).decode()
    return encoded_string

def write_file(string, path):
    text_file = open(path, "w+", newline='')
    text_file.write(string)
    text_file.close()

def chunks(lst, n):
    """Yield successive n-sized chunks from lst."""
    for i in range(0, len(lst), n):
        yield lst[i:i + n]

index_html = read_file("./skeleton.html")
pkg_viewer_wasm = read_file_base64("./pkg/tnviewer_bg.wasm")
pkg_viewer_js = read_file("./pkg/tnviewer.js")

leaflet_js = read_file("./js/leaflet_07/leaflet.js")
leaflet_css = read_file("./js/leaflet_07/leaflet.css")
leaflet_draw_js = read_file("./js/leaflet_07/leaflet.draw.js")
leaflet_draw_css = read_file("./js/leaflet_07/leaflet.draw.css")
leaflet_snap_js = read_file("./js/leaflet_07/leaflet.snap.js")
leaflet_geometryutil_js = read_file("./js/leaflet_07/leaflet.geometryutil.js")

# leaflet_js = read_file("./js/leaflet/leaflet.js")
# leaflet_css = read_file("./js/leaflet/leaflet.css")
# leaflet_draw_js = read_file("./js/leaflet/leaflet.draw.js")
# leaflet_draw_css = read_file("./js/leaflet/leaflet.draw.css")
# leaflet_snap_js = read_file("./js/leaflet/leaflet.snap.js")
# leaflet_geometryutil_js = read_file("./js/leaflet/leaflet.geometryutil.js")

main_css = read_file("./main.css")
fixup_js = "\r\n".join([
    "async function __wbg_init(input) {",
    "    if (wasm !== undefined) return wasm;",
    "    const imports = __wbg_get_imports();",
    "    __wbg_init_memory(imports);",
    "    var v = base64ToArrayBuffer(window.GLOBAL_WASM);",
    "    const { instance, module } = await WebAssembly.instantiate(v, imports);",
    "    return __wbg_finalize_init(instance, module);",
    "}",
])

fixup2_js = "\r\n".join([
    "window.init = __wbg_init;",
    "window.ui_render_entire_screen = ui_render_entire_screen; ",
    "window.load_nas_xml = load_nas_xml; ",
    "window.get_geojson_fuer_ebene = get_geojson_fuer_ebene;",
    "window.get_labels_fuer_ebene = get_labels_fuer_ebene;",
    "window.ui_render_ribbon = ui_render_ribbon;",
    "window.ui_render_popover_content = ui_render_popover_content;",
    "window.ui_render_project_content = ui_render_project_content;",
    "window.ui_render_secondary_content = ui_render_secondary_content;",
    "window.parse_csv_dataset_to_json = parse_csv_dataset_to_json;",
    "window.get_fit_bounds = get_fit_bounds;",
    "window.export_xlsx = export_xlsx;",
    "window.export_veraenderte_flst = export_veraenderte_flst;",
    "window.export_alle_flst = export_alle_flst;",
    "window.export_flst_id_nach_eigentuemer = export_flst_id_nach_eigentuemer;",
    "window.get_gebaeude_geojson_fuer_aktive_flst = get_gebaeude_geojson_fuer_aktive_flst;",
    "window.export_pdf = export_pdf;",
    "window.search_for_gebauede = search_for_gebauede;",
])

pkg_viewer_js_fixed = []
emit_wr = True
for line in pkg_viewer_js.splitlines():
    if "async function __wbg_init(input) {" in line:
        emit_wr = False
        for l in fixup_js.splitlines():
            pkg_viewer_js_fixed.append(l)
    elif "export { initSync }" in line:
        emit_wr = True
        pkg_viewer_js_fixed.append(line)
    else:
        if emit_wr:
            pkg_viewer_js_fixed.append(line)
        else:
            pass

pkg_viewer_js_fixed.append("")
for l in fixup2_js.splitlines():
    pkg_viewer_js_fixed.append(l)
pkg_viewer_js = "\r\n".join(pkg_viewer_js_fixed)

out_file = []
write_line = True

global_wasm_script = chunks(pkg_viewer_wasm, 100)
wasm_script = ["window.GLOBAL_WASM = ["]
for l in global_wasm_script:
    wasm_script.append("    \"" + l + "\",")
wasm_script_out = "\r\n".join(wasm_script)
wasm_script_out += "\r\n].join('');\r\n"

for line in index_html.splitlines():
    if "<!--LEAFLET_CSS_LINK-->" in line:
        out_file.append("<style>" + leaflet_css + "</style>")
        out_file.append("<style>" + leaflet_draw_css + "</style>")
    elif "<!--LEAFLET_JS-->" in line:
        out_file.append("<script type='text/javascript'>" + leaflet_js +"</script>")
        out_file.append("<script type='text/javascript'>" + leaflet_draw_js +"</script>")
        out_file.append("<script type='text/javascript'>" + leaflet_geometryutil_js +"</script>")
        out_file.append("<script type='text/javascript'>" + leaflet_snap_js +"</script>")
    elif "<!--MAIN_CSS-->" in line:
        out_file.append("<style>" + main_css + "</style>")
    elif "// PUT_WASM_JS_HERE" in line:
        out_file.append(wasm_script_out)
        out_file.append(pkg_viewer_js)
    else:
        out_file.append(line)

write_file("\r\n".join(out_file), "index.html")
