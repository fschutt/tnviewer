# TNViewer

[tnviewer.de](https://tnviewer.de) ist ein Tool zur einfacheren Überprüfung der 
Tatsächlichen Nutzung von Landflächen in Brandenburg (und anderen Bundesländern).

<img width="1470" alt="image" src="https://github.com/user-attachments/assets/6b38d815-bed4-424e-aabb-94ebad9426a8" />

## Technische Dokumentation

### Problemsituation und Arbeitsablauf

Beim Bearbeiten der tatsächlichen Nutzung in ALKIS treten öfter verschiedene Probleme auf, die
technisch durch das Programm „TNViewer“ gelöst werden könnten. Um die Probleme zu verstehen,
muss man zunächst den jetzigen Arbeitsablauf kennen:

Beim Bearbeiten eines Projekts (z.B. einer Gemarkung oder Flur) werden zunächst CSV-Daten aus dem
Liegenschaftskataster (LiKa online) und ALKIS-Daten im NAS-XML-Format aus dem GeoBroker
Brandenburg heruntergeladen. Danach werden die Flurstücke (mittels Excel und GeoGraf) gesichtet,
inwiefern sich die tatsächliche Nutzung verändert. Diese Änderungen werden in einer Excel-Liste
eingetragen, ebenso werden Zeilen markiert, bei denen sich voraussichtlich die Wirtschaftsart ändert.
Danach wird der Vermessungsriss in GeoGraf gezeichnet, d.h. es werden Linien und Texte in GeoGraf
als Grafikobjekte (nicht als Flächen) einzeichnet. Nach der Überprüfung des Risses auf Richtigkeit
werden die Änderungen in DAVID geändert. Nach der Übernahme in die DHK werden die Produkte
(Änderungsmitteilungen) geprüft, hierbei ist wichtig, ob ein Flurstück eine veränderte Wirtschaftsart
hat (da nur Flurstücke relevant sind, wo sich auch die Wirtschaftsart ändert).
Hierbei kommt es zu verschiedenen Fehlerquellen und aufwendigen Prozessen, die umso schlimmer
werden, je größer das Projekt (Flur oder Gemarkung) ist:

1. Es ist sehr einfach, im Riss ein Flurstück zu verändern, aber es in der Excel-Liste [P1] nicht einzutragen.
2. Man muss als Bearbeiter im Voraus wissen, an welchen Flurstücken die Wirtschaftsart geändert
wird. Bedeutet, man muss (auf Papier / im PDF) bei jeder Änderung nachgucken, welcher
Wirtschaftsart diese Änderung zugeordnet ist und ob es diese Wirtschaftsart bereits an
irgendeiner Teilfläche gibt.
3. Wenn man bei dieser Klassifizierung einen Fehler macht, ist das Deckblatt für den
Vermessungsantrag falsch, was nervig ist, weil dort die Unterschriften des Amtsleiters, etc.
eingetragen sind, aber den Beleg, ob man richtig gearbeitet hat, bekommt man erst am Ende
des Projekts.
4. Für jeden Riss muss der Blattkopf die abgebildeten Flurnummern anzeigen und die Legende
muss nur die Kürzel anzeigen, welche im Riss auch zu sehen sind.
5. Grafische Rissobjekte wie „siehe Anschlussriss 4 / 10“ müssen per Hand erstellt werden.
6. Sollte es bei der Bearbeitung in DAVID irgendein technisches Problem geben (z.B. ein Projekt
wurde nicht gesperrt, technische Probleme bei der Fortführung) gibt es keinen Weg, die
Änderungen separat von den ALKIS-Daten zu speichern: wenn die Objekt-IDs nicht mehr
stimmen, muss man die Bearbeitung in DAVID wiederholen.
7. Es kann sehr leicht zu Unterschieden zwischen Excel-Liste / Vermessungsriss und DAVID-
Bearbeitung kommen. Jede Änderung wird mindestens zweimal digitalisiert (in DAVID und in
GeoGraf). Derzeit ist die einzige Kontrolle ein anderer Mitarbeiter (Vier-Augen-Prinzip), was
Zeit kostet.
8. Bearbeitete Flurstücke müssen nach Eigentümer gruppiert werden (für ein Formular, das später
in der Arbeitsmappe landet). Derzeit wird das von Hand in Excel mit Kopieren / Einfügen
erledigt.

Das Programm „TNViewer“ ist eine HTML-Anwendung (technische Details siehe Abschnitt 2), welches
diesen Problemen zuvorkommen soll: Je früher ein Fehler gefunden wird, desto weniger Zeit und
Gehirn kostet es später, diesen zu korrigieren.

### Arbeitsablauf

Im Programm kann man (über „Projekt aus CSV“ und „NAS-Daten importieren“) die CSV-Daten aus
LiKA online und die NAS-Daten aus dem GeoBroker importieren:



Auf der linken Seite sieht man die Flurstücke, zu denen man in der Karte per Doppelklick navigieren
kann. Danach kann man (über „Nutzung einzeichnen“) neue Nutzungen digitalisieren oder existieren
Nutzungen direkt über die Flurstücksliste ändern:

<img width="837" alt="image" src="https://github.com/user-attachments/assets/7986904f-eb3e-434b-b746-5bcb2286209a" />

<img width="463" alt="image" src="https://github.com/user-attachments/assets/63015bfe-bf0e-49c8-99f6-8fb6378580e6" />

Die Nutzungsarten-Kürzel entstammen aus dem hinterlegten Nutzungsartenkatalog (momentan GID 7.1). 
Ebenso kann man oben rechts die Suche benutzen, falls man sich bei der Klassifizierung
einer Fläche nicht sicher ist (in der jetzigen Bearbeitungsweise muss man das PDF / Papier zu
Hand nehmen).

<img width="620" alt="image" src="https://github.com/user-attachments/assets/1d4cd1aa-f28c-49be-aff2-a6e9447ff12d" />

Ist die Digitalisierung abgeschlossen, müssen die Flächen noch bereinigt werden (Änderungen säubern
1 – 6), damit die Punkte 100% auf den Flurstücksgrenzen sitzen und nicht etwa sich leicht
überschneiden. Diese Säuberung ist in verschiedene Schritte unterteilt, damit es bei Fehlern einfacher
ist, diese einzugrenzen. Wenn die Säuberung erfolgreich abgeschlossen ist, kann man die
digitalisierten Flächen ausgeben: Einmal nach GeoGraf („Export GEOgraf“) sowie nach DAVID („Export
DAVID“). Hierfür müssen Rissgebiete (blau) über die Änderungen (grün) gelegt werden, damit schnell
geplant werden kann, wie viele Risse man für die Arbeitsmappe benötigt.

Beim Export nach GEOgraf erhält man einen ZIP-Ordner mit den Elementen, die jeder Riss benötigt:
Blattkopf, Legende (berechnet je nach sichtbaren Kürzeln) und Anschlussrisse, sowie die
bürokratischen Dokumente: Antragbegleitblatt, Fortführungsbeleg und Bearbeitungsliste (generierte
Excel-Liste, welche Flurstücke bearbeitet wurden):

<img width="1059" alt="image" src="https://github.com/user-attachments/assets/b0e391a3-d0c3-4899-96ee-42d26df5fa53" />

Ebenso erhält man eine berechnete Excel-Liste, welche Flurstücke wie geändert wurden und - da die
Wirtschaftsarten programmtechnisch für jedes Kürzel hinterlegt sind - ebenso den Beleg, ob ein
Flurstück veränderte Wirtschaftsarten hat:


Zusätzlich enthält jeder Riss-Ordner eine Vorschau, damit man Probleme schnell erkennt und nicht erst
die Linien und Texte in GEOgraf importieren muss, um Platzierungsprobleme zu sehen. Hierbei ist zu
bemerken, dass die roten Linien zwar als Flächen in der Software gespeichert werden, aber als
Linienobjekte exportiert werden, wenn sie eine Flurstücksgrenze überschneiden. Das ist wichtig zur
Überprüfung, ob die Änderungen sauber auf den Linien liegen.

Danach ist es sehr leicht, die Texte und Linien in GEOgraf zu importieren: Man muss nur über „Import -
GeoGraf“ die „Projekt.GRAFBAT.out“ Datei (siehe Abbildung Seite 3) in GeoGraf laden, damit werden
die Texte, Linie und Punkte automatisch in GeoGraf übernommen. Ebenso hat jeder Riss eine eigene
„Menge“, damit man Änderungen nicht doppelt in verschiedenen Rissen darstellt und die Elemente
eines Risses unabhängig von anderen Änderungen bearbeiten kann.

Nach dem Import und leichter Korrektur der Textpositionen nach GeoGraf sieht dann der Riss ca. so aus:

Man beachte, dass die meisten Texte nicht mehr von Hand bearbeitet werden müssen, die
Textposition, Platzierung und Bezugspfeile werden vollautmatisch gesetzt!

Da die Textplatzierung automatisch geschieht, sind ebenso alle notwendigen Flächen richtig
beschriftet. Alles, was zum manuellen Aufbereiten der Risse noch notwendig ist, ist die Elemente wie
Blattkopf, Legende und Anschlussrisse zu platzieren und über GeoGraf das PDF zu generieren.

Die importierten Objekte (Texte und Linien) stellen somit sicher, dass jede Fläche richtig beschriftet ist.
Die Textplatzierung ist allerdings noch nicht perfekt, daher muss man manuell möglicherweise noch
die Texte etwas verschieben. Dass man das nicht mehr tun muss, ist ein Ziel in der zukünftigen
Entwicklung des Programms, allerdings war das erste Ziel die Korrektheit.
Wenn die thematischen Änderungen bestätigt wurden, kann man die Änderungen in eine XML-Datei
ausgeben („Export DAVID“), die man in der DAVID EQK vor dem Schritt „Bearbeitung“ einlesen kann
(„NAS-Import“). Beim Einlesen in DAVID bildet DAVID die nötigen Objekte. Somit erspart man sich
eine doppelte Digitalisierung der Flächen und ist gegen Datenverlust bei Programmabstürzen
gewappnet.

<img width="913" alt="image" src="https://github.com/user-attachments/assets/f115d520-5fa9-4781-be9c-88b5d3107d32" />

Ebenso stellt diese Arbeitsweise sicher, dass alle Änderungen im Riss in DAVID eingezeichnet werden,
sodass eine weitere Überprüfung durch einen Mitarbeiter weniger Zeit kostet bzw. überflüssig ist.
Ebenso müssen die Produkte (Änderungsmitteilungen) nur noch oberflächlich überprüft werden, da
die Änderungen der Wirtschaftsarten bereits vom Programm errechnet wurde.
Abgesehen von den vermiedenen Fehlerquellen hat das Programm eine massive Zeitersparnis, welche
je nach Projektgröße (bei einer regulären Gemarkung mit ca. 500 – 1000 Flurstücken) ca. 60 – 70%
bedeutet:

- Doppelte Digitalisierung in DAVID / GEOgraf ist überflüssig
- Änderungen müssen nicht mehr von Hand dokumentiert werden
- Navigation zu Flurstücken ist wesentlich einfacher als in GEOgraf
- Legenden, Blattköpfe und Anschlussrisse müssen nicht von Hand erstellt werden

Das höchste Risiko bei der neuen Arbeitsweise ist eine falsche Verschneidung der Teilflächen. Hier ist
äußerste Vorsicht geboten, da die Korrektheit des gesamten Systems daran hängt, ob die Teilflächen
richtig verschnitten werden. Allerdings würde diese falsche Verschneidung spätestens im
Vermessungsriss auffallen.

### Potentielle technische Risiken

Technisch ist die Anwendung nichts Anderes als eine lokale HTML-Seite, daher ist die Anwendung
auch ohne Internet verfügbar, da kein Server benötigt wird. Systemvoraussetzung ist lediglich ein
moderner Webbrowser (Firefox, Edge, Chrome).

Die Anwendung verwendet viele externe Bibliotheken, welche üblicherweise jede einzeln auf Sicherheit
überprüft werden müssten. Allerdings verwendet die HTML-Seite einen Trick: der Hauptteil des Codes
ist mit WebAssembly (WASM) umgesetzt. WebAssembly ist ein Bytecode-Format, d.h. der Quellcode
liegt nicht menschlich lesbar vor (der Code wird zunächst von der Quellsprache C++, oder in
diesem Fall: Rust zu WASM kompiliert). WASM hat einen entscheidenden Vorteil: es wird in einer
Sandbox im Browser ausgeführt und hat keine Zugriff auf das Netzwerk, Dateisystem oder die Uhr des
Computers. Insofern können aus WASM heraus keine Daten versendet werden.

Der JavaScript-Teil der Anwendung besteht zum größten Teil nur aus Aufrufen von WASM-Funktionen,
Hilfsfunktionen sowie Interaktionen für die Benutzeroberfläche. Somit benötigt die Anwendung keinen
Server, sondern kann komplett lokal laufen und auch einfach kopiert / editiert / ausgetauscht werden.
Die einzigen externen JavaScript-Bibliotheken sind:

- Leaflet.js (https://leafletjs.com/): quelloffene JS-Bibliothek für interaktive Karten / Web-GIS Systeme)
- Leaflet.draw (ebenso von den LeafLet-Entwicklern, fügt die Fähigkeit zum Einzeichnen neuer Polygone hinzu)
- Leaflet.snap (https://github.com/makinacorpus/Leaflet.Snap - entwickelt von der französischen
GIS-Firma Makina-Corpus, https://makina-corpus.com/): Notwendig, damit beim Digitalisieren
Punkte direkt auf Linien und Punkt der existierenden Flurstücke einrasten („snapping“).

Die Chance, dass diese JS-Bibliotheken Schadsoftware enthalten, ist sehr gering. Alle Bibliotheken sind
gebündelt und werden nicht automatisch aus dem Internet bezogen, sondern Updates müssten
manuell geschehen.

Software-Updates im klassischen Sinne gibt es nicht, da man einfach eine neue Version der HTML-
Seite im Browser öffnen kann. Die Distribution der Software erfolgt entweder durch einen Link oder
durch einen internen Austauschserver. Ebenso könnte die Sicherheit erhöht werden, wenn das Tool
nicht im Internet zu finden ist, d.h. wenn niemand weiß, dass das Amt dieses interne Tool verwendet,
kann es also auch nicht angegriffen werden.

### Potentielle organisatorische Risiken

Abgesehen von den technischen Risiken gibt es das organisatorische Risiko, dass z.B. der
Softwareentwickler keine Lust mehr hat oder verunglückt oder den Job wechselt. In diesem Fall sind
aber immernoch die GEOgraf und DAVID-Projekte vorhanden, da sich das Programm nicht als Ersatz
für GEOgraf und DAVID, sondern als Ergänzung sieht.

Organisatorisch ändert sich nichts an den Produkten der Bearbeitung der tatsächlichen Nutzung, es
entsteht lediglich eine zusätzlichen „Auftragsnummer.json“ Datei, die die Änderungen des
Bearbeiters zusätzlich speichert. Hierbei werden auch sensitive Eigentümerdaten in der Datei
gespeichert, daher ist ein Versenden dieser JSON-Dateien nicht ratsam:
