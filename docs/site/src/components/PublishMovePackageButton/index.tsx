import React from "react";
import { useCurrentAccount, useSignAndExecuteTransaction } from "@iota/dapp-kit";
import { Transaction } from "@iota/iota-sdk/transactions";
import { MovePackageJsonData } from "./types/types";

export default function PublishMovePackageButton(
    _contractJson,
) {
    const { mutate: signAndExecuteTransaction } = useSignAndExecuteTransaction();
    const currentAccount = useCurrentAccount();

    const contractJson: MovePackageJsonData = JSON.parse(_contractJson.contractJson);

    const onClick = () => {
        const movePublishTx = new Transaction();

        console.log('Publishing contract', contractJson);
        console.log('Modules', contractJson.modules);
        console.log('Dependencies', contractJson.dependencies);
        const upgradeCap = movePublishTx.publish({
            modules: contractJson.modules,
            dependencies: contractJson.dependencies,
        });
        console.log('Published contract', contractJson);
        movePublishTx.transferObjects([upgradeCap], movePublishTx.pure.address(currentAccount?.address));

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

    return <button onClick={onClick}>Publish</button>
}