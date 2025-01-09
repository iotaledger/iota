// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PortStream } from '_src/shared/messaging/portStream';
import { WindowMessageStream } from '_src/shared/messaging/windowMessageStream';
import type { Message } from '_src/shared/messaging/messages';
import { generateWalletMessageStreamIdentifiers } from '_src/shared/utils/generateWalletMessageStreamIdentifiers';
import { take } from 'rxjs';

function createPort(windowMsgStream: WindowMessageStream, currentMsg?: Message) {
    const port = PortStream.connectToBackgroundService('iota_content<->background');
    if (currentMsg) {
        port.sendMessage(currentMsg);
    }
    port.onMessage.subscribe((msg) => {
        windowMsgStream.send(msg);
    });
    const windowMsgSub = windowMsgStream.messages.subscribe((msg) => {
        port.sendMessage(msg);
    });
    port.onDisconnect.subscribe((port) => {
        windowMsgSub.unsubscribe();
        createPort(windowMsgStream);
    });
}

export function setupMessagesProxy() {
    const clientIds = generateWalletMessageStreamIdentifiers(process.env.APP_NAME);
    const windowMsgStream = new WindowMessageStream(clientIds.target, clientIds.name);
    windowMsgStream.messages.pipe(take(1)).subscribe((msg) => {
        createPort(windowMsgStream, msg);
    });
}
