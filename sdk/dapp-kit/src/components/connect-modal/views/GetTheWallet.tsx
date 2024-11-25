// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { WALLET_DOWNLOAD_URL } from '../../../constants/walletDefaults.js';
import { ArrowTopRightIcon } from '../../icons/ArrowTopRightIcon.js';
import { IotaIcon } from '../../icons/IotaIcon.js';
import { Button } from '../../ui/Button.js';
import * as styles from './GetTheWallet.css.js';

export function GetTheWalletView() {
    return (
        <div className={styles.container}>
            <div className={styles.content}>
                <IotaIcon className={styles.icon} width={48} height={48} />
                <p className={styles.text}>Don't have a wallet yet?</p>
            </div>
            <a href={WALLET_DOWNLOAD_URL} target="_blank" rel="noopener noreferrer">
                <Button size="md" variant="primary" className={styles.button}>
                    Get the IOTA wallet
                    <ArrowTopRightIcon width={16} height={16} />
                </Button>
            </a>
        </div>
    );
}
