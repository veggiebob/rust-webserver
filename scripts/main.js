var sendMessage = function(message) {
    var xmlHttp = new XMLHttpRequest(); //returns a XMLHttpRequest object
    var mimeType = "text/plain";
    xmlHttp.open('PUT', '/', true);
    xmlHttp.setRequestHeader('Content-Type', mimeType);
    xmlHttp.setRequestHeader('Content-Length', message.length);
    xmlHttp.send(message);
}