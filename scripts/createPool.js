const { connect, transactions, keyStores } = require("near-api-js");
const fs = require("fs");
const path = require("path");
const homedir = require("os").homedir();

const CREDENTIALS_DIR = ".near-credentials";
// NOTE: replace "example" with your accountId
const CONTRACT_NAME = "nearnftamm.testnet";
const nftContractId = ""
const SENDER_ACCOUNT_ID = "test0.nearnftamm.testnet"

let credentialsPath = path.join(homedir, CREDENTIALS_DIR);
const keyStore = new keyStores.UnencryptedFileSystemKeyStore(credentialsPath);

const config = {
    keyStore,
    networkId: "testnet",
    nodeUrl: "https://rpc.testnet.near.org",
};

sendTransactions();

async function sendTransactions() {
    const near = await connect({ ...config, keyStore });
    console.log(config)
    const account = await near.account(SENDER_ACCOUNT_ID);

    await account.signAndSendTransaction({
        receiverId: "dai.fakes.testnet",
        actions: [
            transactions.functionCall("ft_transfer_call", Buffer.from(JSON.stringify({ receiver_id: CONTRACT_NAME, amount: "40000000000000000000", msg: "" })), 100000000000000, "1")
        ],
    });

    const result = await account.signAndSendTransaction({
        receiverId: USDC,
        actions: [
            transactions.functionCall("ft_transfer_call", Buffer.from(JSON.stringify({ receiver_id: CONTRACT_NAME, amount: "40000000", msg: "" })), 100000000000000, "1")
        ],
    });

    await account.signAndSendTransaction({
        receiverId: USDT,
        actions: [
            transactions.functionCall("ft_transfer_call", Buffer.from(JSON.stringify({ receiver_id: CONTRACT_NAME, amount: "40000000", msg: "" })), 100000000000000, "1")
        ],
    });

    await account.signAndSendTransaction({
        receiverId: CONTRACT_NAME,
        actions: [
            transactions.functionCall("add_stable_liquidity", Buffer.from(JSON.stringify({ pool_id: 0, amounts: ["40000000", "40000000", "40000000000000000000"], min_shares: "0" })), 100000000000000, "1")
        ],
    });

    console.log(result);
}