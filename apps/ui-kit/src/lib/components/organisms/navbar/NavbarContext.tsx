// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import React, { createContext, useReducer, ReactNode, Dispatch } from 'react';

// Define the state and action types
interface State {
    isOpen: boolean;
}

export enum ActionType {
    ToggleNavbarOpen = 'TOGGLE_NAVBAR_OPEN',
}

type Action = { type: ActionType.ToggleNavbarOpen };

// Initial state
const initialState: State = {
    isOpen: false,
};

// Reducer function
function reducer(state: State, action: Action): State {
    switch (action.type) {
        case ActionType.ToggleNavbarOpen:
            return { ...state, isOpen: !state.isOpen };
        default:
            throw new Error(`Unhandled action type: ${action.type}`);
    }
}

// Create context
const NavbarContext = createContext<{ state: State; dispatch: Dispatch<Action> }>(
    {} as { state: State; dispatch: Dispatch<Action> },
);

// Create provider component
const NavbarProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
    const [state, dispatch] = useReducer(reducer, initialState);

    return <NavbarContext.Provider value={{ state, dispatch }}>{children}</NavbarContext.Provider>;
};

export { NavbarProvider, NavbarContext };
