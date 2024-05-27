// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useEffect, useState } from 'react';

import {
    NotificationData,
    NotificationType,
    useNotificationStore,
} from '@/stores/notificationStore';

const ALERT_FADE_OUT_DURATION = 700;

const NOTIFICATION_TYPE_TO_COLOR = {
    [NotificationType.Info]: 'bg-blue-500',
    [NotificationType.Success]: 'bg-green-500',
    [NotificationType.Error]: 'bg-red-500',
    [NotificationType.Warning]: 'bg-yellow-500',
};

function Notification(props: { notification: NotificationData }): JSX.Element {
    const clearNotification = useNotificationStore((state) => state.clearNotification);
    const [transition, setTransition] = useState('opacity-100');

    const { notification } = props;

    const bgColor = NOTIFICATION_TYPE_TO_COLOR[notification.type];

    useEffect(() => {
        const fadeOutputTimeout = setTimeout(() => {
            setTransition('opacity-0');
        }, notification.duration - ALERT_FADE_OUT_DURATION);

        const removeTimeout = setTimeout(() => {
            clearNotification(notification.index);
        }, notification.duration);

        return () => {
            clearTimeout(fadeOutputTimeout);
            clearTimeout(removeTimeout);
        };
    }, [notification, clearNotification]);

    return (
        <div
            className={`flex items-center justify-center rounded-xl text-center ${bgColor} mt-1 w-[300px] p-2 transition-opacity duration-[${ALERT_FADE_OUT_DURATION}] ease-in ${transition}`}
        >
            {notification.message}
        </div>
    );
}

export default function Notifications(): JSX.Element {
    const notifications = useNotificationStore((state) => state.notifications);
    return (
        <div className="absolute right-2 top-1 z-50">
            {notifications.map((notification) => (
                <Notification key={`${notification.index}`} notification={notification} />
            ))}
        </div>
    );
}
