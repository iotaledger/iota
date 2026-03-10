// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from 'vitest';

import { findForbiddenWords } from './findForbiddenWords';

describe('findForbiddenWords', () => {
    it('should return empty array when no forbidden words are found', () => {
        const result = findForbiddenWords('hello', ['bad', 'worse']);
        expect(result).toEqual([]);
    });

    it('should find a single forbidden word', () => {
        const result = findForbiddenWords('hello-bad-world', ['bad']);
        expect(result).toEqual([{ word: 'bad', start: 6, end: 9 }]);
    });

    it('should find multiple forbidden words', () => {
        const result = findForbiddenWords('bad-hello-worse', ['bad', 'worse']);
        expect(result).toEqual([
            { word: 'bad', start: 0, end: 3 },
            { word: 'worse', start: 10, end: 15 },
        ]);
    });

    it('should find the same word multiple times', () => {
        const result = findForbiddenWords('bad-hello-bad', ['bad']);
        expect(result).toEqual([
            { word: 'bad', start: 0, end: 3 },
            { word: 'bad', start: 10, end: 13 },
        ]);
    });

    it('should be case insensitive', () => {
        const result = findForbiddenWords('hello-BAD-world', ['bad']);
        expect(result).toEqual([{ word: 'BAD', start: 6, end: 9 }]);
    });

    it('should find overlapping matches', () => {
        const result = findForbiddenWords('badword', ['bad', 'word']);
        expect(result).toEqual([
            { word: 'bad', start: 0, end: 3 },
            { word: 'word', start: 3, end: 7 },
        ]);
    });

    it('should handle empty string', () => {
        const result = findForbiddenWords('', ['bad']);
        expect(result).toEqual([]);
    });

    it('should handle empty forbidden words list', () => {
        const result = findForbiddenWords('hello', []);
        expect(result).toEqual([]);
    });

    it('should find partial matches within words', () => {
        const result = findForbiddenWords('badger', ['bad']);
        expect(result).toEqual([{ word: 'bad', start: 0, end: 3 }]);
    });

    it('should handle hyphens in name', () => {
        const result = findForbiddenWords('hello-bad-world', ['bad']);
        expect(result).toEqual([{ word: 'bad', start: 6, end: 9 }]);
    });
});
