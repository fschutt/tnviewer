import json
import re
import pprint
import os
import subprocess
import shutil
from sys import platform
import time
import base64

def create_folder(path):
    os.mkdir(path)

def remove_path(path):
    """ param <path> could either be relative or absolute. """
    if os.path.isfile(path) or os.path.islink(path):
        os.remove(path)  # remove the file
    elif os.path.isdir(path):
        shutil.rmtree(path)  # remove dir and all contains
    else:
        raise ValueError("file {} is not a file or dir.".format(path))

def zip_directory(output_filename, dir_name):
    shutil.make_archive(output_filename, 'zip', dir_name)

def copy_file(src, dest):
    shutil.copyfile(src, dest)

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

index_html = read_file("./index.html")
pkg_viewer_wasm = read_file_base64("./pkg/tnviewer_bg.wasm")
pkg_viewer_js = read_file("./pkg/tnviewer.js")
fixup_js = read_file("./js/fixup.js")
fixup2_js = read_file("./js/fixup2.js")
leaflet_js = read_file("./js/leaflet/leaflet.js")
leaflet_css = read_file("./js/leaflet/leaflet.css")
main_css = read_file("./main.css")

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
for line in index_html.splitlines():
    if "<!--LEAFLET_CSS_LINK-->" in line:
        out_file.append("<style>" + leaflet_css + "</style>")
    elif "<!--LEAFLET_JS-->" in line:
        out_file.append("<script type='text/javascript'>" + leaflet_js +"</script>")
    elif "<!--MAIN_CSS-->" in line:
        out_file.append("<style>" + main_css + "</style>")
    elif "<!--PUT_WASM_JS_HERE-->" in line:
        out_file.append("<script type='text/javascript'> var GLOBAL_WASM = '"  + pkg_viewer_wasm + "'; </script>")
        out_file.append("<script type='module'>" + pkg_viewer_js + "</script>")
    elif "<!--WASM_LOADING_SCRIPT_BEGIN-->" in line:
        write_line = False
    elif "<!--WASM_LOADING_SCRIPT_END-->" in line:
        write_line = True
    else:
        if write_line:
            out_file.append(line)
        else:
            pass

write_file("\r\n".join(out_file), "out.html")
