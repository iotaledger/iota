// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaMoveObject, IotaObjectData } from '@iota/iota-sdk/client';

export const COIN_TYPE = '0x2::coin::Coin';
const COIN_TYPE_ARG_REGEX = /^0x2::coin::Coin<(.+)>$/;

export const IOTA_BIP44_COIN_TYPE = 4218;

// TODO use sdk
export class Coin {
    public static isCoin(obj: IotaObjectData) {
        const type = obj?.content?.dataType === 'package' ? 'package' : obj?.content?.type;
        return type?.startsWith(COIN_TYPE) ?? false;
    }

    public static getCoinTypeArg(obj: IotaMoveObject) {
        const res = obj.type.match(COIN_TYPE_ARG_REGEX);
        return res ? res[1] : null;
    }

    public static isIOTA(obj: IotaMoveObject) {
        const arg = Coin.getCoinTypeArg(obj);
        return arg ? Coin.getCoinSymbol(arg) === 'IOTA' : false;
    }

    public static getCoinSymbol(coinTypeArg: string) {
        return coinTypeArg.substring(coinTypeArg.lastIndexOf(':') + 1);
    }

    public static getBalance(obj: IotaMoveObject): bigint {
        return BigInt((obj.fields as { balance: string }).balance);
    }

    public static getID(obj: IotaMoveObject): string {
        return (obj.fields as { id: { id: string } }).id.id;
    }

    public static getCoinTypeFromArg(coinTypeArg: string) {
        return `${COIN_TYPE}<${coinTypeArg}>`;
    }
}
