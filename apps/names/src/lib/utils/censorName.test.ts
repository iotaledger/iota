// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from 'vitest';

import { censorName } from './censorName';

describe('censorName', () => {
    it('should return unchanged name when no forbidden words are found', () => {
        const result = censorName('hello', ['bad', 'worse']);
        expect(result).toBe('hello');
    });

    it('should censor a single forbidden word', () => {
        const result = censorName('hello-bad-world', ['bad']);
        expect(result).toBe('hello-***-world');
    });

    it('should censor multiple forbidden words', () => {
        const result = censorName('bad-hello-worse', ['bad', 'worse']);
        expect(result).toBe('***-hello-*****');
    });

    it('should censor the same word multiple times', () => {
        const result = censorName('bad-hello-bad', ['bad']);
        expect(result).toBe('***-hello-***');
    });

    it('should be case insensitive', () => {
        const result = censorName('hello-BAD-world', ['bad']);
        expect(result).toBe('hello-***-world');
    });

    it('should censor adjacent matches', () => {
        const result = censorName('badword', ['bad', 'word']);
        expect(result).toBe('*******');
    });

    it('should handle empty string', () => {
        const result = censorName('', ['bad']);
        expect(result).toBe('');
    });

    it('should handle empty forbidden words list', () => {
        const result = censorName('hello', []);
        expect(result).toBe('hello');
    });

    it('should censor partial matches within words', () => {
        const result = censorName('badger', ['bad']);
        expect(result).toBe('***ger');
    });
});
