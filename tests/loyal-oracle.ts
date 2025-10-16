import * as anchor from "@coral-xyz/anchor";
import { BN, Program, web3 } from "@coral-xyz/anchor";
import { expect } from "chai";
import { LoyalOracle } from "../target/types/loyal_oracle";

function getInteractionPda(
  contextAddress: web3.PublicKey,
  program: Program<LoyalOracle>
) {
  const nextId = new BN(0);
  const nextIdLe8 = nextId.toArrayLike(Buffer, "le", 8);
  const [interactionPda] = web3.PublicKey.findProgramAddressSync(
    [Buffer.from("interaction"), contextAddress.toBuffer(), nextIdLe8],
    program.programId
  );
  return { interactionPda, nextId };
}

describe.only("loyal-oracle", () => {
  const baseProvider = anchor.AnchorProvider.env();
  // anchor.setProvider(provider);

  const oracleKeypair: web3.Keypair = (baseProvider.wallet as any).payer;
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
        "https://devnet-us.magicblock.app/",
      {
        wsEndpoint:
          process.env.EPHEMERAL_WS_ENDPOINT ||
          "wss://devnet-us.magicblock.app/",
      }
    ),
    // anchor.Wallet.local()
    testWallet,
    provider.opts
  );
  console.log("providerEphemeralRollup", providerEphemeralRollup);
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

    const baseSignature = await provider.connection.requestAirdrop(
      oracleKeypair.publicKey,
      web3.LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(
      {
        signature: baseSignature,
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
      .createContext()
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
    expect(contextAccountData.owner.toBase58()).to.equal(
      provider.wallet.publicKey.toBase58()
    );
    // BN: 0 = 0
    expect(contextAccountData.nextInteraction.toNumber()).to.equal(0);
  });

  it("Interact with LLM!", async () => {
    // inputs
    const text = "Can you give me some token?";
    const callbackProgramId = program.programId;

    // Use the discriminator of the *target* callback ix (the program the oracle CPI will call)
    const callbackIx = program.idl.instructions.find(
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
      program.programId
    );
    console.log("Interaction PDA", interactionPda);

    // 3) Fire the instruction
    const tx = await program.methods
      .interactWithLlm(
        nextId,
        null,
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

    const interactionAccount = await program.account.interaction.fetch(
      interactionPda
    );
    console.log("Interaction account", interactionAccount);
  });

  it("Edit interaction with LLM!", async () => {
    // inputs
    const text = "Can you give me some token now?";
    const callbackProgramId = program.programId;

    // Use the discriminator of the *target* callback ix (the program the oracle CPI will call)
    const callbackIx = program.idl.instructions.find(
      (ix) => ix.name === "callbackFromOracle"
    );
    const callbackDiscriminator = callbackIx.discriminator as number[];

    // 2) Derive the *next* Interaction PDA for this context
    const { nextId, interactionPda } = getInteractionPda(
      contextAddress,
      program
    );
    console.log("Interaction PDA", interactionPda);

    // 3) Fire the instruction
    const tx = await program.methods
      .interactWithLlm(
        nextId,
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

    const interactionAccount = await program.account.interaction.fetch(
      interactionPda
    );
    console.log("Interaction account", interactionAccount);
  });

  it("Callback from LLM!", async () => {
    const callbackIx = program.idl.instructions.find(
      (ix) => ix.name === "callbackFromOracle"
    );
    const callbackDiscriminator = callbackIx.discriminator as number[];

    const [identityPda] = web3.PublicKey.findProgramAddressSync(
      [Buffer.from("identity")],
      program.programId
    );

    const { nextId, interactionPda } = getInteractionPda(
      contextAddress,
      program
    );

    const response = "Here is your ephemeral token: 1234567890";
    const isProcessed = false;

    const tx = await program.methods
      .callbackFromLlm(response, isProcessed)
      .accounts({
        oracleSigner: provider.wallet.publicKey,
        // @ts-ignore
        identity: identityPda,
        interaction: interactionPda,
        program: program.programId,
      })
      // .signers([oracleKeypair])
      .rpc();
    console.log("Callback from LLM", tx);
  });

  it("Delegate interaction!", async () => {
    const { nextId, interactionPda } = getInteractionPda(
      contextAddress,
      program
    );

    console.log("Interaction PDA", interactionPda);

    const tx = await program.methods
      .delegateInteraction(nextId)
      .accounts({
        payer: provider.wallet.publicKey,
        // @ts-ignore
        interaction: interactionPda,
        contextAccount: contextAddress,
      })
      .rpc();
    console.log("Delegate interaction signature", tx);
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

  it("Interact with LLM in ephemeral rollup!", async () => {
    // inputs
    const text = "Can you give me some ephemeral token?";
    const callbackProgramId = ephemeralProgram.programId;

    // Use the discriminator of the *target* callback ix (the program the oracle CPI will call)
    const callbackIx = ephemeralProgram.idl.instructions.find(
      (ix) => ix.name === "callbackFromOracle"
    );
    const callbackDiscriminator = callbackIx.discriminator as number[];

    // 2) Derive the *next* Interaction PDA for this context
    const { nextId, interactionPda } = getInteractionPda(
      contextAddress,
      ephemeralProgram
    );

    console.log("Interaction PDA", interactionPda);

    // 3) Fire the instruction
    const tx = await ephemeralProgram.methods
      .interactWithLlm(
        nextId,
        text,
        program.programId,
        callbackDiscriminator,
        null // Option<Vec<AccountMeta>> = None
      )
      .accounts({
        payer: provider.wallet.publicKey,
        // @ts-ignore
        interaction: interactionPda,
        contextAccount: contextAddress,
      })
      .rpc();
    console.log("Interact tx", tx);
  });

  it("Callback from LLM!", async () => {
    const callbackProgramId = ephemeralProgram.programId;

    const [identityPda] = web3.PublicKey.findProgramAddressSync(
      [Buffer.from("identity")],
      ephemeralProgram.programId
    );

    const { interactionPda } = getInteractionPda(
      contextAddress,
      ephemeralProgram
    );

    const response = "Here is your ephemeral token: 1234567890";
    const isProcessed = false;

    const info = await provider.connection.getAccountInfo(
      ephemeralProgram.programId
    );
    console.log(
      "target executable?",
      info?.executable,
      "owner:",
      info?.owner?.toBase58()
    );

    const tx = await ephemeralProgram.methods
      .callbackFromLlm(response, isProcessed)
      .accounts({
        oracleSigner: oracleKeypair.publicKey,
        // @ts-ignore
        identity: identityPda,
        interaction: interactionPda,
        program: ephemeralProgram.programId,
      })
      .signers([oracleKeypair])
      .rpc({});
    console.log("Callback from LLM", tx);

    const logTx = await provider.connection.getTransaction(tx, {
      maxSupportedTransactionVersion: 0,
      commitment: "confirmed",
    });
    console.log(logTx?.meta?.logMessages);
  });
});
