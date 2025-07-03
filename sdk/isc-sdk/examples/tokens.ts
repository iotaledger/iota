/* eslint-disable require-extensions/require-extensions */
import {
    AccountsContractMethod,
    CoreContract,
    EvmRpcClient,
    getHname,
    IscTransaction,
    L2_FROM_L1_GAS_BUDGET,
} from '../src/index.js';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { IotaClient } from '@iota/iota-sdk/client';
import { CONFIG } from './config.js';
import { bcs } from '@iota/iota-sdk/bcs';

const { L1, L2 } = CONFIG;
// console.log('L1:', L1, 'L2:', L2);
const client = new IotaClient({
    url: L1.rpcUrl,
});
const evmRpcClient = new EvmRpcClient(L2.evmRpcUrl);

const MNEMONIC =
    'mom program scrap easily doctor seed slender secret mad flat foam hospital cherry seek river you obscure column blood reflect arch pencil cat burst';

// const TOKEN_COIN_TYPE =
//     '0xe1e88f4962b3ea96cfad19aee42f666b04bbce4dc4327c3cd63f1b8ff16e13b2::tool_coin::TOOL_COIN';
// const TOKEN_COIN_TYPE =
//     '0x2ec9e750b79178d0c6f3aceca00f43990a9f30c0d26590a160a2af71d4b02916::mint_coin::MINT_COIN';

const TOKEN_COIN_TYPE =
    '0x3fbd238eea1f4ce7d797148954518fce853f24a8be01b47388bfa2262602fefa::vusd::VUSD';
// 0x2::coin::Coin<0x929065320c756b8a4a841deeed013bd748ee45a28629c4aaafc56d8948ebb081::vusd::VUSD>
// Wrap your code in an async function
async function main() {
    const keypair = Ed25519Keypair.deriveKeypair(MNEMONIC);
    const address = keypair.toIotaAddress();

    // EVM Address
    const recipientAddress = process.argv[2];

    if (!recipientAddress) {
        console.error('Please provide a recipient address as an argument');
        process.exit(1);
    }
    const coinMetadata = await client.getCoinMetadata({ coinType: TOKEN_COIN_TYPE });
    console.log('Coin Metadata:', coinMetadata);
    // Amount to send (0.01 IOTAs)
    const amountToSend = BigInt(1 * 1_000_000_000); // 1 IOTA in nanos
    // Amount to send (1 Boxfish)
    const tokenAmountToSend = BigInt(1 * 1_000_000); // 1 VUSD in nanos (6 decimals)
    // We also need to place a little more in the bag to cover the L2 gas
    const amountToPlace = amountToSend + L2_FROM_L1_GAS_BUDGET;

    console.log('Sending...');

    const iscTx = new IscTransaction(L1);
    const tx = iscTx.transaction();

    const bag = iscTx.newBag();

    // Place IOTA
    const iotaCoin = iscTx.coinFromAmount({ amount: amountToPlace });
    iscTx.placeCoinInBag({ coin: iotaCoin, bag, coinType: IOTA_TYPE_ARG });

    // Place Token
    const [tokenCoin] = tx.splitCoins(
        // tx.object('0xff467e47343ea4f4724c5ed62edfc2db94c70d74417a28d2c3257b6830e48d39'), // mint coin
        // tx.object('0xf7662ffd9cb079d8e75ab4805ba78fdb0e0fb78cf49aa0fa01ecb7ebdf15d04e'),

        tx.object('0x30535f52178e17e18e228709943e7e732f1f75445b3ba21eb98898fbb3f82587'),
        [tx.pure(bcs.U64.serialize(tokenAmountToSend))],
    );
    iscTx.placeCoinInBag({
        bag,
        coin: tokenCoin,
        coinType: TOKEN_COIN_TYPE,
    });

    iscTx.createAndSendToEvm({
        bag,
        transfers: [
            [IOTA_TYPE_ARG, amountToSend],
            // [TOKEN_COIN_TYPE, tokenAmountToSend],
        ],
        address: recipientAddress,
        accountsContract: getHname(CoreContract.Accounts),
        accountsFunction: getHname(AccountsContractMethod.TransferAllowanceTo),
    });

    const transaction = iscTx.build();
    transaction.setSender(address);

    const txBytes = await transaction.build({ client });
    const txDryRun = await client.dryRunTransactionBlock({
        transactionBlock: txBytes,
    });
    console.log('Dry run result:', txDryRun);
    const { digest } = await client.signAndExecuteTransaction({
        signer: keypair,
        transaction,
    });

    await client.waitForTransaction({
        digest,
    });

    console.log('Sent!');

    const l1BalanceInL2 = await evmRpcClient.getBalanceBaseToken(address);

    console.log(
        `L2 balance of the L1 address ${address}: ${JSON.stringify(l1BalanceInL2, null, 2)}`,
    );
}

// Execute the function and handle errors
main().catch((error) => {
    console.error('Error executing script:', error);
    process.exit(1);
});
