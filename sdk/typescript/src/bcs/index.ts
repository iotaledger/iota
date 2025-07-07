// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { bcs } from '@iota/bcs';

import {
    Address,
    AppId,
    Argument,
    CallArg,
    Command,
    CompressedSignature,
    GasData,
    Intent,
    IntentMessage,
    IntentScope,
    IntentVersion,
    MultiSig,
    MultiSigPkMap,
    MultiSigPublicKey,
    ObjectArg,
    ObjectDigest,
    Owner,
    ProgrammableMoveCall,
    ProgrammableTransaction,
    PublicKey,
    SenderSignedData,
    SenderSignedTransaction,
    SharedObjectRef,
    StructTag,
    IotaObjectRef,
    TransactionData,
    TransactionDataV1,
    TransactionExpiration,
    TransactionKind,
    TypeTag,
    PasskeyAuthenticator,
} from './bcs.js';
import { TransactionEffects } from './effects.js';

export type { TypeTag } from './types.js';

export { TypeTagSerializer } from './type-tag-serializer.js';
export { BcsType, type BcsTypeOptions } from '@iota/bcs';

const iotaBcs = {
    ...bcs,
    U8: bcs.u8(),
    U16: bcs.u16(),
    U32: bcs.u32(),
    U64: bcs.u64(),
    U128: bcs.u128(),
    U256: bcs.u256(),
    ULEB128: bcs.uleb128(),
    Bool: bcs.bool(),
    String: bcs.string(),
    Address,
    AppId,
    Argument,
    CallArg,
    CompressedSignature,
    GasData,
    Intent,
    IntentMessage,
    IntentScope,
    IntentVersion,
    MultiSig,
    MultiSigPkMap,
    MultiSigPublicKey,
    ObjectArg,
    ObjectDigest,
    Owner,
    ProgrammableMoveCall,
    ProgrammableTransaction,
    PublicKey,
    SenderSignedData,
    SenderSignedTransaction,
    SharedObjectRef,
    StructTag,
    IotaObjectRef,
    Command,
    TransactionData,
    TransactionDataV1,
    TransactionExpiration,
    TransactionKind,
    TypeTag,
    TransactionEffects,
    PasskeyAuthenticator,
};

export { iotaBcs as bcs };
