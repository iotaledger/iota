// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { WALLET_DOWNLOAD_URL } from '../../../constants/walletDefaults.js';
import { Button } from '../../ui/Button.js';
import * as styles from './GetTheWallet.css.js';

export function GetTheWalletView() {
    return (
        <div className={styles.container}>
            <p className={styles.text}>Don't have a wallet yet?</p>
            <a href={WALLET_DOWNLOAD_URL} target="_blank" rel="noopener noreferrer">
                <Button size="lg" variant="primary" className={styles.button}>
                    Get the wallet
                </Button>
            </a>
        </div>
    );
}
