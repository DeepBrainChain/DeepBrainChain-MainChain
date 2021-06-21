"use strict";
exports.__esModule = true;
var types_1 = require("@polkadot/types");
var registry = new types_1.TypeRegistry();
var s = new types_1.Struct(registry, {
    foo: types_1.Text,
    bar: types_1.U32
}, { foo: 'bazzing', bar: 69 });
console.log(s['toHex']());
// testEncode('toHex', '0x1c62617a7a696e6745000000');
