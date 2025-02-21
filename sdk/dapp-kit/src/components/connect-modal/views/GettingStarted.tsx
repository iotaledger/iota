// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button } from '../../ui/Button.js';
import { Heading } from '../../ui/Heading.js';
import { InfoSection } from '../InfoSection.js';
import * as styles from './GettingStarted.css.js';

export function GettingStarted() {
    return (
        <div className={styles.container}>
            <Heading as="h2">Get Started with IOTA</Heading>
            <div className={styles.content}>
                <InfoSection title="Install the IOTA Wallet Extension">
                    We recommend pinning IOTA Wallet to your taskbar for quicker access.
                </InfoSection>
                <InfoSection title="Create or Import a Wallet">
                    Be sure to back up your wallet using a secure method. Never share your secret
                    phrase with anyone.
                </InfoSection>
                <InfoSection title="Refresh Your Browser">
                    Once you set up your wallet, refresh this window browser to load up the
                    extension.
                </InfoSection>
                <div className={styles.installButtonContainer}>
                    <Button variant="primary" asChild>
                        <a
                            href="https://chrome.google.com/webstore/detail/iota-wallet/iidjkmdceolghepehaaddojmnjnkkija"
                            target="_blank"
                            rel="noreferrer"
                        >
                            Install Wallet Extension
                        </a>
                    </Button>
                </div>
            </div>
        </div>
    );
}
