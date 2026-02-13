// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { coerce, gte } from 'semver';

export function semverGte(version: string, minVersion: string): boolean {
    const coercedVersion = coerce(version);
    const coercedMin = coerce(minVersion);

    if (!coercedVersion || !coercedMin) return false;

    return gte(coercedVersion, coercedMin);
}

export function isValidSemver(version: string): boolean {
    return coerce(version) !== null;
}
