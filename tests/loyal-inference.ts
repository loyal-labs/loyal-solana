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
        commitment: "processed",
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

  it("Query model on Solana", async () => {
    const msg = Buffer.from("hello");
    const processing = true;
    const tx = await program.methods.query(msg, processing).rpc({
      commitment: "processed",
      skipPreflight: true,
    });
    console.log("Query: ", tx);
  });

  it("Measure latency for 10 queries", async () => {
    const start = Date.now();
    for (let i = 0; i < 10; i++) {
      const queryStart = Date.now();
      const msg = Buffer.from("hello");
      const processing = true;
      const tx = await program.methods.query(msg, processing).rpc({
        commitment: "processed",
        skipPreflight: true,
      });
      const queryEnd = Date.now();
      const queryDuration = queryEnd - queryStart;
      console.log(`Query ${i} duration: ${queryDuration}ms`);
    }
    const end = Date.now();
    const duration = end - start;
    console.log("Total duration: ", duration, "ms");
  });

  it("Delegate chat to ER", async () => {
    const tx = await program.methods.delegate().rpc();
    console.log("Delegate: ", tx);
  });

  it("Query model on ER", async () => {
    const msg = Buffer.from("hello");
    const processing = true;

    const tx = await ephemeralProgram.methods
      .queryDelegated(msg, processing)
      .rpc({
        commitment: "processed",
        skipPreflight: true,
      });

    console.log("Query: ", tx);
  });

  it("Measure latency for 10 queries on ER", async () => {
    const start = Date.now();
    for (let i = 0; i < 10; i++) {
      const queryStart = Date.now();
      const msg = Buffer.from("hello");
      const processing = true;
      const tx = await ephemeralProgram.methods
        .queryDelegated(msg, processing)
        .rpc({
          commitment: "processed",
          skipPreflight: true,
        });
      const queryEnd = Date.now();
      const queryDuration = queryEnd - queryStart;
      console.log(`Query ${i} duration: ${queryDuration}ms`);
    }
    const end = Date.now();
    const duration = end - start;
    console.log("Total duration: ", duration, "ms");
  });

  it("Undelegate chat from ER", async () => {
    const tx = await ephemeralProgram.methods.undelegate().rpc({
      commitment: "processed",
      skipPreflight: true,
    });
    console.log("Undelegate: ", tx);
  });
});
