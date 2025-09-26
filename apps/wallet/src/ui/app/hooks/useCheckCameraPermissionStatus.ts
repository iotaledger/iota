// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { toast } from '@iota/core';
import { useEffect, useState } from 'react';

export function useCheckCameraPermissionStatus(): [PermissionState | null] {
    const [cameraPermissionStatus, setCameraPermissionStatus] = useState<PermissionState | null>(
        null,
    );

    useEffect(() => {
        (async () => {
            try {
                const permission = await navigator.permissions.query({
                    name: 'camera' as PermissionName,
                });
                permission.onchange = () => {
                    setCameraPermissionStatus(permission.state);
                };

                setCameraPermissionStatus(permission.state);
            } catch (_) {
                setCameraPermissionStatus('denied');
                toast.error('Could not check permission status!');
            }
        })();
    }, []);

    return [cameraPermissionStatus];
}
