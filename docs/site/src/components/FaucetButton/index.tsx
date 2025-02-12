import React from "react";
import { useCurrentAccount } from "@iota/dapp-kit";
import { getFaucetHost, requestIotaFromFaucetV1 } from '@iota/iota-sdk/faucet';

export default function FaucetButton() {
    const account = useCurrentAccount();

    const onClick = () => {
        requestIotaFromFaucetV1({
            host: getFaucetHost('testnet'),
            recipient: account.address,
        });
    };

    return (
        account ? (
            <button onClick={onClick} className="button button--primary">Request Token</button>
        ) : (
            <code>No Wallet connected</code>
        )
    );
}
