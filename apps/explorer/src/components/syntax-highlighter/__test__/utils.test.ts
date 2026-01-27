// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, it, expect } from 'vitest';
import { addQuotesToIncompleteLines, detectsUnclosedQuotes } from '../utils';

describe('sanitize syntax highlighter', () => {
    describe('detectsUnclosedQuotes', () => {
        it('no quotes -> false', () => {
            expect(detectsUnclosedQuotes('0: Call foo(bar)')).toBe(false);
        });

        it('balanced quotes -> false', () => {
            expect(detectsUnclosedQuotes('vector<u8>: "http://"')).toBe(false);
        });

        it('unclosed quote -> true', () => {
            expect(detectsUnclosedQuotes('vector<u8>: "htt..)')).toBe(true);
        });

        it('escaped quotes -> false', () => {
            expect(detectsUnclosedQuotes(String.raw`vector<u8>: "he said \"hi\""`)).toBe(false);
        });
    });

    describe('addQuotesToIncompleteLines', () => {
        it('no unclosed quotes -> unchanged', () => {
            const input = `0: Call foo()\n1: Call bar()`;
            expect(addQuotesToIncompleteLines(input)).toBe(input);
        });

        it('unclosed quote + ) suffix -> insert before suffix', () => {
            const input = `0: LdConst[1](vector<u8>: "htt..)\n1: Call ascii::string(vector<u8>)`;
            const output = `0: LdConst[1](vector<u8>: "htt..")\n1: Call ascii::string(vector<u8>)`;
            expect(addQuotesToIncompleteLines(input)).toBe(output);
        });

        it('unclosed quote + ], suffix -> insert before suffix', () => {
            const input = `0: X["abc..],\n1: next`;
            const output = `0: X["abc.."],\n1: next`;
            expect(addQuotesToIncompleteLines(input)).toBe(output);
        });

        it('unclosed quote + no suffix -> append quote', () => {
            const input = `0: vector<u8>: "abc..\n1: next`;
            const output = `0: vector<u8>: "abc.."\n1: next`;
            expect(addQuotesToIncompleteLines(input)).toBe(output);
        });

        it('unclosed quote in line 0 -> line 1 unchanged', () => {
            const input = `0: vector<u8>: "abc..\n1: "ok"\n2: done`;
            const output = `0: vector<u8>: "abc.."\n1: "ok"\n2: done`;
            expect(addQuotesToIncompleteLines(input)).toBe(output);
        });
    });
});
