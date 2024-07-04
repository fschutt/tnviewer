import init, {load_nas_xml} from "../pkg/tnviewer.js";

var projectdata = null;

function replaceDataNasXML(event) {
    var input = document.createElement('input');
    input.type = 'file';
    input.accept = ".xml";

    input.onchange = e => { 
        var file = e.target.files[0]; 
        var reader = new FileReader();
        reader.readAsText(file,'UTF-8');
        reader.onload = readerEvent => {
           var content = readerEvent.target.result;
           var array = new Uint8Array(content);
           init()
           .then(() => { 
                var converted = load_nas_xml(array);
                projectdata = JSON.parse(converted);
                console.log(projectdata);
           })
        }
    }

    input.click();
}