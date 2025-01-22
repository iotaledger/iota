// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type Serializable } from '_shared/cryptography/keystore';
import {
    getEncryptedFromSessionStorage,
    removeFromSessionStorage,
    setToSessionStorageEncrypted,
} from '_src/background/storageUtils';

export function getEphemeralValue<T extends Serializable>(id: string) {
    return getEncryptedFromSessionStorage<T>(id);
}

export function setEphemeralValue<T extends Serializable>(id: string, data: T) {
    return setToSessionStorageEncrypted(id, data);
}

export function clearEphemeralValue(id: string) {
    return removeFromSessionStorage(id);
}
