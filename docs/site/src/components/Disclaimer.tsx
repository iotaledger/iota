'use client';

import React from 'react';
import { CookieManager, type SKCMConfiguration } from '@boxfish-studio/react-cookie-manager';

export default function Disclaimer(): React.JSX.Element {
	const configuration: SKCMConfiguration = {
		disclaimer: {
			title: 'This website uses cookies',
			body: 'We use cookies to improve your experience. You can manage your preferences below. ',
			policyUrl: '/cookie-policy',
		},
		services: {
			googleAnalytics4Id: 'G-SEE2W8WK21'
		},
		theme: {
			primary: 'var(--iota-blue)',
			dark: 'var(--iota-white)',
			medium: '#b0bfd9',
			light: 'var(--iota-black)'
		}
	};

	return <CookieManager configuration={configuration} />;
}
