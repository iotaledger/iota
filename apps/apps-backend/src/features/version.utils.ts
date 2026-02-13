// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { coerce, gte } from 'semver';

/**
 * Check if `version` is greater than or equal to `minVersion`.
 * Coerces partial versions (e.g. "1" -> "1.0.0", "1.2" -> "1.2.0").
 * Returns false if either string is not a valid semver.
 */
export function semverGte(version: string, minVersion: string): boolean {
    const coercedVersion = coerce(version);
    const coercedMin = coerce(minVersion);

    if (!coercedVersion || !coercedMin) return false;

    return gte(coercedVersion, coercedMin);
}

/**
 * Validate that a string can be interpreted as a semver version.
 */
export function isValidSemver(version: string): boolean {
    return coerce(version) !== null;
}
