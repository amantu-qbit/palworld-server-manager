# Enabling signed auto-updates

The app ships with the auto-updater wired up, but it only goes live once you add
your **signing key**. Tauri signs each release with a private key and the app
verifies it with the matching public key — so users only ever install updates you
actually published. This is a **one-time** setup.

> The repo currently carries a throwaway **placeholder** public key so it builds and
> runs. You must replace it (step 2) with your own, or updates will fail
> verification.

## 1. Generate your keypair

```powershell
npm run tauri -- signer generate -w "$env:USERPROFILE\.tauri\palworld.key"
```

You'll be asked for a password — set a strong one and remember it. This writes:

- `…\.tauri\palworld.key` — the **private** key (keep secret, never commit)
- `…\.tauri\palworld.key.pub` — the **public** key
- and prints the public key to the terminal.

## 2. Put the public key in the app

Copy the **public key** (the contents of `palworld.key.pub`, one long base64 line)
into `src-tauri/tauri.conf.json`, replacing the placeholder:

```json
"plugins": {
  "updater": {
    "endpoints": ["https://github.com/amantu-qbit/palworld-server-manager/releases/latest/download/latest.json"],
    "pubkey": "PASTE_YOUR_PUBLIC_KEY_HERE"
  }
}
```

Commit and push that change.

## 3. Add the private key as GitHub Actions secrets

```powershell
Get-Content "$env:USERPROFILE\.tauri\palworld.key" -Raw | gh secret set TAURI_SIGNING_PRIVATE_KEY --repo amantu-qbit/palworld-server-manager
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo amantu-qbit/palworld-server-manager --body "the-password-you-chose"
```

## 4. Cut the release

```powershell
git tag v0.1.3
git push origin v0.1.3
```

The release workflow signs the installers, generates `latest.json`, and attaches
everything to the GitHub release. From then on the installed app checks for updates
on launch and can update itself in one click.

## Notes

- **Back up `palworld.key`** (e.g. a password manager). If you lose it you must
  generate a new keypair and every user has to reinstall by hand to pick up the new
  public key.
- `createUpdaterArtifacts` is enabled, so a **release build only succeeds once the
  secret is set** (steps 1–3). Set it before tagging.
- Users on **v0.1.2 or earlier** install v0.1.3 manually once; every version from
  v0.1.3 onward updates automatically.
