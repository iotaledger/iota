// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { compareSemver, semverGte, isValidSemver } from './version.utils';

describe('version.utils', () => {
    describe('compareSemver', () => {
        it('should return 0 for equal versions', () => {
            expect(compareSemver('1.0.0', '1.0.0')).toBe(0);
            expect(compareSemver('0.0.0', '0.0.0')).toBe(0);
            expect(compareSemver('10.20.30', '10.20.30')).toBe(0);
        });

        it('should return 1 when a > b', () => {
            expect(compareSemver('2.0.0', '1.0.0')).toBe(1);
            expect(compareSemver('1.1.0', '1.0.0')).toBe(1);
            expect(compareSemver('1.0.1', '1.0.0')).toBe(1);
            expect(compareSemver('1.2.0', '1.1.9')).toBe(1);
        });

        it('should return -1 when a < b', () => {
            expect(compareSemver('1.0.0', '2.0.0')).toBe(-1);
            expect(compareSemver('1.0.0', '1.1.0')).toBe(-1);
            expect(compareSemver('1.0.0', '1.0.1')).toBe(-1);
            expect(compareSemver('1.1.9', '1.2.0')).toBe(-1);
        });

        it('should handle versions with only major', () => {
            expect(compareSemver('2', '1')).toBe(1);
            expect(compareSemver('1', '2')).toBe(-1);
            expect(compareSemver('1', '1')).toBe(0);
        });

        it('should handle versions with major.minor', () => {
            expect(compareSemver('1.2', '1.1')).toBe(1);
            expect(compareSemver('1.1', '1.2')).toBe(-1);
            expect(compareSemver('1.1', '1.1')).toBe(0);
        });

        it('should treat missing parts as 0', () => {
            expect(compareSemver('1', '1.0.0')).toBe(0);
            expect(compareSemver('1.0', '1.0.0')).toBe(0);
            expect(compareSemver('1.0.0', '1')).toBe(0);
        });

        it('should return null for invalid versions', () => {
            expect(compareSemver('abc', '1.0.0')).toBeNull();
            expect(compareSemver('1.0.0', 'abc')).toBeNull();
            expect(compareSemver('', '1.0.0')).toBeNull();
            expect(compareSemver('1.0.0', '')).toBeNull();
            expect(compareSemver('1.0.0-beta', '1.0.0')).toBeNull();
            expect(compareSemver('v1.0.0', '1.0.0')).toBeNull();
            expect(compareSemver('1.0.0.0', '1.0.0')).toBeNull();
        });
    });

    describe('semverGte', () => {
        it('should return true when version equals minVersion', () => {
            expect(semverGte('1.5.0', '1.5.0')).toBe(true);
        });

        it('should return true when version is greater than minVersion', () => {
            expect(semverGte('2.0.0', '1.5.0')).toBe(true);
            expect(semverGte('1.6.0', '1.5.0')).toBe(true);
            expect(semverGte('1.5.1', '1.5.0')).toBe(true);
        });

        it('should return false when version is less than minVersion', () => {
            expect(semverGte('1.4.0', '1.5.0')).toBe(false);
            expect(semverGte('1.4.9', '1.5.0')).toBe(false);
            expect(semverGte('0.9.9', '1.0.0')).toBe(false);
        });

        it('should return false for invalid versions', () => {
            expect(semverGte('invalid', '1.0.0')).toBe(false);
            expect(semverGte('1.0.0', 'invalid')).toBe(false);
        });
    });

    describe('isValidSemver', () => {
        it('should return true for valid semver strings', () => {
            expect(isValidSemver('1.0.0')).toBe(true);
            expect(isValidSemver('0.1.0')).toBe(true);
            expect(isValidSemver('10.20.30')).toBe(true);
            expect(isValidSemver('1')).toBe(true);
            expect(isValidSemver('1.2')).toBe(true);
        });

        it('should return false for invalid semver strings', () => {
            expect(isValidSemver('')).toBe(false);
            expect(isValidSemver('abc')).toBe(false);
            expect(isValidSemver('v1.0.0')).toBe(false);
            expect(isValidSemver('1.0.0-beta')).toBe(false);
            expect(isValidSemver('1.0.0.0')).toBe(false);
            expect(isValidSemver(' 1.0.0')).toBe(false);
            expect(isValidSemver('1.0.0 ')).toBe(false);
        });
    });
});
