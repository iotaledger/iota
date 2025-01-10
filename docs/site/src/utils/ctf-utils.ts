import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';

export const handleChallengeSubmit = async ({
    inputText,
    expectedObjectType,
    nftName,
    challengeNumber,
    wallets,
    mutate,
    signAndExecuteTransaction,
    setLoading,
    setCoins,
    setError,
    setShowPopup,
    setDigest
}: any) => {
    setLoading(true);
    setError({
        status: 'success',
        description: '',
        title: '',
    });
    setCoins(null);

    try {
        const NETWORKS = {
            testnet: { url: getFullnodeUrl('testnet') },
        };
        const NFTPackageAddress = "0x61b31360fb89cae585b8cb593edde20dfc690a3f260c12693bbb8b33ebf4707d"
        const client = new IotaClient({ url: NETWORKS.testnet.url });
        const result = await client.getObject({ id: inputText, options: { showType: true } });

        if (result.data.type === expectedObjectType) {
            const message = 'Congratulations! You have successfully completed this level!';
            const wallet = wallets[0];

            mutate(
                { wallet },
                {
                    onSuccess: () => {
                        const tx = () => {
                            const tx = new Transaction();
                            const arg0 = new TextEncoder().encode(`Challenge_${challengeNumber}_${nftName}_NFT`);
                            const arg1 = new TextEncoder().encode('NFT Reward for completing challenge');
                            tx.setGasBudget(50000000);
                            tx.moveCall({
                                target: `${NFTPackageAddress}::CTF_NFT::mint_to_sender`,
                                arguments: [tx.pure.vector('u8', arg0), tx.pure.vector('u8', arg1)],
                            });
                            return tx;
                        };

                        signAndExecuteTransaction(
                            {
                                transaction: tx(),
                            },
                            {
                                onSuccess: ({ digest }: any) => {
                                    client.waitForTransaction({ digest, options: { showEffects: true } }).then(() => {
                                        setDigest(digest)
                                        setError({
                                            status: 'success',
                                            description: 'An NFT reward was minted and transferred to your IOTA wallet address for completing the challenge.',
                                            title: 'NFT Minted',
                                        });
                                        setCoins(message);
                                        setLoading(false);
                                        setShowPopup(true);
                                    });
                                },
                                onError: (error: any) => {
                                    setError({
                                        status: 'error',
                                        description: `Failed to execute transaction : ${error}`,
                                        title: 'Submission failed',
                                    });
                                    setLoading(false);
                                    setShowPopup(true);
                                },
                            }
                        );
                    },
                }
            );
        } else {
            setCoins('Invalid Flag Object Id. Please try again.');
        }
    } catch (err: any) {
        setError({
            status: 'error',
            description: err.message || 'An error occurred. Please try again.',
            title: 'Submission failed',
        });
        setLoading(false);
    }
};

export const handleMintLeapFrogSubmit = async ({
    nft,
    wallets,
    mutate,
    signAndExecuteTransaction,
    setLoading,
    setCoins,
    setError,
    setShowPopup,
}: any) => {
    setLoading(true);
    setError({
        status: 'success',
        description: '',
        title: '',
    });
    setCoins(null);

    try {
        const NETWORKS = {
            testnet: { url: getFullnodeUrl('testnet') },
        };
        const NFTPackageAddress = "0x649884331fa662235b2c06c6eb488e5327105dded1331f6b7541ef4fdbd9eeca"
        const client = new IotaClient({ url: NETWORKS.testnet.url });

            const message = 'Congratulations! You have successfully completed this level!';
            const wallet = wallets[0];

            mutate(
                { wallet },
                {
                    onSuccess: () => {
                        const tx = () => {
                            const tx = new Transaction();
                            const arg0 = new TextEncoder().encode(nft.name);
                            const arg1 = new TextEncoder().encode(nft.description);
                            const arg2 = new TextEncoder().encode(nft.url);
                            tx.setGasBudget(50000000);
                            tx.moveCall({
                                target: `${NFTPackageAddress}::leap_frog_nft::mint_to_sender`,
                                arguments: [tx.pure.vector('u8', arg0), tx.pure.vector('u8', arg1), tx.pure.vector('u8', arg2), tx.pure.address(nft.address)],
                            });
                            return tx;
                        };

                        signAndExecuteTransaction(
                            {
                                transaction: tx(),
                            },
                            {
                                onSuccess: ({ digest }: any) => {
                                    client.waitForTransaction({ digest, options: { showEffects: true } }).then(() => {
                                        setError({
                                            status: 'success',
                                            description: 'An NFT reward is minted to your IOTA wallet address upon completing the challenge.',
                                            title: 'NFT Minted',
                                        });
                                        setCoins(message);
                                        setLoading(false);
                                        setShowPopup(true);
                                    });
                                },
                                onError: (error: any) => {
                                    setError({
                                        status: 'error',
                                        description: `Failed to execute transaction : ${error}`,
                                        title: 'Submission failed',
                                    });
                                    setLoading(false);
                                    setShowPopup(true);
                                },
                            }
                        );
                    },
                }
            );
    } catch (err: any) {
        setError({
            status: 'error',
            description: err.message || 'An error occurred. Please try again.',
            title: 'Submission failed',
        });
        setLoading(false);
    }
};
