import { IotaClient } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { EvmRpcClient } from '@iota/isc-sdk';
import { ethers, JsonRpcProvider } from 'ethers';
import { CONFIG } from '../config/config';
import { checkBalanceWithRetries } from '../utils/utils';

export async function getL1BalanceForAddress(address: string): Promise<string> {
    const { L1 } = CONFIG;

    const client = new IotaClient({
        url: L1.rpcUrl,
    });

    const balance = await client.getBalance({ owner: address });

    return ethers.formatUnits(balance.totalBalance, 9);
}

export async function getEVMBalanceForAddress(address: string): Promise<string> {
    const provider = new JsonRpcProvider(CONFIG.L2.rpcUrl);
    const balanceWei = await provider.getBalance(address);

    return ethers.formatEther(balanceWei);
}

export async function getL1CoinBalanceForAddress(
    address: string,
    coinType: string,
): Promise<string> {
    const { L1 } = CONFIG;

    const client = new IotaClient({
        url: L1.rpcUrl,
    });

    const balance = await client.getAllBalances({ owner: address });
    const coinBalance = balance.find((coin) => coin.coinType === coinType);
    if (!coinBalance) {
        throw new Error(`Coin type ${coinType} not found in balance.`);
    }
    return coinBalance.totalBalance;
}

export async function getL2CoinBalanceForAddress(
    address: string,
    coinType: string,
): Promise<string> {
    const { L2 } = CONFIG;
    const evmRpcClient = new EvmRpcClient(L2.evmRpcUrl);
    const balance = await evmRpcClient.getBalanceBaseToken(address);
    if (coinType === IOTA_TYPE_ARG) {
        return balance.baseTokens;
    }
    const nativeToken = balance?.nativeTokens?.find((token) => token.coinType === coinType);
    return nativeToken ? nativeToken.balance : '0';
}

export async function checkL1IotaBalanceWithRetries(address: string) {
    return await checkBalanceWithRetries(() => getL1BalanceForAddress(address), 'L1');
}

export async function checkL2IotaBalanceWithRetries(address: string) {
    return await checkBalanceWithRetries(() => getEVMBalanceForAddress(address), 'L2');
}

export async function checkL1CoinBalanceForAddressWithRetries(address: string, coinType: string) {
    return await checkBalanceWithRetries(() => getL1CoinBalanceForAddress(address, coinType), 'L1');
}

export async function checkL2CoinBalanceForAddressWithRetries(address: string, coinType: string) {
    return await checkBalanceWithRetries(() => getL2CoinBalanceForAddress(address, coinType), 'L2');
}
