/**
 * SWIZZLED VERSION: 3.5.2
 * REASONS:
 *  - 
 */
import React from 'react';
import {
  ConnectButton,
  ConnectModal,
  useAccounts,
  useAutoConnectWallet,
  useConnectWallet,
  useCurrentAccount,
  useCurrentWallet,
  useDisconnectWallet,
  useIotaClient,
  useReportTransactionEffects,
  useSignAndExecuteTransaction,
  useSignPersonalMessage,
  useSignTransaction,
  useSwitchAccount,
  useWallets,
} from '@iota/dapp-kit';
import { Transaction } from '@iota/iota-sdk/transactions';

// Add react-live imports you need here
const ReactLiveScope = {
  React,
  ...React,

  ConnectButton,
  ConnectModal,
  useAccounts,
  useAutoConnectWallet,
  useConnectWallet,
  useCurrentAccount,
  useCurrentWallet,
  useDisconnectWallet,
  useIotaClient,
  useReportTransactionEffects,
  useSignAndExecuteTransaction,
  useSignPersonalMessage,
  useSignTransaction,
  useSwitchAccount,
  useWallets,

  Transaction,
};

export default ReactLiveScope;
