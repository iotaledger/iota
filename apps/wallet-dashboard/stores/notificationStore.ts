// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { indexGenerator } from '@/lib/utils';
import { create } from 'zustand';

const indexGen = indexGenerator(200);

export enum NotificationType {
    Success = 'success',
    Error = 'error',
    Warning = 'warning',
    Info = 'info',
}

export type NotificationData = {
    index: number;
    message: string;
    type: NotificationType;
    duration: number;
};

interface NotificationsState {
    notifications: NotificationData[];
    addNotification: (message: string, type?: NotificationType, duration?: number) => void;
    clearNotification: (index: number) => void;
}

export const useNotificationStore = create<NotificationsState>()((set) => ({
    notifications: [],
    addNotification: (
        message: string,
        type: NotificationType = NotificationType.Success,
        duration: number = 3000,
    ) =>
        set((state) => {
            const index = indexGen.next().value;

            const newNotification: NotificationData = {
                index,
                message,
                type,
                duration,
            };

            return {
                notifications: [...state.notifications, newNotification],
            };
        }),
    clearNotification: (index: number) =>
        set((state) => {
            return {
                notifications: [
                    ...state.notifications.filter((notification) => notification.index !== index),
                ],
            };
        }),
}));
