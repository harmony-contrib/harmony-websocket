const crypto = require('crypto');
const magic = '258EAFA5-E914-47DA-95CA-C5AB0DC85B11';
const secWebSocketKey = 'QV2loWLUgcr4EryR7vxX4g==';

let secWebSocketAccept = crypto.createHash('sha1')
	.update(secWebSocketKey + magic)
	.digest('base64');

console.log(secWebSocketAccept);