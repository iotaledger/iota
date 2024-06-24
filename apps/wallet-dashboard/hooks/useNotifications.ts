// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNotificationStore } from '@/stores/notificationStore';

export function useNotifications() {
    const addNotification = useNotificationStore((state) => state.addNotification);
    return { addNotification };
}
