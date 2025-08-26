import * as anchor from "@coral-xyz/anchor";
import { Program, web3 } from "@coral-xyz/anchor";
import { LoyalInference } from "../target/types/loyal_inference";
import { Connection, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { expect } from "chai";
import { sendMagicTransaction } from "@magicblock-labs/ephemeral-rollups-sdk";

const SEED_TEST_PDA = "loyal-pda-test-dev";

describe("loyal-inference", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const provider = anchor.getProvider() as anchor.AnchorProvider;
  const program = anchor.workspace.loyal_inference as Program<LoyalInference>;
  const [pda] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from(SEED_TEST_PDA)],
    program.programId
  );
  const providerEphemeralRollup = new anchor.AnchorProvider(
    new anchor.web3.Connection(
      process.env.PROVIDER_ENDPOINT || "https://devnet.magicblock.app/",
      {
        wsEndpoint: process.env.WS_ENDPOINT || "wss://devnet.magicblock.app/",
      }
    ),
    anchor.Wallet.local()
  );
  const ephemeralProgram = new Program(program.idl, providerEphemeralRollup);

  console.log("Base Layer Connection: ", provider.connection.rpcEndpoint);
  console.log(
    "Ephemeral Rollup Connection: ",
    providerEphemeralRollup.connection.rpcEndpoint
  );
  console.log(`Current SOL Public Key: ${anchor.Wallet.local().publicKey}`);

  console.log("Program ID: ", program.programId.toString());
  console.log("Loyal PDA: ", pda.toString());

  before(async function () {
    const balance = await provider.connection.getBalance(
      anchor.Wallet.local().publicKey
    );
    console.log("Current balance is", balance / LAMPORTS_PER_SOL, " SOL", "\n");
  });

  it("Initialize chat on Solana", async () => {
    const startTime = Date.now();
    const txHash = await program.methods
      .initialize()
      .accounts({
        payer: anchor.getProvider().publicKey,
      })
      .rpc({ skipPreflight: true });
    const duration = Date.now() - startTime;
    console.log(`${duration}ms (Base Layer) Initialize txHash: ${txHash}`);

    const chat = await program.account.loyalChat.fetch(pda);
    expect(chat.msgIn.length).to.equal(0);
    expect(chat.msgOut.length).to.equal(0);
    expect(chat.processing).to.be.false;
    expect(chat.userTurn).to.be.true;
  });

  it("Send message to model", async () => {
    const startTime = Date.now();
    let msg = "Hello, how are you?";
    let msgBuffer = Buffer.from(msg);

    const txHash = await program.methods
      .messageIn(msgBuffer)
      .accounts({
        payer: anchor.getProvider().publicKey,
      })
      .rpc();
    const duration = Date.now() - startTime;
    console.log(`${duration}ms (Base Layer) Message in txHash: ${txHash}`);

    const chat = await program.account.loyalChat.fetch(pda);
    let msgIn = chat.msgIn;
    let msgInString = Buffer.from(msgIn).toString();
    expect(msgInString).to.equal(msg);
    expect(chat.processing).to.be.true;
    expect(chat.userTurn).to.be.false;
  });

  it("Send message from model to user", async () => {
    const startTime = Date.now();
    let msg = "I'm fine, thank you!";
    let msgBuffer = Buffer.from(msg);

    const txHash = await program.methods
      .messageOut(msgBuffer)
      .accounts({
        payer: anchor.getProvider().publicKey,
      })
      .rpc();
    const duration = Date.now() - startTime;
    console.log(`${duration}ms (Base Layer) Message out txHash: ${txHash}`);

    const chat = await program.account.loyalChat.fetch(pda);
    let msgOut = chat.msgOut;
    let msgOutString = Buffer.from(msgOut).toString();
    expect(msgOutString).to.equal(msg);
    expect(chat.processing).to.be.false;
    expect(chat.userTurn).to.be.true;
  });

  it("Delegate counter to ER", async () => {
    const start = Date.now();

    let tx = await program.methods
      .delegate()
      .accounts({
        payer: anchor.getProvider().publicKey,
      })
      .rpc();
    const duration = Date.now() - start;
    console.log(`${duration}ms delegate txHash: ${tx}`);
  });

  it("Undelegate chat from ER", async () => {
    const start = Date.now();
    let tx = await program.methods
      .undelegate()
      .accounts({
        payer: anchor.getProvider().publicKey,
      })
      .rpc();
    const duration = Date.now() - start;
    console.log(`${duration}ms undelegate txHash: ${tx}`);
  });
});
