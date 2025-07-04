'use client';

import React from 'react';
import { CookieManagerProvider } from '@boxfish-studio/react-cookie-manager';

export function ContextProviders({ children }: React.PropsWithChildren): React.JSX.Element {
	return <CookieManagerProvider>{children}</CookieManagerProvider>;
}
