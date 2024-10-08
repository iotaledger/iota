// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useSignTransactionBlock, useIotaClient } from "@iota/dapp-kit";
import { IotaTransactionBlockResponse } from "@iota/iota-sdk/client";
import { TransactionBlock } from "@iota/iota-sdk/transactions";
import toast from "react-hot-toast";

/**
 * A hook to execute transactions.
 * It signs the transaction using the wallet and executes it through the RPC.
 *
 * That allows read-after-write consistency and is generally considered a best practice.
 */
export function useTransactionExecution() {
  const client = useIotaClient();
  const { mutateAsync: signTransactionBlock } = useSignTransactionBlock();

  const executeTransaction = async (
    txb: TransactionBlock,
  ): Promise<IotaTransactionBlockResponse | void> => {
    try {
      const signature = await signTransactionBlock({
        transactionBlock: txb,
      });

      const res = await client.executeTransactionBlock({
        transactionBlock: signature.transactionBlockBytes,
        signature: signature.signature,
        options: {
          showEffects: true,
          showObjectChanges: true,
        },
      });

      toast.success("Successfully executed transaction!");
      return res;
    } catch (e: any) {
      toast.error(`Failed to execute transaction: ${e.message as string}`);
    }
  };

  return executeTransaction;
}
