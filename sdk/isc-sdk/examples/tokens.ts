import {
    AccountsContractMethod,
    CoreContract,
    EvmRpcClient,
    getHname,
    IscTransaction,
    L2_FROM_L1_GAS_BUDGET,
} from '../src/index';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { IotaClient } from '@iota/iota-sdk/client';
import { CONFIG } from './config';
import { bcs } from '@iota/iota-sdk/bcs';

const { L1, L2 } = CONFIG;

const client = new IotaClient({
    url: L1.rpcUrl,
});
const evmRpcClient = new EvmRpcClient(L2.evmRpcUrl);

const MNEMONIC =
    'mom program scrap easily doctor seed slender secret mad flat foam hospital cherry seek river you obscure column blood reflect arch pencil cat burst';
const TOKEN_COIN_TYPE =
    '0xe1e88f4962b3ea96cfad19aee42f666b04bbce4dc4327c3cd63f1b8ff16e13b2::tool_coin::TOOL_COIN';

const keypair = Ed25519Keypair.deriveKeypair(MNEMONIC);
const address = keypair.toIotaAddress();

// EVM Address
const recipientAddress = process.argv[2];

if (!recipientAddress) {
    console.error('Please provide a recipient address as an argument');
    process.exit(1);
}

// Amount to send (0.01 IOTAs)
const amountToSend = BigInt(1 * 1_000_000);
// Amount to send (1 Boxfish)
const tokenAmountToSend = BigInt(1);
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
const tokenCoin = tx.splitCoins(
    tx.object('0xf7662ffd9cb079d8e75ab4805ba78fdb0e0fb78cf49aa0fa01ecb7ebdf15d04e'),
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
        [TOKEN_COIN_TYPE, tokenAmountToSend],
    ],
    address: recipientAddress,
    accountsContract: getHname(CoreContract.Accounts),
    accountsFunction: getHname(AccountsContractMethod.TransferAllowanceTo),
});

const transaction = iscTx.build();
transaction.setSender(address);

await transaction.build({ client });

const { digest } = await client.signAndExecuteTransaction({
    signer: keypair,
    transaction,
});

await client.waitForTransaction({
    digest,
});

console.log('Sent!');

const l1BalanceInL2 = await evmRpcClient.getBalanceBaseToken(address);

console.log(`L2 balance of the L1 address ${address}: ${JSON.stringify(l1BalanceInL2)}`);
