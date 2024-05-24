// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client'

import { useCurrentAccount, useCurrentWallet, useDeriveAddress } from "@mysten/dapp-kit";
import React from "react";

function StakingDashboardPage(): JSX.Element {
    const [derivedAddress, setDerivedAddress] = React.useState('<Connect your Wallet>');
    const [accountIndex, setAccountIndex] = React.useState('0');
    const [addressIndex, setAddressIndex] = React.useState('0');
    const derive = useDeriveAddress();
    const { connectionStatus } = useCurrentWallet();
    const account = useCurrentAccount();

    React.useEffect(() => {
        if(connectionStatus === 'connected' && account?.address) {
            derive.mutateAsync({
                accountIndex: Number(accountIndex),
                addressIndex: Number(addressIndex),
                address: account?.address
            }).then(res => {
                setDerivedAddress(res.address)
            }).catch((err) => {
                console.log(err);
                setDerivedAddress('error')
            })
        }
    }, [accountIndex, addressIndex, connectionStatus, account?.address])

    return (
        <div className="flex flex-col items-center justify-center pt-12">
            <h1>MIGRATIONS</h1>
            <p>The address with an account index of 0 should be the same as your first account.</p>
            <span>Account</span>
            <input type="number" className="inline-block w-16" value={accountIndex} onChange={(e) => setAccountIndex(e.target.value)}></input>
            <span>Address</span>
            <input type="number" className="inline-block w-16" value={addressIndex} onChange={(e) => setAddressIndex(e.target.value)}></input>
            <p>{derivedAddress}</p>
        </div>
    );
}

export default StakingDashboardPage;
