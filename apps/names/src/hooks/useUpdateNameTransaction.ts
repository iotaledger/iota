// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { IotaNamesTransaction } from '@iota/iota-names-sdk';
import { Transaction } from '@iota/iota-sdk/transactions';
import { useQuery } from '@tanstack/react-query';

import { useIotaNamesClient } from '@/contexts';
import { getGasSummary } from '@/lib/utils/getGasSummary';

import { queryKey } from './queryKey';

interface UseUpdateNameTransactionOptions {
    address: string;
    updates: NameUpdate[];
}

export type NameUpdate =
    | {
          type: 'set-data';
          nftId: string;
          key: string;
          value: string;
          isSubname: boolean;
      }
    | {
          type: 'unset-data';
          nftId: string;
          key: string;
          isSubname: boolean;
      }
    | {
          type: 'set-target-address';
          address: string | undefined;
          isSubname: boolean;
          nftId: string;
      }
    | {
          type: 'set-public';
          name: string;
      }
    | {
          type: 'unset-public';
      }
    | {
          type: 'edit-setup';
          name: string;
          parentNftId: string;
          allowChildCreation: boolean;
          allowTimeExtension: boolean;
      }
    | {
          type: 'new-subname';
          parentNftId: string;
          subname: string;
          expirationDateParent: Date;
          allowChildCreation: boolean;
          allowTimeExtension: boolean;
      }
    | {
          type: 'delete-name';
          nft: string;
          isSubname: boolean;
      }
    | {
          type: 'renew-name';
          name: string;
          nftId: string;
          years: number;
          address?: string;
          couponCode?: string;
      }
    | {
          type: 'renew-subname';
          nftId: string;
          expirationDate: Date;
      }
    | {
          type: 'register-name';
          name: string;
          years?: number;
          price: number;
          setPublic: boolean;
          address?: string;
          couponCode?: string;
      };

export function useUpdateNameTransaction({ address, updates }: UseUpdateNameTransactionOptions) {
    const client = useIotaClient();
    const { iotaNamesClient } = useIotaNamesClient();
    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: [...queryKey.updateName(address), updates],
        queryFn: async () => {
            const tx = new Transaction();
            const iotaNamesTx = new IotaNamesTransaction(iotaNamesClient, tx);

            for (const update of updates) {
                switch (update.type) {
                    case 'set-data':
                        iotaNamesTx.setUserData({
                            nft: update.nftId,
                            key: update.key,
                            value: update.value,
                            isSubname: update.isSubname,
                        });
                        break;
                    case 'unset-data':
                        iotaNamesTx.unsetUserData({
                            nft: update.nftId,
                            key: update.key,
                            isSubname: update.isSubname,
                        });
                        break;
                    case 'set-target-address':
                        iotaNamesTx.setTargetAddress({
                            nft: update.nftId,
                            address: update.address,
                            isSubname: update.isSubname,
                        });
                        break;
                    case 'set-public':
                        iotaNamesTx.setPublic(update.name);
                        break;
                    case 'unset-public':
                        iotaNamesTx.unsetPublic();
                        break;
                    case 'edit-setup':
                        iotaNamesTx.editSetup({
                            parentNft: tx.object(update.parentNftId),
                            name: update.name,
                            allowChildCreation: update.allowChildCreation,
                            allowTimeExtension: update.allowTimeExtension,
                        });
                        break;
                    case 'new-subname':
                        const subnameNft = iotaNamesTx.createSubname({
                            parentNft: tx.object(update.parentNftId),
                            name: update.subname,
                            expirationTimestampMs: update.expirationDateParent.getTime(),
                            allowChildCreation: update.allowChildCreation,
                            allowTimeExtension: update.allowTimeExtension,
                        });
                        iotaNamesTx.transaction.transferObjects([subnameNft], address);
                        break;
                    case 'delete-name':
                        iotaNamesTx.burnExpired({
                            nft: tx.object(update.nft),
                            isSubname: update.isSubname,
                        });
                        break;
                    case 'renew-name':
                        await iotaNamesTx.renew({
                            nft: update.nftId,
                            name: update.name,
                            years: update.years,
                            coin: tx.gas,
                            address: update.address,
                            couponCodes: update.couponCode ? [update.couponCode] : [],
                        });
                        break;
                    case 'renew-subname':
                        iotaNamesTx.extendExpiration({
                            nft: update.nftId,
                            expirationTimestampMs: update.expirationDate.getTime(),
                        });
                        break;
                    case 'register-name':
                        const [coin] = iotaNamesTx.transaction.splitCoins(tx.gas, [update.price]);
                        let nft;
                        if (update.years) {
                            nft = await iotaNamesTx.registerWithYears({
                                name: update.name,
                                years: update.years,
                                coin,
                                address: update.address,
                                couponCodes: update.couponCode ? [update.couponCode] : [],
                            });
                        } else {
                            nft = await iotaNamesTx.register({
                                name: update.name,
                                coin,
                                address: update.address,
                                couponCodes: update.couponCode ? [update.couponCode] : [],
                            });
                        }
                        if (update.setPublic) {
                            iotaNamesTx.setTargetAddress({
                                nft: nft,
                                address: address,
                                isSubname: false,
                            });
                            iotaNamesTx.setPublic(update.name);
                        }
                        iotaNamesTx.transaction.transferObjects([nft, coin], address);
                        break;
                }
            }

            iotaNamesTx.transaction.setSender(address);
            const txBytes = await iotaNamesTx.transaction.build({
                client,
            });

            const txDryRun = await client.dryRunTransactionBlock({
                transactionBlock: txBytes,
            });

            if (txDryRun.effects.status.status !== 'success') {
                throw new Error(txDryRun.effects.status.error || 'Transaction dry run failed');
            }

            return {
                txBytes,
                txDryRun,
            };
        },
        enabled: !!address && !!updates.length,
        gcTime: 0,
        select: ({ txBytes, txDryRun }) => {
            return {
                transaction: Transaction.from(txBytes),
                gasSummary: getGasSummary(txDryRun),
            };
        },
    });
}
