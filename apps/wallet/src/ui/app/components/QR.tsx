// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useEffect, useState } from 'react';
import { default as QrCode } from 'qrious';

export function QR({ data }: { data: string }) {
    const [qrImage, setQrImage] = useState('');
    const qrCode = new QrCode();

    useEffect(() => {
        if (data) {
            generateQrCode();
        }
    }, [data]);

    function generateQrCode(): void {
        qrCode.set({
            background: '#ffffff',
            foreground: '#000000',
            level: 'L',
            size: 150,
            value: data,
        });

        setQrImage(qrCode.toDataURL('image/png'));
    }
    return <img src={qrImage} alt={data} />;
}
