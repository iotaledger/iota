import React, { useState } from "react";
import { useCurrentAccount } from "@iota/dapp-kit";
import { getFaucetHost, requestIotaFromFaucetV1 } from '@iota/iota-sdk/faucet';

export default function FaucetButton() {
    const account = useCurrentAccount();
    const [isRequesting, setIsRequesting] = useState(false);
    const [hasAlreadyRequested, sethasAlreadyRequested] = useState(false);

    const onClick = async () => {
        setIsRequesting(true);
        try {
            await requestIotaFromFaucetV1({
                host: getFaucetHost('testnet'),
                recipient: account.address,
            });
        } catch (error) {
            console.error("Faucet request failed", error);
        } finally {
            setIsRequesting(false);
            sethasAlreadyRequested(true);
        }
    };

    return (
        account ? (
            hasAlreadyRequested ? (
                <code>Already requested Token</code>
            ) : (
                <button onClick = { onClick } className = "button button--primary" disabled = { isRequesting }>
                    { isRequesting? "Requesting...": "Request Token" }
                </button >
            )
        ) : (
            <code>No Wallet connected</code>
        )
    );
}
