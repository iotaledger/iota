/** Copyright (c) 2024 IOTA Stiftung
 * SPDX-License-Identifier: Apache-2.0
 *
 * Example demonstrating queries for checking the unlock conditions of a basic
 * output. In order to work, it requires a network with test objects
 * generated from iota-genesis-builder/src/stardust/test_outputs.
 */

import {getFullnodeUrl, IotaClient, IotaParsedData} from "@iota/iota-sdk/client";
import {Ed25519Keypair} from "@iota/iota-sdk/keypairs/ed25519";
import {Transaction} from "@iota/iota-sdk/transactions";

async function main() {
    // Build a client to connect to the local IOTA network.
    const iotaClient = new IotaClient({url: getFullnodeUrl('localnet')});

    // Get the Basic object.
    const basicOutputObjectId = "0xde09139ed46b9f5f876671e4403f312fad867c5ae5d300a252e4b6a6f1fa1fbd";
    const basicOutputObject = await iotaClient.getObject({id: basicOutputObjectId, options: { showContent: true }});
    if (!basicOutputObject) {
        throw new Error("Basic output object not found");
    }

    // Extract contents of the BasicOutput object.
    const moveObject = basicOutputObject.data?.content as IotaParsedData;
    if (moveObject.dataType != "moveObject") {
        throw new Error("BasicOutput is not a move object");
    }

    // Treat fields as key-value object.
    const fields = moveObject.fields as Record<string, any>;
    console.log(fields);

    const storageDepositReturnUc = fields['storage_deposit_return_uc'];
    if (storageDepositReturnUc) {
        console.log(`Storage Deposit Return Unlock Condition info: ${storageDepositReturnUc}`);
    }

    const timeLockUc = fields['time_lock_uc'];
    if (timeLockUc) {
        console.log(`Timelocked until: ${timeLockUc}`);
    }

    const expirationUc = fields['expiration_uc'];
    if (expirationUc) {
        console.log(`Expiration Unlock Condition info: ${expirationUc}`);
    }

}

main().catch(error => {
    console.error(`Error: ${error.message}`);
});