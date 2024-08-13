import {PublicKey, SYSVAR_CLOCK_PUBKEY, TransactionInstruction} from "@solana/web3.js";
import {serializeTrackingInstructionData} from "./trackingInstructionData";

export async function buildTrackV1Instruction(
    programId: PublicKey,
    transaction_id: Uint8Array,
): Promise<TransactionInstruction> {
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
        data:  serializeTrackingInstructionData({
            TrackV1: {
                transaction_id
            }
        })
    });
}