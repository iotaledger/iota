// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useActiveAddress, useUnlockedGuard } from '_hooks';
import { Loading, NFTDisplayCard, Overlay } from '_components';
import { Navigate, useNavigate, useParams } from 'react-router-dom';
import { TransferNFTForm } from './TransferNFTForm';
import { useOwnedNFT, useIsAssetTransferable } from '@iota/core';

export function NftTransferPage() {
    const { nftId } = useParams();
    const address = useActiveAddress();
    // verify that the nft is owned by the user and is transferable
    const { data: ownedNFT, isPending: isNftLoading } = useOwnedNFT(nftId || '', address);
    const { data: isAssetTransferable, isLoading: isCheckingAssetTransferability } =
        useIsAssetTransferable(ownedNFT);
    const navigate = useNavigate();
    const isGuardLoading = useUnlockedGuard();
    const isPending = isNftLoading || isGuardLoading || isCheckingAssetTransferability;
    return (
        <Overlay showModal title="Send NFT" closeOverlay={() => navigate('/nfts')} showBackButton>
            <Loading loading={isPending}>
                <div className="flex h-full w-full flex-col gap-md">
                    {nftId && !!ownedNFT && isAssetTransferable ? (
                        <>
                            <div className="w-[172px] self-center">
                                <NFTDisplayCard objectId={nftId} wideView />
                            </div>
                            <TransferNFTForm objectId={nftId} objectType={ownedNFT.type} />
                        </>
                    ) : (
                        <Navigate to="/" replace />
                    )}
                </div>
            </Loading>
        </Overlay>
    );
}
