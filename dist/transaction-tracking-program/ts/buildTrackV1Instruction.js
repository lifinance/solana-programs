var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
import { SYSVAR_CLOCK_PUBKEY, TransactionInstruction } from "@solana/web3.js";
import { serializeTrackingInstructionData } from "./trackingInstructionData";
export function buildTrackV1Instruction(programId, transaction_id) {
    return __awaiter(this, void 0, void 0, function* () {
        if (transaction_id.length !== 9)
            throw new Error('Invalid transaction_id length (' + transaction_id.length + ' bytes)');
        return new TransactionInstruction({
            keys: [
                {
                    pubkey: SYSVAR_CLOCK_PUBKEY,
                    isSigner: false,
                    isWritable: false,
                }
            ],
            programId: programId,
            data: serializeTrackingInstructionData({
                TrackV1: {
                    transaction_id
                }
            })
        });
    });
}