// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin, type GasSummaryType } from '@mysten/core';
import { useCurrentAccount } from '@mysten/dapp-kit';
import { formatAddress } from '@mysten/sui.js/utils';

export const GAS_TYPE_ARG = '0x2::sui::SUI';
export const GAS_SYMBOL = 'SUI';

export default function GasSummary({ gasSummary }: { gasSummary?: GasSummaryType }) {
    const [gas, symbol] = useFormatCoin(gasSummary?.totalGas, GAS_TYPE_ARG);
    const address = useCurrentAccount();

    if (!gasSummary) return null;

    return (
        <div className="p-3">
            <h3 className="text-center font-semibold">Gas Fees</h3>
            <div className="flex w-full flex-col items-center gap-2 px-4 py-3">
                <div className="flex w-full items-center justify-center">
                    {address?.address === gasSummary?.owner && (
                        <div className="mr-auto">You Paid</div>
                    )}
                    <p>
                        {gasSummary?.isSponsored ? '0' : gas} {symbol}
                    </p>
                </div>
                {gasSummary?.isSponsored && gasSummary.owner && (
                    <>
                        <div className="flex w-full justify-between">
                            Paid by Sponsor
                            {gas} {symbol}
                        </div>
                        <div className="flex w-full justify-between">
                            Sponsor:
                            {formatAddress(gasSummary.owner)}
                        </div>
                    </>
                )}
            </div>
        </div>
    );
}
