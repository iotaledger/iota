import React from "react";
import { useCurrentAccount, useSignAndExecuteTransaction } from "@iota/dapp-kit";
import { Transaction } from "@iota/iota-sdk/transactions";
import { MovePackageJsonData } from "./types";

export default function PublishMovePackageButton(
    { contractJson }: { contractJson: MovePackageJsonData }
) {
    const { mutate: signAndExecuteTransaction } = useSignAndExecuteTransaction();
    const currentAccount = useCurrentAccount();

    const onClick = () => {
        const movePublishTx = new Transaction();

        const upgradeCap = movePublishTx.publish({
            modules: contractJson.modules,
            dependencies: contractJson.dependencies,
        });
        movePublishTx.transferObjects([upgradeCap], movePublishTx.pure.address(currentAccount.address));

        signAndExecuteTransaction(
            {
                transaction: movePublishTx,
            },
            {
                onSuccess: () => {
                    console.log('Transaction succeeded');
                }
            },
        );
    };

    return (
        currentAccount ? (
            <button className='button button--primary' onClick={onClick}>Publish</button>
        ) : (
            <code>No Wallet connected</code>
        )
    );
}