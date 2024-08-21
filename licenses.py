import json
import re

def read_file(path):
    text_file = open(path, 'r')
    text_file_contents = text_file.read()
    text_file.close()
    return text_file_contents

def write_file(string, path):
    text_file = open(path, "w+", newline='')
    text_file.write(string)
    text_file.close()

s = read_file("./licenses.json")
y = json.loads(s)

html = "<body style='font-family:sans-serif;'>"
html += "<h1>TNViewer - Rechtliche Dokumentation</h1>\r\n"
html += "<h2>Lizenzen (Lizenztext siehe unten)</h2>\r\n"
licenses = {}

y.append({"name": "LeafletJS", "version": "0.7.7", "authors": "Volodymyr Agafonkin", "repository": "https://leafletjs.com","license": "MIT"})
y.append({"name": "LeafletJS.draw", "version": "0.7.7", "authors": "Jacob Toye", "repository": "https://github.com/Leaflet/Leaflet.draw","license": "MIT"})
y.append({"name": "Leaflet.snap", "version": "0.0.5", "authors": "Mathieu Leplatre, Tobias Bieniek, Frédéric Bonifas", "repository": "https://github.com/makinacorpus/Leaflet.Snap","license": "MIT"})

for obj in y:
    name = ""
    if obj["authors"] is not None:
        name = " von " + re.sub(r"<(.*)>", "", obj["authors"]).strip().replace("|", " und ")
        name = re.sub(r"<(.*)", "", name)
    license = ""
    if "MIT" in obj["license"]:
        license = "MIT"
    elif "Apache-2.0" in obj["license"]:
        license = "Apache-2.0"
    else:
        license = obj["license"]
    licenses[license] = ""
    html += "<div style='padding:2px;page-break-inside:avoid;'>"
    html += "<p style='line-height:0.7;font-family:monospace;font-size:12px;font-weight:bold;'>" + obj["name"] + " v" + obj["version"] + "" + name + ": "  + license + "</p>\r\n"
    repository = obj["repository"]
    if repository is None:
        repository = "https://crates.io/crates/" + obj["name"] + "/" + obj["version"]
    html += "<a style='font-family:monospace;line-height:0.1;font-size:12px;font-weight:bold;'href= " + repository + ">" + repository + "</a>\r\n"
    html += "</div>"

all_licenses = list(licenses.keys())

for l in all_licenses:
    text = read_file("./licenses/" + l + ".de.txt")
    html += "<div style='page-break-before: always;'></div>"
    html += "<h3>" + l + "</h3>\r\n"
    html += "<pre>\r\n" + text + "</pre>\r\n"

html += "</body>"
write_file(html, "./licenses.html")