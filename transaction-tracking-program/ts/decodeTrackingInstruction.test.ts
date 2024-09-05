import {decodeTrackingInstruction} from "./decodeTrackingInstruction";
import {test, expect} from "@jest/globals";

test('decodeTrackingInstruction', async () => {
    const trackingInstruction = await decodeTrackingInstruction({
        accounts: [],
        programIdIndex: 0,
        data: '1An6UebxCZV',
    });

    expect(trackingInstruction).toEqual({
        TrackV1: {transaction_id: Array.from([1, 2, 3, 4, 5, 6, 7, 0])}
    })
})