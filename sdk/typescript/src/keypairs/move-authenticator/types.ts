// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { CallArg, ObjectArg } from '../../bcs/types.js';

/**
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
 * The resolved MoveAuthenticator data structure.
 * Fields match the Rust MoveAuthenticator struct.
 */
export interface MoveAuthenticatorData {
    callArgs: CallArg[];
    typeArgs: string[];
    objectToAuthenticate: ObjectArg;
}
