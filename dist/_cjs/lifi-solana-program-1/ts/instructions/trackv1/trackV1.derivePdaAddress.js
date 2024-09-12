"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.derivePdaAddress = derivePdaAddress;
const web3_js_1 = require("@solana/web3.js");
const borsher_1 = require("borsher");
function derivePdaAddress(programId, epoch) {
    return web3_js_1.PublicKey.findProgramAddressSync([(0, borsher_1.borshSerialize)(borsher_1.BorshSchema.u64, epoch)], programId)[0];
}
