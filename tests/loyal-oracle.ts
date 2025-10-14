import * as anchor from "@coral-xyz/anchor";
import { BN, Program, web3 } from "@coral-xyz/anchor";
import { expect } from "chai";
import { LoyalOracle } from "../target/types/loyal_oracle";

describe.only("loyal-oracle", () => {
  const baseProvider = anchor.AnchorProvider.env();
  const testKeypair = web3.Keypair.generate();
  const testWallet = new anchor.Wallet(testKeypair);
  const provider = new anchor.AnchorProvider(
    baseProvider.connection,
    testWallet,
    baseProvider.opts
  );
  anchor.setProvider(provider);

  const program = anchor.workspace.LoyalOracle as Program<LoyalOracle>;
  const providerEphemeralRollup = new anchor.AnchorProvider(
    new anchor.web3.Connection(
      process.env.EPHEMERAL_PROVIDER_ENDPOINT ||
        "https://devnet.magicblock.app/",
      {
        wsEndpoint:
          process.env.EPHEMERAL_WS_ENDPOINT || "wss://devnet.magicblock.app/",
      }
    ),
    testWallet,
    provider.opts
  );
  const ephemeralProgram = new Program<LoyalOracle>(
    program.idl,
    providerEphemeralRollup
  );

  const [contextAccount, bump] = web3.PublicKey.findProgramAddressSync(
    [Buffer.from("context"), provider.wallet.publicKey.toBuffer()],
    program.programId
  );
  const contextAddress = web3.PublicKey.findProgramAddressSync(
    [Buffer.from("context"), provider.wallet.publicKey.toBuffer()],
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

  before(async () => {
    const { blockhash, lastValidBlockHeight } =
      await provider.connection.getLatestBlockhash();
    const signature = await provider.connection.requestAirdrop(
      provider.wallet.publicKey,
      web3.LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(
      {
        signature,
        blockhash,
        lastValidBlockHeight,
      },
      "confirmed"
    );
  });

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
    const tx = await program.methods
      .createContext("I'm a helpful assistant.")
      .accounts({
        payer: provider.wallet.publicKey,
        // @ts-ignore
        contextAccount: contextAccount,
      })
      .rpc({ skipPreflight: true });
    console.log("Your transaction signature", tx);

    const contextAccountData = await program.account.contextAccount.fetch(
      contextAddress
    );
    console.log("Context account data", contextAccountData);
    expect(contextAccountData.text).to.equal("I'm a helpful assistant.");
    expect(contextAccountData.owner.toBase58()).to.equal(
      provider.wallet.publicKey.toBase58()
    );
    // BN: 0 = 0
    expect(contextAccountData.nextInteraction.toNumber()).to.equal(0);
  });

  it("Delegate context!", async () => {
    const tx = await program.methods
      .delegateContext()
      .accounts({
        payer: provider.wallet.publicKey,
        // @ts-ignore
        contextAccount: contextAddress,
      })
      .rpc();
    console.log("Delegate context signature", tx);
  });

  it("Interact with LLM!", async () => {
    // inputs
    const text = "Can you give me some token?";
    const callbackProgramId = ephemeralProgram.programId;

    // Use the discriminator of the *target* callback ix (the program the oracle CPI will call)
    const callbackIx = ephemeralProgram.idl.instructions.find(
      (ix) => ix.name === "callbackFromOracle"
    );
    const callbackDiscriminator = callbackIx.discriminator as number[];

    // 2) Derive the *next* Interaction PDA for this context
    const ctxBefore = await program.account.contextAccount.fetch(
      contextAddress
    );
    console.log("Context before", ctxBefore);
    const nextId = ctxBefore.nextInteraction as BN;
    const nextIdLe8 = nextId.toArrayLike(Buffer, "le", 8);

    const [interactionPda] = web3.PublicKey.findProgramAddressSync(
      [Buffer.from("interaction"), contextAddress.toBuffer(), nextIdLe8],
      ephemeralProgram.programId
    );
    console.log("Interaction PDA", interactionPda);

    // 3) Fire the instruction
    const tx = await ephemeralProgram.methods
      .interactWithLlm(
        text,
        callbackProgramId,
        callbackDiscriminator,
        null // Option<Vec<AccountMeta>> = None
      )
      .accounts({
        payer: provider.wallet.publicKey,
        // @ts-ignore
        interaction: interactionPda,
        contextAccount: contextAddress,
      })
      .rpc({ skipPreflight: true });
    console.log("Interact tx", tx);
  });

  // it("Interact with LLM!", async () => {
  //   const callbackDisc = program.idl.instructions.find(
  //     (ix) => ix.name === "callbackFromOracle"
  //   )!.discriminator;

  //   const prompt = "Can you give me some token?";
  //   const tx = await program.methods
  //     .interactWithLlm(prompt, program.programId, callbackDisc, null)
  //     .accounts({
  //       payer: provider.wallet.publicKey,
  //       // @ts-ignore
  //       interaction: interactionAddress,
  //       contextAccount: contextAddress,
  //     })
  //     .rpc();
  //   console.log("Interact with LLM signature", tx);

  //   const interactionAccount = await program.account.interaction.fetch(
  //     interactionAddress
  //   );
  //   expect(interactionAccount.text).to.equal(prompt);
  //   expect(interactionAccount.isProcessed).to.be.false;
  //   expect(interactionAccount.callbackProgramId.toBase58()).to.equal(
  //     program.programId.toBase58()
  //   );
  // });

  // it("Delegate interaction!", async () => {
  //   const tx = await program.methods
  //     .delegateInteraction()
  //     .accounts({
  //       payer: anchor.getProvider().publicKey,
  //       // @ts-ignore
  //       interaction: interactionAddress,
  //       contextAccount: contextAddress,
  //     })
  //     .rpc();
  //   console.log("Delegate interaction signature", tx);
  // });

  // it("Interact with LLM in ephemeral rollup!", async () => {
  //   const callbackDisc = ephemeralProgram.idl.instructions.find(
  //     (ix) => ix.name === "callbackFromOracle"
  //   )!.discriminator;

  //   const prompt = "Can you give me some token?";
  //   const tx = await ephemeralProgram.methods
  //     .interactWithLlm(prompt, program.programId, callbackDisc, null)
  //     .accounts({
  //       payer: provider.wallet.publicKey,
  //       // @ts-ignore
  //       interaction: interactionAddress,
  //       contextAccount: contextAddress,
  //     })
  //     .rpc();
  //   console.log("Interact with LLM signature", tx);

  //   const interactionAccount = await ephemeralProgram.account.interaction.fetch(
  //     interactionAddress
  //   );
  //   expect(interactionAccount.text).to.equal(prompt);
  //   expect(interactionAccount.isProcessed).to.be.false;
  //   expect(interactionAccount.callbackProgramId.toBase58()).to.equal(
  //     ephemeralProgram.programId.toBase58()
  //   );
  // });
});
