# Move on IOTA Capture the Flag (CTF)

## Trying the Challenges

The challenges are already deployed on the IOTA Testnet. To get started, you need to start with reading the [CTF introduction](../../docs/content/developer/iota-move-ctf/introduction.mdx), follow its steps to interact with the challenges, and capture the flags.

## Deploying the challneges yourself (Not Required)

If you want to experiment further with the contracts or deploy the challenges in your own environment, you can set them up yourself using the provided `deploy.py` script. While this is not required to participate in the CTF, deploying the challenges locally can help you explore the contracts in a controlled environment.

To deploy the challenges yourself:

1. **Prepare the Environment:**
   - Ensure you have Python 3 installed.
   - Install the IOTA CLI tool and ensure it is available in your system's `PATH`.

2. **Run the Deployment Script:**
   - Inside the CTF directory, run the [`deploy.py`](../../../../examples/ctf/deploy.py).
   - The script requires two arguments:
     1. The path to the `iota` binary (the CLI tool you installed earlier). You can find the path by running `which iota` in your terminal in Linux or macOS, or `where iota` in Windows.
     2. The full path to the CTF repository.
   - Example command:
     ```bash
     python3 deploy.py /path/to/iota /path/to/ctf/repository
     ```

3. **Interact with the Deployed Contracts:**
   - Once the challenges are deployed in your local environment, you can interact with them in a similar way as on the devnet. You’ll be able to submit transactions, test edge cases, and attempt to capture the flags using the deployed contracts.

This setup allows you to modify the contracts and experiment with different scenarios.
