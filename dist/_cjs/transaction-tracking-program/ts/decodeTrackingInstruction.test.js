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
const decodeTrackingInstruction_1 = require("./decodeTrackingInstruction");
const globals_1 = require("@jest/globals");
(0, globals_1.test)('decodeTrackingInstruction', () => __awaiter(void 0, void 0, void 0, function* () {
    const trackingInstruction = yield (0, decodeTrackingInstruction_1.decodeTrackingInstruction)('1An6UebxCZV');
    (0, globals_1.expect)(trackingInstruction).toEqual({
        TrackV1: { transaction_id: Array.from([1, 2, 3, 4, 5, 6, 7, 0]) }
    });
}));
