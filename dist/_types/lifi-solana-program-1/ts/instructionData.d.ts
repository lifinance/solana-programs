import { Buffer } from "buffer";
export type InstructionData = {
  TrackV1: {
    transaction_id: Uint8Array;
  };
};
export declare function serializeInstructionData(data: InstructionData): Buffer;
export declare function deserializeBinaryInstructionData(
  binaryInstructionData: Uint8Array,
): InstructionData;
export declare function deserializeBase58InstructionData(
  base58InstructionData: string,
): InstructionData;
//# sourceMappingURL=instructionData.d.ts.map
