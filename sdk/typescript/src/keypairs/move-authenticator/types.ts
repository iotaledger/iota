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
 * Input kind for specifying how to provide call arguments before resolution.
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
 * A resolved CallArg matching the Rust CallArg enum:
 *   Pure(Vec<u8>)
 *   Object(ObjectArg)
 *
 * Where ObjectArg is:
 *   ImmOrOwnedObject(ObjectRef)
 *   SharedObject { id, initial_shared_version, mutable }
 *   Receiving(ObjectRef)
 */
export type ResolvedCallArg =
    | {
          Pure: {
              bytes: Uint8Array | string;
          };
      }
    | {
          Object: ResolvedObjectArg;
      };

export type ResolvedObjectArg =
    | {
          ImmOrOwnedObject: ObjectRef;
      }
    | {
          SharedObject: {
              objectId: string;
              initialSharedVersion: string | number;
              mutable: boolean;
          };
      }
    | {
          Receiving: ObjectRef;
      };

/**
 * The resolved MoveAuthenticator data structure.
 */
export interface MoveAuthenticatorData {
    callArgs: ResolvedCallArg[];
    typeArgs: string[];
    objectToAuthenticate: ResolvedCallArg;
}
