// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/**
 * Object reference with object ID, version, and digest.
 */
export interface ObjectRef {
    objectId: string;
    version: string | number;
    digest: string;
}

/**
 * Input kind for specifying how to provide call arguments.
 */
export type InputKind =
    | { ImmutableOrOwned: string } // Object ID
    | {
          Shared: {
              objectId: string;
              mutable: boolean;
          };
      }
    | { Pure: Uint8Array };

/**
 * Resolved input for MoveAuthenticator call arguments.
 */
export type MoveAuthenticatorInput =
    | {
          $kind: 'ImmutableOrOwned';
          ImmutableOrOwned: ObjectRef;
      }
    | {
          $kind: 'Shared';
          Shared: {
              objectId: string;
              initialSharedVersion: string | number;
              mutable: boolean;
          };
      }
    | {
          $kind: 'Pure';
          Pure: number[];
      };

/**
 * Account reference for MoveAuthenticator.
 * The account must be either immutable or shared.
 */
export type MoveAuthenticatorAccount =
    | {
          $kind: 'Immutable';
          Immutable: ObjectRef;
      }
    | {
          $kind: 'Shared';
          Shared: {
              objectId: string;
              initialSharedVersion: string | number;
          };
      };

/**
 * The resolved MoveAuthenticator data structure.
 */
export interface MoveAuthenticatorData {
    callArgs: MoveAuthenticatorInput[];
    typeArgs: string[];
    account: MoveAuthenticatorAccount;
}
