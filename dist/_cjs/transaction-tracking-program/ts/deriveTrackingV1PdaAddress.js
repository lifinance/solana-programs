"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.deriveTrackingV1PdaAddress = deriveTrackingV1PdaAddress;
const web3_js_1 = require("@solana/web3.js");
const borsher_1 = require("borsher");
function deriveTrackingV1PdaAddress(programId, epoch) {
    return web3_js_1.PublicKey.findProgramAddressSync([
        (0, borsher_1.borshSerialize)(borsher_1.BorshSchema.u64, epoch)
    ], programId)[0];
}
