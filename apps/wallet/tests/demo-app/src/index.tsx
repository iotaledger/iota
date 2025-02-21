// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaWallet } from '_src/dapp-interface/walletStandardInterface';
import { Transaction } from '@iota/iota-sdk/transactions';
import { getWallets, ReadonlyWalletAccount, type Wallet } from '@iota/wallet-standard';
import { useEffect, useState } from 'react';
import ReactDOM from 'react-dom/client';

function getDemoTransaction(address: string) {
    const txb = new Transaction();
    const [coin] = txb.splitCoins(txb.gas, [1]);
    txb.transferObjects([coin], address);
    return txb;
}

function getAccount(account: ReadonlyWalletAccount, useWrongAccount: boolean) {
    if (useWrongAccount && account) {
        const newAccount = new ReadonlyWalletAccount({
            address: '0x00000001',
            chains: account.chains,
            features: account.features,
            publicKey: account.publicKey,
            icon: account.icon,
            label: account.label,
        });
        return newAccount;
    }
    return account;
}

function findIotaWallet(wallets: readonly Wallet[]) {
    return (wallets.find((aWallet) => aWallet.name.includes('IOTA Wallet')) ||
        null) as IotaWallet | null;
}

function App() {
    const [iotaWallet, setIotaWallet] = useState<IotaWallet | null>(() =>
        findIotaWallet(getWallets().get()),
    );
    const [error, setError] = useState<string | null>(null);
    const [accounts, setAccounts] = useState<ReadonlyWalletAccount[]>(
        () => iotaWallet?.accounts || [],
    );
    const [useWrongAccounts, setUseWrongAccounts] = useState(false);

    useEffect(() => {
        const walletsApi = getWallets();
        function updateWallets() {
            setIotaWallet(findIotaWallet(walletsApi.get()));
        }
        const unregister1 = walletsApi.on('register', updateWallets);
        const unregister2 = walletsApi.on('unregister', updateWallets);
        return () => {
            unregister1();
            unregister2();
        };
    }, []);
    useEffect(() => {
        if (iotaWallet) {
            return iotaWallet.features['standard:events'].on('change', ({ accounts }) => {
                if (accounts) {
                    setAccounts(iotaWallet.accounts);
                }
            });
        }
    }, [iotaWallet]);
    if (!iotaWallet) {
        return <h1>IOTA Wallet not found</h1>;
    }
    return (
        <>
            <h1>IOTA Wallet is installed. ({iotaWallet.name})</h1>
            {accounts.length ? (
                <ul data-testid="accounts-list">
                    {accounts.map((anAccount) => (
                        <li key={anAccount.address}>{anAccount.address}</li>
                    ))}
                </ul>
            ) : (
                <button onClick={async () => iotaWallet.features['standard:connect'].connect()}>
                    Connect
                </button>
            )}
            <label>
                <input
                    type="checkbox"
                    checked={useWrongAccounts}
                    onChange={() => setUseWrongAccounts((v) => !v)}
                />
                Use wrong account
            </label>
            <button
                onClick={async () => {
                    setError(null);
                    try {
                        const txb = getDemoTransaction(accounts[0]?.address || '');
                        await iotaWallet.features[
                            'iota:signAndExecuteTransaction'
                        ].signAndExecuteTransaction({
                            transaction: txb,
                            account: getAccount(accounts[0], useWrongAccounts),
                            chain: 'iota:unknown',
                        });
                    } catch (e) {
                        setError((e as Error).message);
                    }
                }}
            >
                Send transaction
            </button>
            <button
                onClick={async () => {
                    setError(null);
                    try {
                        const txb = getDemoTransaction(accounts[0]?.address || '');
                        await iotaWallet.features['iota:signTransaction'].signTransaction({
                            transaction: txb,
                            account: getAccount(accounts[0], useWrongAccounts),
                            chain: 'iota:unknown',
                        });
                    } catch (e) {
                        setError((e as Error).message);
                    }
                }}
            >
                Sign transaction
            </button>
            <button
                onClick={async () => {
                    setError(null);
                    try {
                        await iotaWallet.features['iota:signPersonalMessage']?.signPersonalMessage({
                            account: getAccount(accounts[0], useWrongAccounts),
                            message: new TextEncoder().encode('Test message'),
                        });
                    } catch (e) {
                        setError((e as Error).message);
                    }
                }}
            >
                Sign message
            </button>
            {error ? (
                <div>
                    <h6>Error</h6>
                    <div>{error}</div>
                </div>
            ) : null}
        </>
    );
}

ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
