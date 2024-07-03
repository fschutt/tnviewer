
const downloadURL = (data, fileName) => {
    const a = document.createElement('a')
    a.href = data
    a.download = fileName
    document.body.appendChild(a)
    a.style.display = 'none'
    a.click()
    a.remove()
}

const downloadBlob = (data, fileName, mimeType) => {

    const blob = new Blob([data], {
        type: mimeType
    })

    const url = window.URL.createObjectURL(blob)

    downloadURL(url, fileName)

    setTimeout(() => window.URL.revokeObjectURL(url), 1000)
}

/*
document.querySelector('input').addEventListener('change', function() {

    var reader = new FileReader();
    reader.onload = function() {
        var arrayBuffer = this.result;
        var array = new Uint8Array(arrayBuffer);
        var xlsx = xml_to_xlsx(array, document.getElementById("opt1").checked);
        downloadBlob(xlsx, "Konvertiert.xlsx", 'application/octet-stream');
    }
    
    reader.readAsArrayBuffer(this.files[0]);

}, false);
*/