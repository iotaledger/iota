// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import st from './TransactionRequest.module.scss';

export interface MiniNFTProps {
    size?: 'xs' | 'sm';
    url: string;
    name?: string | null;
}

export function MiniNFT({ size = 'sm', url, name }: MiniNFTProps) {
    const sizes = size === 'xs' ? st.nftImageTiny : st.nftImageSmall;
    return <img src={url} className={sizes} alt={name || 'Nft Image'} />;
}
