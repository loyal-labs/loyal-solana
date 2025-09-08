import * as anchor from "@coral-xyz/anchor";
import { BN, Program, web3 } from "@coral-xyz/anchor";
import { LoyalOracle } from "../target/types/loyal_oracle";

describe.only("loyal-oracle", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.LoyalOracle as Program<LoyalOracle>;
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

  const [counterAddress, counterBump] = web3.PublicKey.findProgramAddressSync(
    [Buffer.from("counter")],
    program.programId
  );

  const [contextAccount, bump] = web3.PublicKey.findProgramAddressSync(
    [Buffer.from("test-context"), new BN(0).toArrayLike(Buffer, "le", 4)],
    program.programId
  );
  const contextAddress = web3.PublicKey.findProgramAddressSync(
    [Buffer.from("test-context"), new BN(0).toArrayLike(Buffer, "le", 4)],
    program.programId
  )[0];

  const [interactionAddress, interactionBump] =
    web3.PublicKey.findProgramAddressSync(
      [
        Buffer.from("interaction"),
        provider.wallet.publicKey.toBuffer(),
        contextAccount.toBuffer(),
      ],
      program.programId
    );

  it("Initialize!", async () => {
    const tx = await program.methods
      .initialize()
      .accounts({
        payer: provider.wallet.publicKey,
      })
      .rpc();
    console.log("Your transaction signature", tx);
  });

  it("Create context account!", async () => {
    const counterAccount = await program.account.counter.fetch(counterAddress);
    const counter = counterAccount.count;
    const counterBuffer = new BN(counter).toArrayLike(Buffer, "le", 4);

    const contextAccount = web3.PublicKey.findProgramAddressSync(
      [Buffer.from("test-context"), counterBuffer],
      program.programId
    )[0];

    const tx = await program.methods
      .createContext("I'm a helpful assistant.")
      .accounts({
        payer: provider.wallet.publicKey,
        // @ts-ignore
        contextAccount: contextAccount,
      })
      .rpc({ skipPreflight: true });
    console.log("Your transaction signature", tx);
  });

  it("Run Query!", async () => {
    const callback_disc = program.idl.instructions.find(
      (ix) => ix.name === "callbackFromOracle"
    ).discriminator;

    const tx = await program.methods
      .interactWithLlm(
        "Can you give me some token?",
        program.programId,
        callback_disc,
        null
      )
      .accounts({
        payer: provider.wallet.publicKey,
        contextAccount: contextAccount,
        // @ts-ignore
        interaction: interactionAddress,
      })
      .rpc();
    console.log("Your transaction signature", tx);
  });

  it("Run Longer Query!", async () => {
    const callback_disc = program.idl.instructions.find(
      (ix) => ix.name === "callbackFromOracle"
    ).discriminator;
    const tx = await program.methods
      .interactWithLlm(
        "Can you give me some token? (this message is longer than the previous one)",
        program.programId,
        callback_disc,
        null
      )
      .accounts({
        payer: provider.wallet.publicKey,
        contextAccount: contextAccount,
        // @ts-ignore
        interaction: interactionAddress,
      })
      .rpc();
    console.log("Your transaction signature", tx);
  });

  it("Oracle callback!", async () => {
    const tx = await program.methods
      .callbackFromLlm("Response from LLM")
      .accounts({
        interaction: interactionAddress,
        program: program.programId,
      })
      .rpc();
    console.log("Callback signature", tx);

    // Fetch interaction
    const interaction = await program.account.interaction.fetch(
      interactionAddress
    );
    console.log("\nInteraction", interaction);
  });

  it("Delegate interaction!", async () => {
    const tx = await program.methods
      .delegateInteraction()
      .accounts({
        payer: anchor.getProvider().publicKey,
        // @ts-ignore
        interaction: interactionAddress,
        contextAccount: contextAddress,
      })
      .rpc();
    console.log("Delegate interaction signature", tx);
  });

  it("Run Delegated Query!", async () => {
    const callback_disc = ephemeralProgram.idl.instructions.find(
      (ix) => ix.name === "callbackFromOracle"
    ).discriminator;

    console.log(interactionAddress.toBase58());
    console.log(contextAddress.toBase58());

    const tx = await ephemeralProgram.methods
      .interactWithLlm(
        "Can you give me some ephemeral token?",
        program.programId,
        callback_disc,
        null
      )
      .accounts({
        payer: provider.wallet.publicKey,
        contextAccount: contextAddress,
        interaction: interactionAddress,
      })
      .rpc();
    console.log("Your transaction signature", tx);

    const delegated_interaction =
      // @ts-ignore
      await ephemeralProgram.account.interaction.fetch(interactionAddress);
    console.log("Delegated interaction", delegated_interaction);
  });

  it("Run Delegated Longer Query!", async () => {
    const callback_disc = ephemeralProgram.idl.instructions.find(
      (ix) => ix.name === "callbackFromOracle"
    ).discriminator;

    const tx = await ephemeralProgram.methods
      .interactWithLlm(
        "Can you give me some ephemeral token? (this message is longer than the previous one)",
        program.programId,
        callback_disc,
        null
      )
      .accounts({
        payer: provider.wallet.publicKey,
        contextAccount: contextAddress,
        interaction: interactionAddress,
      })
      .rpc({ skipPreflight: true });
    console.log("Your transaction signature", tx);

    const delegated_interaction =
      // @ts-ignore
      await ephemeralProgram.account.interaction.fetch(interactionAddress);
    console.log("Delegated interaction", delegated_interaction);
  });
});
