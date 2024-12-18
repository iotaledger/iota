// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { Transaction } from '@iota/iota-sdk/transactions';
import { config } from 'dotenv';

import { DeepBookMarketMaker } from './deepbookMarketMaker.js';

// Load private key from .env file
config();

(async () => {
    const privateKey = process.env.PRIVATE_KEY;
    if (!privateKey) {
        throw new Error('Private key not found');
    }

    // Initialize with balance managers if created
    const balanceManagers = {
        MANAGER_1: {
            address: '0x6149bfe6808f0d6a9db1c766552b7ae1df477f5885493436214ed4228e842393',
            tradeCap: '',
        },
    };
    const mmClient = new DeepBookMarketMaker(
        privateKey,
        'testnet',
        balanceManagers,
        process.env.ADMIN_CAP,
    );

    const tx = new Transaction();

    // Read only call
    console.log(await mmClient.checkManagerBalance('MANAGER_1', 'IOTA'));
    console.log(await mmClient.getLevel2Range('IOTA_DBUSDC', 0.1, 100, true));

    // // Balance manager contract call
    // mmClient.balanceManager.depositIntoManager('MANAGER_1', 'IOTA', 10)(tx);

    // // Example PTB call
    // mmClient.placeLimitOrderExample(tx);
    // mmClient.flashLoanExample(tx);

    let res = await mmClient.signAndExecute(tx);

    console.dir(res, { depth: null });
})();
