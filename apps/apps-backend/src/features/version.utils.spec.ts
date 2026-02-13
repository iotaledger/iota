// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { semverGte, isValidSemver } from './version.utils';

describe('version.utils', () => {
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

        it('should coerce partial versions', () => {
            expect(semverGte('1', '1.0.0')).toBe(true);
            expect(semverGte('1.0', '1.0.0')).toBe(true);
            expect(semverGte('2', '1.5.0')).toBe(true);
            expect(semverGte('1', '1.0.1')).toBe(false);
        });

        it('should handle prefixed and prerelease versions via coercion', () => {
            expect(semverGte('v1.5.0', '1.5.0')).toBe(true);
            expect(semverGte('1.5.0-beta', '1.5.0')).toBe(true);
        });
    });

    describe('isValidSemver', () => {
        it('should return true for valid semver strings', () => {
            expect(isValidSemver('1.0.0')).toBe(true);
            expect(isValidSemver('0.1.0')).toBe(true);
            expect(isValidSemver('10.20.30')).toBe(true);
            expect(isValidSemver('1')).toBe(true);
            expect(isValidSemver('1.2')).toBe(true);
            expect(isValidSemver('v1.0.0')).toBe(true);
            expect(isValidSemver('1.0.0-beta')).toBe(true);
        });

        it('should return false for invalid semver strings', () => {
            expect(isValidSemver('')).toBe(false);
            expect(isValidSemver('abc')).toBe(false);
        });
    });
});
