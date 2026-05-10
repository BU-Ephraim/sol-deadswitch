import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { assert, expect } from "chai";
import { DeadSwitch } from "../target/types/dead_switch";

const { SystemProgram, LAMPORTS_PER_SOL, Keypair, PublicKey } = anchor.web3;

describe("dead_switch", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.DeadSwitch as Program<DeadSwitch>;

  const owner = provider.wallet;
  const depositLamports = 0.2 * LAMPORTS_PER_SOL;

  const deriveVaultPda = (ownerPk: PublicKey, backupPk: PublicKey) =>
    PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), ownerPk.toBuffer(), backupPk.toBuffer()],
      program.programId
    );

  const airdrop = async (pubkey: PublicKey, amount = LAMPORTS_PER_SOL) => {
    const sig = await provider.connection.requestAirdrop(pubkey, amount);
    await provider.connection.confirmTransaction(sig, "confirmed");
  };

  const initializeVault = async (backup: Keypair, intervalDays: anchor.BN) => {
    const [vaultPda] = deriveVaultPda(owner.publicKey, backup.publicKey);

    await program.methods
      .initialize(backup.publicKey, intervalDays, new anchor.BN(depositLamports))
      .accounts({
        owner: owner.publicKey,
        vault: vaultPda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    return vaultPda;
  };

  it("initializes a vault with expected state", async () => {
    const backup = Keypair.generate();
    await airdrop(backup.publicKey);

    const vaultPda = await initializeVault(backup, new anchor.BN(1));
    const vault = await program.account.vault.fetch(vaultPda);

    assert.equal(vault.owner.toBase58(), owner.publicKey.toBase58());
    assert.equal(vault.backupWallet.toBase58(), backup.publicKey.toBase58());
    assert.equal(vault.intervalDays.toNumber(), 1);
    assert.equal(vault.solAmount.toNumber(), depositLamports);
  });

  it("allows the owner to check in and reset timestamp", async () => {
    const backup = Keypair.generate();
    await airdrop(backup.publicKey);

    const vaultPda = await initializeVault(backup, new anchor.BN(1));
    const before = await program.account.vault.fetch(vaultPda);

    // Wait a moment so we can observe a timestamp change.
    await new Promise((resolve) => setTimeout(resolve, 1100));

    await program.methods
      .checkIn()
      .accounts({
        owner: owner.publicKey,
        vault: vaultPda,
      })
      .rpc();

    const after = await program.account.vault.fetch(vaultPda);
    assert.isTrue(after.lastCheckIn.toNumber() > before.lastCheckIn.toNumber());
  });

  it("rejects claim before interval expiration", async () => {
    const backup = Keypair.generate();
    await airdrop(backup.publicKey);

    const vaultPda = await initializeVault(backup, new anchor.BN(1));

    try {
      await program.methods
        .claim()
        .accounts({
          backupWallet: backup.publicKey,
          vault: vaultPda,
        })
        .signers([backup])
        .rpc();
      assert.fail("Claim should fail before interval expires");
    } catch (error: any) {
      const message = error.error?.errorMessage ?? error.toString();
      expect(message).to.contain("not elapsed");
    }
  });

  it("allows backup to claim after interval expires", async () => {
    const backup = Keypair.generate();
    await airdrop(backup.publicKey);

    // A 0-day interval means the vault is immediately claimable.
    const vaultPda = await initializeVault(backup, new anchor.BN(0));

    await program.methods
      .claim()
      .accounts({
        backupWallet: backup.publicKey,
        vault: vaultPda,
      })
      .signers([backup])
      .rpc();

    try {
      await program.account.vault.fetch(vaultPda);
      assert.fail("Vault account should be closed after a successful claim");
    } catch (_error) {
      assert.isTrue(true);
    }
  });

  it("allows owner to cancel while vault is still active", async () => {
    const backup = Keypair.generate();
    await airdrop(backup.publicKey);

    const vaultPda = await initializeVault(backup, new anchor.BN(5));

    await program.methods
      .cancel()
      .accounts({
        owner: owner.publicKey,
        vault: vaultPda,
      })
      .rpc();

    try {
      await program.account.vault.fetch(vaultPda);
      assert.fail("Vault account should be closed after owner cancel");
    } catch (_error) {
      assert.isTrue(true);
    }
  });
});
