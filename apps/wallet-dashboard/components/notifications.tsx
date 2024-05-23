// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useEffect, useState } from 'react';

import {
	Notification,
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

function Notification(props: { notification: Notification }): JSX.Element {
	const { notification } = props;
	const bgColor = NOTIFICATION_TYPE_TO_COLOR[notification.type];

	const [css, setCss] = useState<string>(
		`flex items-center justify-center text-center rounded-xl ${bgColor} mt-1 p-2 w-[300px] transition-opacity duration-[${ALERT_FADE_OUT_DURATION}] ease-in opacity-100`,
	);

	const clearNotification = useNotificationStore((state) => state.clearNotification);

	useEffect(() => {
		const fadeOutputTimeout = setTimeout(() => {
			setCss((css) => `${css.replace('opacity-100', 'opacity-0')}`);
		}, notification.duration - ALERT_FADE_OUT_DURATION);

		const removeTimeout = setTimeout(() => {
			clearNotification(notification.index);
		}, notification.duration);

		return () => {
			clearTimeout(fadeOutputTimeout);
			clearTimeout(removeTimeout);
		};
	}, [notification, clearNotification]);

	return <div className={css}>{notification.message}</div>;
}

export default function Notifications(): JSX.Element {
	const notifications = useNotificationStore((state) => state.notifications);
	return (
		<div className="absolute top-1 right-2 z-50">
			{notifications.map((notification) => (
				<Notification key={`${notification.index}`} notification={notification} />
			))}
		</div>
	);
}
