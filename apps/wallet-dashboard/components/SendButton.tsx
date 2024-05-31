// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import Button from './Button';
import { usePopups } from '@/hooks';
import type { CoinStruct } from '@mysten/sui.js/client';
import { SendCoinPopup } from './Popup';

interface SendButtonProps {
    address: string | undefined;
    coin: CoinStruct;
}

function SendButton({ address, coin }: SendButtonProps): JSX.Element {
    const { openPopup, closePopup } = usePopups();

    const openSendTokenPopup = (coin: CoinStruct) => {
        if (address && coin) {
            openPopup(<SendCoinPopup coin={coin} senderAddress={address} onClose={closePopup} />);
        }
    };

    return <Button onClick={() => openSendTokenPopup(coin)}>Send</Button>;
}

export default SendButton;
