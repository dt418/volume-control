# Getting the signing credentials

The release pipeline reads five credential sets from repository secrets. This
page explains where each one comes from, what it costs, and how to install it.

> Since June 2023 the CA/Browser Forum requires code-signing private keys to
> live in FIPS 140-2 hardware/HSM. Most newly issued OV/EV certificates are
> therefore **not exportable as a `.pfx`**. Use a cloud-signing service
> (Azure Trusted Signing, SSL.com eSigner, DigiCert KeyLocker) or run a
> self-hosted runner with the hardware token attached.

## Windows — Authenticode

### Option A: cloud signing (recommended, works on GitHub-hosted runners)

**Azure Trusted Signing** (~US$10/month, requires an Azure subscription):

1. Create an Azure account and a Trusted Signing account.
2. Complete identity validation (organization, or individual in supported
   regions); this can take days.
3. Create a certificate profile (Public Trust).
4. Register an Entra app/service principal with the *Trusted Signing
   Certificate Profile Signer* role; create a client secret.
5. Configure the workflow to use `azure/trusted-signing-action` or
   `signtool sign /dlib Azure.CodeSigning.Dlib.dll /dmdf metadata.json`.

The current scripts accept a PFX or a certificate thumbprint, so switching to
Azure requires a small workflow change (a `trusted-signing` step before NSIS
and again for the setup EXE). Ask for it when the account exists.

Alternatives with the same shape: SSL.com eSigner, DigiCert KeyLocker.

### Option B: exportable PFX (if your CA provides one)

Not typical for new certificates, but valid for a certificate you already own
and are allowed to export:

```powershell
# 1. Export the certificate + key from the Windows store
$pwd = ConvertTo-SecureString 'choose-a-strong-password' -Force -AsPlainText
Export-PfxCertificate -Cert Cert:\CurrentUser\My\<THUMBPRINT> `
  -FilePath volumecontrol-signing.pfx -Password $pwd

# 2. Base64 it and store the secret (never commit the file)
[Convert]::ToBase64String([IO.File]::ReadAllBytes('volumecontrol-signing.pfx')) |
  gh secret set WINDOWS_CERT_PFX_BASE64
'choose-a-strong-password' | gh secret set WINDOWS_CERT_PASSWORD
Remove-Item volumecontrol-signing.pfx
```

Verify locally before tagging:

```powershell
$env:WINDOWS_CERT_PFX_BASE64 = '<base64>'
$env:WINDOWS_CERT_PASSWORD = '<password>'
pwsh scripts/sign-windows.ps1 -Files output\egui\volumecontrol-<ver>-windows-x64-setup.exe
pwsh scripts/sign-windows.ps1 -Files output\egui\volumecontrol-<ver>-windows-x64-setup.exe -VerifyOnly
```

`-VerifyOnly` must print `Successfully verified` and exit 0.

### Option C: self-hosted runner with a hardware token

Install the certificate on the runner, set `WINDOWS_CERT_THUMBPRINT` as the
secret, and keep the token attached. GitHub-hosted runners cannot see it.

## macOS — Developer ID + notarization

Requires the Apple Developer Program (US$99/year).

1. Enroll at <https://developer.apple.com/programs/> and wait for approval.
2. In Keychain Access on a Mac: *Certificate Assistant → Request a Certificate
   From a Certificate Authority*, save the CSR.
3. In the developer portal: *Certificates → + → Developer ID Application*,
   upload the CSR, download and install the certificate.
4. Export the certificate **with its private key** as `.p12` and set a password.
5. Base64 the `.p12` and store it:

```bash
base64 -i DeveloperID.p12 | gh secret set MACOS_CERT_P12_BASE64
printf '%s' 'p12-password' | gh secret set MACOS_CERT_PASSWORD
printf '%s' 'Developer ID Application: Your Name (TEAMID)' |
  gh secret set MACOS_SIGN_IDENTITY
```

6. Notarization credentials. Generate an **app-specific password** at
   <https://appleid.apple.com> (Sign-In and Security → App-Specific Passwords),
   then store:

```bash
printf '%s' 'team-id' | gh secret set MACOS_TEAM_ID
printf '%s' 'you@example.com' | gh secret set MACOS_APPLE_ID
printf '%s' 'abcd-efgh-ijkl-mnop' | gh secret set MACOS_APP_PASSWORD
```

Alternatively create a keychain profile once (`xcrun notarytool
store-credentials volumecontrol --apple-id … --team-id … --password …`) and
store its name as `MACOS_NOTARY_PROFILE` on a self-hosted runner.

Verify locally on a Mac:

```bash
codesign --verify --deep --strict --verbose=2 VolumeControl.app
spctl --assess --type execute --verbose=4 VolumeControl.app
xcrun stapler validate VolumeControl.app
```

## Linux — GPG (free)

```bash
gpg --full-generate-key            # RSA 4096, sign-only, no expiry (or 1y)
gpg --list-secret-keys --keyid-format LONG
# GPG_KEY_ID = the 40-char fingerprint
gpg --export-secret-keys --armor <FINGERPRINT> | base64 -w0 |
  gh secret set GPG_PRIVATE_KEY_BASE64
printf '%s' '<FINGERPRINT>' | gh secret set GPG_KEY_ID
```

Keep the private key backed up offline; CI imports it only for tag builds.

## Temporary unsigned macOS

While Apple credentials are pending, set the repository variable
`MACOS_SIGNING_DISABLED=true`. The macOS package then builds ad-hoc and logs a
warning instead of failing the signing gate; Windows Authenticode and Linux GPG
remain mandatory. Remove the variable when Developer ID credentials are added.

For a self-signed Windows certificate (development only) also set
`WINDOWS_ALLOW_UNTRUSTED_SIGNATURE=true`; the build signs and reports an
untrusted-root warning. Never use this for a public release.

## Checklist after provisioning

```bash
gh secret list --repo dt418/volume-control
```

Then create a test tag from a commit you intend to release. The pipeline will:

1. sign `volumecontrol-host.exe` and the NSIS setup, verify `/pa`;
2. sign the `.app` with hardened runtime, notarize and staple it;
3. sign `SHA256SUMS.txt` with GPG;
4. only then run the `Attach to release` job (`EGUI_RELEASE_PUBLISH=true`).

If any credential is missing the package job fails with
`REQUIRE_SIGNING=true but …` and nothing is uploaded — that is the expected
fail-closed behavior.
