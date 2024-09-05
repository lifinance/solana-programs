var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
import { decodeTrackingInstruction } from "./decodeTrackingInstruction";
import { test, expect } from "@jest/globals";
test('decodeTrackingInstruction', () => __awaiter(void 0, void 0, void 0, function* () {
    const trackingInstruction = yield decodeTrackingInstruction({
        accounts: [],
        programIdIndex: 0,
        data: '1An6UebxCZV',
    });
    expect(trackingInstruction).toEqual({
        TrackV1: { transaction_id: Array.from([1, 2, 3, 4, 5, 6, 7, 0]) }
    });
}));
