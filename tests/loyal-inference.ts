import * as anchor from "@coral-xyz/anchor";
import { Program, web3 } from "@coral-xyz/anchor";
import { LoyalInference } from "../target/types/loyal_inference";
import { Connection, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { expect } from "chai";
import { getClosestValidator } from "@magicblock-labs/ephemeral-rollups-sdk";

const SEED_TEST_PDA = "randomized-seed";

describe("loyal-inference", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.loyal_inference as Program<LoyalInference>;

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

  before(async function () {
    const balance = await provider.connection.getBalance(
      anchor.Wallet.local().publicKey
    );
    console.log("Current balance is", balance / LAMPORTS_PER_SOL, " SOL", "\n");
  });

  it("Initialize chat on Solana", async () => {
    const tx = await program.methods.initialize().rpc();
    console.log("Initialize: ", tx);
  });

  it("Delegate chat to ER", async () => {
    const validator = await getClosestValidator(provider.connection);
    const tx = await program.methods
      .delegate({
        commitFrequencyMs: 30_000,
        validator: validator,
      })
      .rpc();
    console.log("Delegate: ", tx);
  });

  it("Query model on ER", async () => {
    const msg = Buffer.from("hello");
    const tx = await ephemeralProgram.methods.queryDelegated(msg).rpc();
    console.log("Query: ", tx);
  });

  it("Query model multiple times", async () => {
    let attempts = 0;
    const startTime = Date.now();

    while (attempts < 10) {
      const attemptStartTime = Date.now();
      const msg = Buffer.from("hello");
      const tx = await ephemeralProgram.methods.queryDelegated(msg).rpc();
      console.log("Query: ", tx);
      attempts++;
      const duration = Date.now() - attemptStartTime;
      console.log(`Attempt ${attempts} took ${duration}ms`);
    }
    const totalTime = Date.now() - startTime;
    console.log(`Total time: ${totalTime}ms`);
  });

  it("Undelegate chat from ER", async () => {
    const tx = await ephemeralProgram.methods.undelegate().rpc();
    console.log("Undelegate: ", tx);
  });
});
