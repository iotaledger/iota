// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CONSTANTS, QueryKey } from "@/constants";
import { useTransactionExecution } from "@/hooks/useTransactionExecution";
import { useCurrentAccount, useIotaClient } from "@iota/dapp-kit";
import { IotaObjectData } from "@iota/iota-sdk/client";
import { TransactionBlock } from "@iota/iota-sdk/transactions";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import toast from "react-hot-toast";

/**
 * Builds and executes the PTB to lock an object.
 */
export function useLockObjectMutation() {
  const account = useCurrentAccount();
  const executeTransaction = useTransactionExecution();

  return useMutation({
    mutationFn: async ({ object }: { object: IotaObjectData }) => {
      if (!account?.address)
        throw new Error("You need to connect your wallet!");
      const txb = new TransactionBlock();

      const [locked, key] = txb.moveCall({
        target: `${CONSTANTS.escrowContract.packageId}::lock::lock`,
        arguments: [txb.object(object.objectId)],
        typeArguments: [object.type!],
      });

      txb.transferObjects([locked, key], txb.pure.address(account.address));

      return executeTransaction(txb);
    },
  });
}

/**
 * Builds and executes the PTB to unlock an object.
 */
export function useUnlockMutation() {
  const account = useCurrentAccount();
  const executeTransaction = useTransactionExecution();
  const client = useIotaClient();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      lockedId,
      keyId,
      iotaObject,
    }: {
      lockedId: string;
      keyId: string;
      iotaObject: IotaObjectData;
    }) => {
      if (!account?.address)
        throw new Error("You need to connect your wallet!");
      const key = await client.getObject({
        id: keyId,
        options: {
          showOwner: true,
        },
      });

      if (
        !key.data?.owner ||
        typeof key.data.owner === "string" ||
        !("AddressOwner" in key.data.owner) ||
        key.data.owner.AddressOwner !== account.address
      ) {
        toast.error("You are not the owner of the key");
        return;
      }

      const txb = new TransactionBlock();

      const item = txb.moveCall({
        target: `${CONSTANTS.escrowContract.packageId}::lock::unlock`,
        typeArguments: [iotaObject.type!],
        arguments: [txb.object(lockedId), txb.object(keyId)],
      });

      txb.transferObjects([item], txb.pure.address(account.address));

      return executeTransaction(txb);
    },
    onSuccess: () => {
      setTimeout(() => {
        // invalidating the queries after a small latency
        // because the indexer works in intervals of 1s.
        // if we invalidate too early, we might not get the latest state.
        queryClient.invalidateQueries({
          queryKey: [QueryKey.Locked],
        });
      }, 1_000);
    },
  });
}
