export * as TrackV1 from "./ts/instructions/trackv1";
export * from "./ts/instructionData";

export type InstructionData = {
  TrackV1: {
    transaction_id: Uint8Array;
  };
};
