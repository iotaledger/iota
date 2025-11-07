import type {
    DryRunTransactionBlockResponse,
    GasCostSummary,
    IotaGasData,
    IotaTransactionBlockResponse,
    TransactionEffects,
} from '@iota/iota-sdk/client';
import { Optional } from '@tanstack/react-query';

export type GasSummaryType =
    | (GasCostSummary &
          Optional<IotaGasData, keyof IotaGasData> & {
              isSponsored: boolean;
              gasUsed: GasCostSummary;
              totalGas?: string;
              owner?: string;
          })
    | null;

export function getGasSummary(
    transaction: IotaTransactionBlockResponse | DryRunTransactionBlockResponse,
): GasSummaryType {
    const { effects } = transaction;
    if (!effects) {
        return null;
    }
    let sender: string | undefined;
    let gasData = {} as IotaGasData;
    if ('transaction' in transaction && transaction.transaction?.data) {
        sender = transaction.transaction.data.sender;
        gasData = transaction.transaction.data.gasData;
    } else if ('input' in transaction) {
        sender = transaction.input.sender;
        gasData = transaction.input.gasData;
    }
    const owner = gasData?.owner;
    const totalGas = getTotalGasUsed(effects);

    return {
        ...effects.gasUsed,
        ...gasData,
        owner,
        totalGas: totalGas?.toString(),
        isSponsored: !!owner && !!sender && owner !== sender,
        gasUsed: transaction?.effects!.gasUsed,
    };
}

export function getTotalGasUsed(effects: TransactionEffects): bigint | undefined {
    const gasSummary = effects?.gasUsed;
    return gasSummary
        ? BigInt(gasSummary.computationCost) +
              BigInt(gasSummary.storageCost) -
              BigInt(gasSummary.storageRebate)
        : undefined;
}
