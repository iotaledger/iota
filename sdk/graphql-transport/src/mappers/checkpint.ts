// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Checkpoint, EndOfEpochData } from '@iota/iota-sdk/client';

import type { Rpc_Checkpoint_FieldsFragment } from '../generated/queries.js';

export function mapGraphQLCheckpointToRpcCheckpoint(
    checkpoint: Rpc_Checkpoint_FieldsFragment,
): Checkpoint {
    const endOfEpochTx = checkpoint.endOfEpoch.nodes[0];
    let endOfEpochData: EndOfEpochData | undefined;

    if (
        endOfEpochTx?.kind?.__typename === 'EndOfEpochTransaction' &&
        endOfEpochTx.kind?.transactions.nodes[0].__typename === 'ChangeEpochTransactionV2'
    ) {
        endOfEpochData = {
            epochCommitments: [], // TODO
            nextEpochCommittee:
                endOfEpochTx.kind.transactions.nodes[0].epoch?.validatorSet?.committeeMembers?.nodes.map(
                    (val) => [val.credentials?.authorityPubKey, val.votingPower?.toString()!],
                ) ?? [],
            nextEpochProtocolVersion: String(
                endOfEpochTx.kind.transactions.nodes[0].epoch?.protocolConfigs.protocolVersion,
            ),
            epochSupplyChange: 0, // TODO: https://github.com/iotaledger/iota/issues/1738
        };
    }

    return {
        checkpointCommitments: [], // TODO
        digest: checkpoint.digest,
        endOfEpochData,
        epoch: String(checkpoint.epoch?.epochId),
        epochRollingGasCostSummary: {
            computationCost: checkpoint.rollingGasSummary?.computationCost,
            computationCostBurned: checkpoint.rollingGasSummary?.computationCostBurned,
            nonRefundableStorageFee: checkpoint.rollingGasSummary?.nonRefundableStorageFee,
            storageCost: checkpoint.rollingGasSummary?.storageCost,
            storageRebate: checkpoint.rollingGasSummary?.storageRebate,
        },
        networkTotalTransactions: String(checkpoint.networkTotalTransactions),
        ...(checkpoint.previousCheckpointDigest
            ? { previousDigest: checkpoint.previousCheckpointDigest }
            : {}),
        sequenceNumber: String(checkpoint.sequenceNumber),
        timestampMs: new Date(checkpoint.timestamp).getTime().toString(),
        transactions:
            checkpoint.transactionBlocks?.nodes.map(
                (transactionBlock) => transactionBlock.digest!,
            ) ?? [],
        validatorSignature: checkpoint.validatorSignatures,
    };
}
