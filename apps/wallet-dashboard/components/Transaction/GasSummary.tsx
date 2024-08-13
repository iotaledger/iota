// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin, type GasSummaryType } from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { formatAddress, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

export default function GasSummary({ gasSummary }: { gasSummary: GasSummaryType }) {
    const [gas, symbol] = useFormatCoin(gasSummary?.totalGas, IOTA_TYPE_ARG);
    const address = useCurrentAccount();

    return (
        <div className="rounded-lg border border-solid border-black p-3">
            <h3 className="text-center font-semibold">Gas Fees</h3>
            <div className="flex w-full flex-col items-center gap-2 px-4 py-3">
                <div className="flex items-center justify-center gap-4">
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
