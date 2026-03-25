// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { CallArg, ObjectArg } from '../../bcs/types.js';

/**
 * @experimental
 * Call arg for specifying how to provide call arguments before resolution.
 */
export type MoveAuthenticatorCallArg =
    | { ImmutableOrOwned: string } // Object ID
    | {
          Shared: {
              objectId: string;
              mutable: boolean;
          };
      }
    | { Pure: Uint8Array };

/**
 * @experimental
 * The resolved MoveAuthenticator data structure, versioned as a discriminated
 * union. Add new variants here when new versions are introduced on the Rust side.
 */
export type MoveAuthenticatorData = MoveAuthenticatorDataV1; // future: | MoveAuthenticatorDataV2;

/**
 * @experimental
 */
export interface MoveAuthenticatorDataV1 {
    version: 'V1';
    callArgs: CallArg[];
    typeArgs: string[];
    objectToAuthenticate: ObjectArg;
}
