import { Buffer } from "buffer";
import { borshDeserialize, BorshSchema, borshSerialize } from "borsher";
import bs58 from "bs58";

export type InstructionData = {
  TrackV1: {
    transaction_id: Uint8Array;
  };
};
const Schema: BorshSchema = BorshSchema.Enum({
  TrackV1: BorshSchema.Struct({
    transaction_id: BorshSchema.Array(BorshSchema.u8, 8),
  }),
});

export function serializeInstructionData(data: InstructionData): Buffer {
  return Buffer.from(borshSerialize(Schema, data));
}

export function deserializeBinaryInstructionData(
  binaryInstructionData: Uint8Array,
): InstructionData {
  return borshDeserialize<InstructionData>(Schema, binaryInstructionData);
}

export function deserializeBase58InstructionData(
  base58InstructionData: string,
): InstructionData {
  return deserializeBinaryInstructionData(bs58.decode(base58InstructionData));
}
