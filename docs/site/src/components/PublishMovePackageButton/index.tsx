import React, { useState } from "react";
import { useCurrentAccount, useIotaClient, useSignAndExecuteTransaction } from "@iota/dapp-kit";
import { Transaction } from "@iota/iota-sdk/transactions";
import { MovePackageJsonData } from "./types";
import { Networks } from "../constant";
import Link from "@docusaurus/Link";

export default function PublishMovePackageButton(
    { contractJson }: { contractJson: MovePackageJsonData }
) {
    const client = useIotaClient();
    const { mutate: signAndExecuteTransaction } = useSignAndExecuteTransaction({
        execute: async ({ bytes, signature }) =>
            await client.executeTransactionBlock({
                transactionBlock: bytes,
                signature,
                options: {
                    // Raw effects are required so the effects can be reported back to the wallet
                    showRawEffects: true,
                    // Show effects and object changes so that the status and packageId can be extracted
                    showEffects: true,
                    showObjectChanges: true,
                },
            }),
    });
    const currentAccount = useCurrentAccount();
    const [isPublishing, setIsPublishing] = useState(false);
    const [packageId, setPackageId] = useState<string | null>(null);

    const onClick = () => {
        setIsPublishing(true);
        const movePublishTx = new Transaction();

        const upgradeCap = movePublishTx.publish({
            modules: contractJson.modules,
            dependencies: contractJson.dependencies,
        });
        // Transfer the upgrade cap to the current account
        movePublishTx.transferObjects([upgradeCap], movePublishTx.pure.address(currentAccount.address));

        signAndExecuteTransaction(
            {
                transaction: movePublishTx,
            },
            {
                onSuccess: (result) => {
                    if (result.effects.status.status === "success") {
                        console.log("Publish successful", result);
                        // Extract the packageId from the object changes
                        const publishedObject = result.objectChanges.find(
                            (obj) => obj.type === "published"
                        ) as {
                            packageId: string;
                        };
                        setPackageId(publishedObject.packageId);
                    }
                    setIsPublishing(false);
                }
            },
        );
    };

    return (
        currentAccount ? (
            <div>
                <button className='button button--primary' onClick={onClick} disabled={isPublishing}>
                    {isPublishing ? "Publishing..." : "Publish"}
                </button>
                {packageId ? 
                    <div className="margin-top--md">
                        Congratulation to your first published package: <Link to={`${Networks.iota_move.explorerUrl}/object/${packageId}`}>{packageId}</Link>
                    </div> : null
                }
            </div>
        ) : (
            <code>No Wallet connected</code>
        )
    );
}
