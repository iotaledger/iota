import { WalletProvider } from '@iota/dapp-kit';

declare function YourApp(): JSX.Element;

function App() {
    return (
        <WalletProvider>
            <YourApp />
        </WalletProvider>
    );
}

export default App;
