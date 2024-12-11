/**
 * SWIZZLED VERSION: 3.5.2
 * REASONS:
 *  - Extend ComponentTypes with custom-WalletConnectButton
 *  - Extend ComponentTypes with custom-NetworkSelector
 */
import ComponentTypes from '@theme-original/NavbarItem/ComponentTypes';
import WalletConnectButton from '@site/src/components/WalletConnectButton';
import NetworkSelector from '@site/src/components/NetworkSelector';

export default {
  ...ComponentTypes,
  'custom-WalletConnectButton': WalletConnectButton,
  'custom-NetworkSelector': NetworkSelector,
};
