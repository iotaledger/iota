// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createBrowserRouter } from 'react-router-dom';

import Root from '../Root';
import Home from './Home';
import SingleKiosk from './SingleKiosk';

export const router = createBrowserRouter([
    {
        path: '/',
        element: <Root />,
        children: [
            {
                path: '',
                element: <Home />,
            },
            {
                path: '/kiosk/:id',
                element: <SingleKiosk />,
            },
        ],
    },
]);
