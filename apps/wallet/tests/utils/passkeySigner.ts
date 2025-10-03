// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { CDPSession } from '@playwright/test';
export async function setPresence(client: CDPSession, authenticatorId: string, enabled: boolean) {
    await client.send('WebAuthn.setAutomaticPresenceSimulation', { authenticatorId, enabled });
}

export async function setVerified(
    client: CDPSession,
    authenticatorId: string,
    isUserVerified: boolean,
) {
    await client.send('WebAuthn.setUserVerified', { authenticatorId, isUserVerified });
}
