// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ArrowTopRight, IotaLogoMark } from '@iota/ui-icons';
import { WALLET_DOWNLOAD_URL } from '../../../constants/walletDefaults.js';
import { Button } from '@iota/apps-ui-kit';

export function GetTheWalletView() {
    return (
        <div className="flex flex-col items-center gap-md p-md--rs">
            <IotaLogoMark height={80} width={80} className="text-neutral-0 dark:text-neutral-100" />
            <p className="text-body-lg text-center text-neutral-10 dark:text-neutral-92">
                Don't have a wallet yet?
            </p>
            <a href={WALLET_DOWNLOAD_URL} target="_blank" rel="noopener noreferrer">
                <Button text="Get the IOTA Wallet Here" icon={<ArrowTopRight />} iconAfterText />
            </a>
        </div>
    );
}
