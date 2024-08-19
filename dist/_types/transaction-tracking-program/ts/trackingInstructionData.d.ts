import { Buffer } from "buffer";
export type TrackingInstructionData = {
    TrackV1: {
        transaction_id: Uint8Array;
    };
};
export declare function serializeTrackingInstructionData(data: TrackingInstructionData): Buffer;
//# sourceMappingURL=trackingInstructionData.d.ts.map