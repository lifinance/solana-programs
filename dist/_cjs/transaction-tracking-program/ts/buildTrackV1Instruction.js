"use strict";
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.buildTrackV1Instruction = buildTrackV1Instruction;
const web3_js_1 = require("@solana/web3.js");
const trackingInstructionData_1 = require("./trackingInstructionData");
const deriveTrackingV1PdaAddress_1 = require("./deriveTrackingV1PdaAddress");
function buildTrackV1Instruction(programId, transaction_id, epoch) {
    return __awaiter(this, void 0, void 0, function* () {
        if (transaction_id.length !== 8)
            throw new Error('Invalid transaction_id length (' + transaction_id.length + ' bytes)');
        const epoch_track_account = (0, deriveTrackingV1PdaAddress_1.deriveTrackingV1PdaAddress)(programId, epoch);
        const instruction_data = (0, trackingInstructionData_1.serializeTrackingInstructionData)({
            TrackV1: {
                transaction_id
            }
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
                }
            ],
            programId: programId,
            data: instruction_data
        });
    });
}
