// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/**
 * Parse a semver string into its numeric components.
 * Accepts formats: "MAJOR", "MAJOR.MINOR", "MAJOR.MINOR.PATCH"
 * Returns null if the string is not a valid semver.
 */
function parseSemver(version: string): [number, number, number] | null {
    const match = version.match(/^(\d+)(?:\.(\d+))?(?:\.(\d+))?$/);
    if (!match) return null;

    const major = parseInt(match[1], 10);
    const minor = match[2] !== undefined ? parseInt(match[2], 10) : 0;
    const patch = match[3] !== undefined ? parseInt(match[3], 10) : 0;

    return [major, minor, patch];
}

/**
 * Compare two semver strings.
 * Returns:
 *  -1 if a < b
 *   0 if a == b
 *   1 if a > b
 *  null if either string is not a valid semver
 */
export function compareSemver(a: string, b: string): -1 | 0 | 1 | null {
    const parsedA = parseSemver(a);
    const parsedB = parseSemver(b);

    if (!parsedA || !parsedB) return null;

    for (let i = 0; i < 3; i++) {
        if (parsedA[i] > parsedB[i]) return 1;
        if (parsedA[i] < parsedB[i]) return -1;
    }

    return 0;
}

/**
 * Check if `version` is greater than or equal to `minVersion`.
 * Returns false if either string is not a valid semver.
 */
export function semverGte(version: string, minVersion: string): boolean {
    const result = compareSemver(version, minVersion);
    return result !== null && result >= 0;
}

/**
 * Validate that a string looks like a semver version.
 */
export function isValidSemver(version: string): boolean {
    return parseSemver(version) !== null;
}
