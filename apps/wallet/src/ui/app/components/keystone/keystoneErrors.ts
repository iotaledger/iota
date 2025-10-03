// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export class KeystoneSigningCanceledByUserError extends Error {
    constructor(message: string) {
        super(message);
        Object.setPrototypeOf(this, KeystoneSigningCanceledByUserError.prototype);
    }
}
