/**
 * SWIZZLED VERSION: 3.5.2
 * REASONS:
 *  - 
 */
import React from 'react';
import {
  ConnectButton,
  ConnectModal,
  IotaClientProvider,
  useAccounts,
  useAutoConnectWallet,
  useConnectWallet,
  useCurrentAccount,
  useCurrentWallet,
  useDisconnectWallet,
  useSignAndExecuteTransactionBlock,
  useSignPersonalMessage,
  useSignTransactionBlock,
  useSwitchAccount,
  useWallets,
  WalletProvider,
} from '@iota/dapp-kit';
import { getDefaultNetwork, getFullnodeUrl } from '@iota/iota-sdk/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

// Add react-live imports you need here
const ReactLiveScope = {
  React,
  ...React,
  ConnectButton,
  ConnectModal,
  IotaClientProvider,
  useAccounts,
  useAutoConnectWallet,
  useConnectWallet,
  useCurrentAccount,
  useCurrentWallet,
  useDisconnectWallet,
  useSignAndExecuteTransactionBlock,
  useSignPersonalMessage,
  useSignTransactionBlock,
  useSwitchAccount,
  useWallets,
  WalletProvider,

  getDefaultNetwork,
  getFullnodeUrl,
  QueryClient,
  QueryClientProvider,
};

export default ReactLiveScope;
