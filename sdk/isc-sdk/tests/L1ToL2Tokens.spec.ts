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
import { requestIotaFromFaucetV0 } from '@iota/iota-sdk/faucet';
import { IotaClient } from '@iota/iota-sdk/client';
import { beforeAll, expect, test } from 'vitest';
import { CONFIG } from './config';
import { Wallet } from 'ethers';
import { bcs } from '@iota/iota-sdk/bcs';
import { requestFunds } from './utils';

const { L1, L2 } = CONFIG;
let client: IotaClient;

beforeAll(async () => {
    client = new IotaClient({
        url: L1.rpcUrl,
    });
});

test('Send IOTA', async () => {
    const keypair = new Ed25519Keypair();
    const address = keypair.toIotaAddress();

    await requestIotaFromFaucetV0({
        host: L1.faucetUrl!,
        recipient: address,
    });

    const wallet = Wallet.createRandom();

    // EVM Address
    const recipientAddress = wallet.address;
    // Amount to send (1 IOTAs)
    const amountToSend = BigInt(1 * 1000000000);
    // We also need to place a little more in the bag to cover the L2 gas
    const amountToPlace = amountToSend + L2_FROM_L1_GAS_BUDGET;

    const iscTx = new IscTransaction(L1);

    const bag = iscTx.newBag();
    const coin = iscTx.coinFromAmount({ amount: amountToPlace });
    iscTx.placeCoinInBag({ coin, bag });
    iscTx.createAndSendToEvm({
        bag,
        transfers: [[IOTA_TYPE_ARG, amountToSend]],
        address: recipientAddress,
        accountsContract: getHname(CoreContract.Accounts),
        accountsFunction: getHname(AccountsContractMethod.TransferAllowanceTo),
    });

    const transaction = iscTx.build();
    transaction.setSender(address);
    await transaction.build({ client });

    await client.signAndExecuteTransaction({
        signer: keypair,
        transaction,
    });

    await sleep(2000);

    const evmClient = new EvmRpcClient(L2.evmRpcUrl);
    const evmBalance = await evmClient.getBalanceBaseToken(recipientAddress);
    expect(evmBalance.baseTokens).toEqual(amountToSend.toString());
});

test('Send Non-IOTA Tokens', async () => {
    const MNEMONIC =
        'mom program scrap easily doctor seed slender secret mad flat foam hospital cherry seek river you obscure column blood reflect arch pencil cat burst';
    const TOKEN_COIN_TYPE =
        '0xe1e88f4962b3ea96cfad19aee42f666b04bbce4dc4327c3cd63f1b8ff16e13b2::tool_coin::TOOL_COIN';

    const keypair = Ed25519Keypair.deriveKeypair(MNEMONIC);
    const address = keypair.toIotaAddress();
    const wallet = Wallet.createRandom();

    await requestFunds(client, L1.faucetUrl!, address);

    // EVM Address
    const recipientAddress = wallet.address;
    // Amount to send (0.01 IOTAs)
    const amountToSend = BigInt(1 * 1_000_000);
    // Amount to send (1 Boxfish)
    const tokenAmountToSend = BigInt(1);
    // We also need to place a little more in the bag to cover the L2 gas
    const amountToPlace = amountToSend + L2_FROM_L1_GAS_BUDGET;

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

    await client.signAndExecuteTransaction({
        signer: keypair,
        transaction,
    });

    await sleep(2000);

    const evmClient = new EvmRpcClient(L2.evmRpcUrl);
    const evmBalance = await evmClient.getBalanceBaseToken(recipientAddress);
    expect(evmBalance.baseTokens).toEqual(amountToSend.toString());
    expect(evmBalance.nativeTokens).toEqual([
        {
            coinType: TOKEN_COIN_TYPE,
            balance: tokenAmountToSend.toString(),
        },
    ]);
});

const sleep = (x) => new Promise((r) => setTimeout(r, x));
