// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import Button from './Button';
import { usePopups } from '@/hooks';
import type { CoinStruct } from '@iota/iota.js/client';
import { SendCoinPopup } from '../Popup';

interface SendCoinButtonProps {
    address: string;
    coin: CoinStruct;
}

function SendCoinButton({ address, coin }: SendCoinButtonProps): JSX.Element {
    const { openPopup, closePopup } = usePopups();

    const openSendTokenPopup = () => {
        openPopup(<SendCoinPopup coin={coin} senderAddress={address} onClose={closePopup} />);
    };

    return <Button onClick={openSendTokenPopup}>Send</Button>;
}

export default SendCoinButton;
