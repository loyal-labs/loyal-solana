import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { LoyalInference } from "../target/types/loyal_inference";
import { LAMPORTS_PER_SOL } from "@solana/web3.js";
import { expect } from "chai";

const SEED_TEST_PDA = "loyal-pda-test";

describe("inference-pay", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const providerEphemeralRollup = new anchor.AnchorProvider(
    new anchor.web3.Connection(
      process.env.PROVIDER_ENDPOINT || "https://devnet-eu.magicblock.app/",
      {
        wsEndpoint:
          process.env.WS_ENDPOINT || "wss://devnet-eu.magicblock.app/",
      }
    ),
    anchor.Wallet.local()
  );
  console.log("Base Layer Connection: ", provider.connection._rpcEndpoint);
  console.log(
    "Ephemeral Rollup Connection: ",
    providerEphemeralRollup.connection._rpcEndpoint
  );
  console.log(`Current SOL Public Key: ${anchor.Wallet.local().publicKey}`);

  const program = anchor.workspace.loyal_inference as Program<LoyalInference>;
  const [pda] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from(SEED_TEST_PDA)],
    program.programId
  );
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
        //@ts-ignore
        chat: pda,
        user: provider.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc({ skipPreflight: true });
    const duration = Date.now() - startTime;
    console.log(`${duration}ms (Base Layer) Initialize txHash: ${txHash}`);
  });

  it("Send message to model", async () => {
    const startTime = Date.now();
    let msg = "Hello, how are you?";
    let msgBuffer = Buffer.from(msg);

    const txHash = await program.methods
      .messageIn(msgBuffer)
      .accounts({
        //@ts-ignore
        chat: pda,
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
});
