import * as anchor from "@coral-xyz/anchor";
import { BN, Program, web3 } from "@coral-xyz/anchor";
import { expect } from "chai";
import { LoyalOracle } from "../target/types/loyal_oracle";

describe.only("loyal-oracle", () => {
  const baseProvider = anchor.AnchorProvider.env();
  // anchor.setProvider(provider);
  const cmk = web3.Keypair.generate().publicKey;
  const txId = web3.Keypair.generate().publicKey;

  const chatId = new BN(0);
  const STATUS_PENDING = 1;
  const STATUS_DONE = 2;
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

  const ephemeralProgram = new Program<LoyalOracle>(
    program.idl,
    providerEphemeralRollup
  );

  const [contextAccount, contextBump] = web3.PublicKey.findProgramAddressSync(
    [Buffer.from("context"), provider.wallet.publicKey.toBuffer()],
    program.programId
  );

  const [chatAddress, chatBump] = web3.PublicKey.findProgramAddressSync(
    [
      Buffer.from("chat"),
      contextAccount.toBuffer(),
      chatId.toArrayLike(Buffer, "le", 8),
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
      .rpc({ skipPreflight: true });
    console.log("Your transaction signature", tx);
  });

  it("Create Context!", async () => {
    const tx = await program.methods
      .createContext()
      .accounts({
        payer: provider.wallet.publicKey,
      })
      .rpc({ skipPreflight: true });
    console.log("Your transaction signature", tx);

    const context = await program.account.contextAccount.fetch(contextAccount);
    console.log("context", context);
  });

  it("Create Chat!", async () => {
    const chatId = new BN(0);

    const tx = await program.methods
      .createChat(chatId, cmk, txId)
      .accounts({
        payer: provider.wallet.publicKey,
        contextAccount: contextAccount,
      })
      .rpc({ skipPreflight: true });
    console.log("Your transaction signature", tx);

    const chat = await program.account.chat.fetch(chatAddress);
    console.log("chat", chat);
  });

  it("Get DEK for user!", async () => {
    const eventP = new Promise<{
      name: string;
      data: { chat: anchor.web3.PublicKey; chatId: anchor.BN; dek: Buffer[] };
      slot: number;
      signature?: string;
    }>(async (resolve) => {
      const listener = await program.addEventListener(
        "dekResponse",
        (event, slot, signature) => {
          program.removeEventListener(listener).catch(() => {});
          resolve({ name: "dekResponse", data: event as any, slot, signature });
        }
      );
    });

    await program.methods
      .getDek()
      .accounts({
        caller: provider.wallet.publicKey,
        chat: chatAddress,
      })
      .rpc();

    const evt = await eventP;
    const userDek = evt.data.dek;
    console.log("userDek", userDek);
    await new Promise((resolve) => setTimeout(resolve, 500));

    const eventPO = new Promise<{
      name: string;
      data: { chat: anchor.web3.PublicKey; chatId: anchor.BN; dek: Buffer[] };
      slot: number;
      signature?: string;
    }>(async (resolve) => {
      const listener = await program.addEventListener(
        "dekResponse",
        (event, slot, signature) => {
          program.removeEventListener(listener).catch(() => {});
          resolve({ name: "dekResponse", data: event as any, slot, signature });
        }
      );
    });

    await program.methods
      .getDek()
      .accounts({
        caller: oracleKeypair.publicKey,
        chat: chatAddress,
      })
      .signers([oracleKeypair])
      .rpc();

    console.log("awaiting oracle dek");

    const evtO = await eventPO;
    const oracleDek = evtO.data.dek;
    console.log("oracleDek", oracleDek);
  });

  it("Update Status From Oracle!", async () => {
    const tx = await program.methods
      .updateStatus(STATUS_DONE)
      .accounts({
        caller: provider.wallet.publicKey,
        chat: chatAddress,
      })
      .rpc({ skipPreflight: true });
    console.log("Your transaction signature", tx);
    let chat = await program.account.chat.fetch(chatAddress);
    expect(chat.status).to.equal(STATUS_DONE);

    const txOracle = await program.methods
      .updateStatus(STATUS_DONE)
      .accounts({
        caller: oracleKeypair.publicKey,
        chat: chatAddress,
      })
      .signers([oracleKeypair])
      .rpc({ skipPreflight: true });
    console.log("Your transaction signature", txOracle);

    chat = await program.account.chat.fetch(chatAddress);
    expect(chat.status).to.equal(STATUS_DONE);
  });

  // it("Delegate Chat!", async () => {
  //   const tx = await program.methods
  //     .delegateChat(chatId)
  //     .accounts({
  //       payer: provider.wallet.publicKey,
  //       // @ts-ignore
  //       chat: chatAddress,
  //       contextAccount: contextAccount,
  //     })
  //     .rpc({ skipPreflight: true });
  //   console.log("Your transaction signature", tx);
  // });
});
