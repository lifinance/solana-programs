"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.buildInstruction = buildInstruction;
const web3_js_1 = require("@solana/web3.js");
const instructionData_1 = require("../../instructionData");
const trackV1_derivePdaAddress_1 = require("./trackV1.derivePdaAddress");
function buildInstruction(programId, transactionId, epoch) {
    if (transactionId.length !== 8)
        throw new Error("Invalid transaction_id length (" + transactionId.length + " bytes)");
    const epoch_track_account = (0, trackV1_derivePdaAddress_1.derivePdaAddress)(programId, epoch);
    const instruction_data = (0, instructionData_1.serializeInstructionData)({
        TrackV1: {
            transaction_id: transactionId,
        },
    });
    return new web3_js_1.TransactionInstruction({
        keys: [
            {
                pubkey: web3_js_1.SYSVAR_CLOCK_PUBKEY,
                isSigner: false,
                isWritable: false,
            },
            {
                pubkey: epoch_track_account,
                isSigner: false,
                isWritable: false,
            },
        ],
        programId: programId,
        data: instruction_data,
    });
}
