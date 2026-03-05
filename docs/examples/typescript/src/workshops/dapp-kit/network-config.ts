import { getFullnodeUrl } from "@iota/iota-sdk/client";
import { createNetworkConfig } from "@iota/dapp-kit";

const { networkConfig, useNetworkVariable } = createNetworkConfig({
  devnet: {
    url: getFullnodeUrl("devnet"),
    variables: {
      packageId: "",
      voucherShopObject: ""
    }
  },
  testnet: {
    url: getFullnodeUrl("testnet"),
    variables: {
      packageId: "",
      voucherShopObject: ""
    }
  }
});

export { useNetworkVariable, networkConfig };
