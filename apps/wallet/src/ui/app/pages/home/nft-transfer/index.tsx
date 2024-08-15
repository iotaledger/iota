// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useActiveAddress } from '_app/hooks/useActiveAddress';
import { Loading, NFTDisplayCard, Overlay } from '_components';
import { useOwnedNFT } from '_hooks';
import { useUnlockedGuard } from '_src/ui/app/hooks/useUnlockedGuard';
import { Navigate, useNavigate, useParams } from 'react-router-dom';

import { TransferNFTForm } from './TransferNFTForm';
import { isAssetTransferable } from '@iota/core';

function NftTransferPage() {
    const { nftId } = useParams();
    const address = useActiveAddress();
    // verify that the nft is owned by the user and is transferable
    const { data: ownedNFT, isPending: isNftLoading } = useOwnedNFT(nftId || '', address);
    const navigate = useNavigate();
    const isGuardLoading = useUnlockedGuard();
    const isPending = isNftLoading || isGuardLoading;
    return (
        <Overlay showModal={true} title="Send NFT" closeOverlay={() => navigate('/nfts')}>
            <div className="flex h-full w-full flex-col">
                <Loading loading={isPending}>
                    {nftId && !!ownedNFT && isAssetTransferable(ownedNFT) ? (
                        <>
                            <div className="mb-7.5">
                                <NFTDisplayCard objectId={nftId} wideView size="sm" />
                            </div>
                            <TransferNFTForm objectId={nftId} objectType={ownedNFT.type} />
                        </>
                    ) : (
                        <Navigate to="/" replace />
                    )}
                </Loading>
            </div>
        </Overlay>
    );
}

export default NftTransferPage;
