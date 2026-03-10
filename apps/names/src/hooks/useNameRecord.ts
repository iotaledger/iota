// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { NameRecord, validateIotaName } from '@iota/iota-names-sdk';
import { useQuery } from '@tanstack/react-query';

import { useIotaNamesClient } from '@/contexts';

import { queryKey } from './queryKey';

type PriceOption = {
    years: number;
    isRegistration: boolean;
};

type UseNameRecordOptions = {
    price?: PriceOption;
};

export type NameRecordData =
    | {
          type: 'unavailable';
          nameRecord: NameRecord;
      }
    | {
          type: 'available';
          price: number;
      }
    | {
          // Not priced because of being too short
          type: 'not-priced';
      };

const DEFAULT_PRICE_OPTION = {
    years: 1,
    isRegistration: true,
};

export function useNameRecord(
    name: string,
    { price = DEFAULT_PRICE_OPTION }: UseNameRecordOptions = {},
) {
    const { iotaNamesClient } = useIotaNamesClient();

    return useQuery({
        queryKey: [...queryKey.nameRecord(name), price],
        async queryFn() {
            const validationError = validateIotaName(name);
            if (validationError) {
                throw new Error(validationError);
            }

            const nameRecord = await iotaNamesClient.getNameRecord(name);
            let nameRecordPrice = null;

            if (!nameRecord) {
                nameRecordPrice = await iotaNamesClient.calculatePrice({
                    name,
                    years: price.years,
                    isRegistration: price.isRegistration,
                });
            }

            return {
                nameRecord,
                price: nameRecordPrice,
            };
        },
        select(data) {
            if (data.nameRecord) {
                return {
                    type: 'unavailable',
                    ...data,
                } as NameRecordData;
            } else if (data.price) {
                return {
                    type: 'available',
                    ...data,
                } as NameRecordData;
            } else {
                return {
                    type: 'not-priced',
                } as NameRecordData;
            }
        },
        enabled: !!iotaNamesClient && name.length > 0,
    });
}
