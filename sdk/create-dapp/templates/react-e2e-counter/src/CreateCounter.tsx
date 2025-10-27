import { Transaction } from "@iota/iota-sdk/transactions";
import { Button, Container } from "@radix-ui/themes";
import { useSignAndExecuteTransaction, useIotaClient } from "@iota/dapp-kit";
import { useNetworkVariable } from "./networkConfig";
import ClipLoader from "react-spinners/ClipLoader";

export function CreateCounter({
  onCreated,
}: {
  onCreated: (id: string) => void;
}) {
  const counterPackageId = useNetworkVariable("counterPackageId");
  const iotaClient = useIotaClient();
  const {
    mutate: signAndExecute,
    isSuccess,
    isPending,
  } = useSignAndExecuteTransaction();

  function create() {
    const tx = new Transaction();

    tx.moveCall({
      arguments: [],
      target: `${counterPackageId}::counter::create`,
    });

    signAndExecute(
      {
        transaction: tx,
      },
      {
        onSuccess: async ({ digest }) => {
          const { effects } = await iotaClient.waitForTransaction({
            digest: digest,
            options: {
              showEffects: true,
            },
          });

          const objectId = effects?.created?.[0]?.reference?.objectId;
          if (objectId) {
            onCreated(objectId);
          } else {
            console.error("Failed to get objectId from transaction effects");
          }
        },
      },
    );
  }

  return (
    <Container>
      <Button
        size="3"
        onClick={() => {
          create();
        }}
        disabled={isSuccess || isPending}
      >
        {isSuccess || isPending ? <ClipLoader size={20} /> : "Create Counter"}
      </Button>
    </Container>
  );
}
