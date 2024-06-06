import React from "react";
import ReactDOM from "react-dom/client";
import "@iota/dapp-kit/dist/index.css";
import "@radix-ui/themes/styles.css";

import { getFullnodeUrl } from "@iota/iota.js/client";
import {
  IotaClientProvider,
  WalletProvider,
  createNetworkConfig,
} from "@iota/dapp-kit";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Theme } from "@radix-ui/themes";
import App from "./App.tsx";

const queryClient = new QueryClient();

const { networkConfig } = createNetworkConfig({
  localnet: { url: getFullnodeUrl("localnet") },
  devnet: { url: getFullnodeUrl("devnet") },
  testnet: { url: getFullnodeUrl("testnet") },
  mainnet: { url: getFullnodeUrl("mainnet") },
});

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Theme appearance="dark">
      <QueryClientProvider client={queryClient}>
        <IotaClientProvider networks={networkConfig} defaultNetwork="devnet">
          <WalletProvider autoConnect>
            <App />
          </WalletProvider>
        </IotaClientProvider>
      </QueryClientProvider>
    </Theme>
  </React.StrictMode>,
);
