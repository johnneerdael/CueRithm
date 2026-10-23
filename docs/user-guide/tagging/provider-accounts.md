# Connect provider accounts

<p class="lead">Beatport, Traxsource, MusicBrainz, iTunes, Bandcamp and Deezer need nothing from you. SonoVault, Volumo, BPM Supreme and Discogs take a key, a token or an account. Connecting one takes about five minutes.</p>

**Before you start**, open **Settings → Automatic tagging**. If you have not turned tagging on yet, follow [Tag new tracks from Beatport and Traxsource](tag-new-tracks-automatically.md) first.

## Which providers need an account { #which-providers }

| Provider | What CueRithm asks for | Required? | Where to get it |
|---|---|---|---|
| SonoVault | A personal API key | Yes. Without one the provider stays unavailable. | The **Get a free API key** link in the setup panel. Verify the account's email address before you test. |
| Volumo | A personal API key | Yes | The **Create a Volumo API token** link in the setup panel |
| BPM Supreme | Your account email and password | Yes, and the account must pass **Test saved BPM Supreme connection** | Your BPM Supreme account |
| Discogs | A personal token | No. Without one Discogs runs anonymously at 25 requests a minute. A verified token allows 60. | A personal access token from your Discogs account's developer settings |
| Beatport, Traxsource, MusicBrainz, iTunes, Bandcamp, Deezer | Nothing | | |

You do not register a developer application with anyone. [Choose providers and fields to write](choose-providers.md#providers) shows what each provider can write and how fast CueRithm may call it.

## Connect a provider { #provider-setup }

1. Find the provider in the list on **Settings → Automatic tagging**. Its status line reads *Optional · setup required* (or *Anonymous · 25 requests/minute* for Discogs).
2. Choose **Set up** followed by the provider name, for example **Set up SonoVault**. If a credential is already saved, the button reads **Manage SonoVault**.
3. Open the sign-up link if you need a key, then paste it into the masked field: **SonoVault API key**, **Volumo API key**, **Discogs personal token**, or **BPM Supreme email** and **BPM Supreme password**.
4. Choose **Save SonoVault key**, **Save Volumo key**, **Save Discogs token** or **Save BPM Supreme account**. The field clears at once, and the key is never shown again.
5. Choose **Test saved SonoVault connection** (or the matching button for your provider). A **Cancel provider operation** button appears while the test runs.
6. When the status line reads *Connection verified*, tick **Use SonoVault** in the list and choose **Save settings**.

Saving a credential does not tick the provider and does not save your other settings. Those are two separate actions, so a typed key never changes your tagging rules by accident.

!!! warning "Gotcha: BPM Supreme and Volumo behave differently"
    **Use BPM Supreme** stays disabled until the connection test succeeds. **Volumo** works only for files that already carry an ISRC (or a saved Volumo track ID with **Match by embedded IDs** on), because it cannot search by artist and title.

## What the test does { #tests }

| Provider | What **Test saved … connection** does |
|---|---|
| SonoVault | Uses one metadata credit. It never uploads audio and never uses recognition. |
| Volumo | Looks up one sample ISRC. If the sample is unavailable, the key stays *unverified*, not rejected. |
| Discogs | Checks the token against your account, which lifts the limit from 25 to 60 requests a minute. |
| BPM Supreme | Signs in with the saved account. |

## Where your credentials are kept { #storage }

CueRithm stores keys, tokens and passwords in your system credential store: Keychain on macOS, Credential Manager on Windows, and the Secret Service (GNOME Keyring or KWallet, for example) on Linux. They are not written to CueRithm's settings, they never appear in [presets](../files/presets.md) or backups, and CueRithm does not read key files from your disk.

Secrets go only to the provider they belong to. Anything you type that you do not save is cleared when you leave the panel.

To revoke a credential, open **Manage …** and choose **Remove … key** (or **… account**). A test that is still running is cancelled, and a late "valid" result is ignored. To swap a key, choose **Replace … key**; **Cancel replacement** backs out.

## Read the status line { #status-messages }

| Status line | Meaning | What to do |
|---|---|---|
| *Optional · setup required* | No credential saved | Connect it, or leave the provider off |
| *Saved · not tested* | A credential is stored but was never checked | Choose **Test saved … connection** |
| *Connection verified* | The provider accepted it | Tick **Use …** and save |
| *Credential rejected* | The provider refused the key or token | **Replace** it, or **Remove** it. Discogs stays available anonymously. |
| *Verify your email before use* | SonoVault has not confirmed your email | Verify the email, then test again |
| *Account access restricted* | The account cannot use this metadata | Check the account with the provider |
| *Rate or quota limit reached* / *Quota exhausted* | Your allowance is used up | Wait. A line such as *Next request after …* appears when a time is known. CueRithm never guesses a reset time. |
| *Anonymous · 25 requests/minute* | Discogs without a verified token | Optional: add a token |
| *Credential store · locked* (or *unavailable*, *denied*, *corrupt*, *ambiguous*) | The system credential store cannot be used | Unlock or reconnect it, then choose **Refresh provider setup** |
| *Credential change needs attention* | A save or removal may not have finished | Open **Manage …** and check or remove the credential |

Public providers and offline analysis keep working while the credential store is unavailable.

## What can go wrong { #what-can-go-wrong }

| Symptom | Cause | Fix |
|---|---|---|
| **Use SonoVault** or **Use Volumo** stays disabled although a key is saved | The last test found a problem: unverified email, rejected key, restricted account, exhausted allowance or a test key | Read the status line, fix that, then test again |
| The test says *This is a test key* | A mock key does not prove live access | Use a live key from the provider |
| *Connection test did not finish* | Network problem or timeout. This does not prove the key is wrong. | Check your connection and test again |
| A track waits in Incoming with a provider error | The provider is rate limited or offline | Wait, then press the row's retry button. If lookups keep returning nothing, see [Catalog tagging finds nothing](../help/troubleshooting/no-catalog-match.md). |

## Related { #related }

- [Choose providers and fields to write](choose-providers.md): what each provider supplies and how requests are paced.
- [Identify a track with no match](identify-unknown-tracks.md): a different opt-in service that needs no account.
- [Review and fix a wrong match](review-a-match.md): see which providers answered for one track.
- [Automatic tagging settings](../reference/settings/automatic-tagging.md)

<!-- verified: apps/desktop/src/components/ProviderSetup.tsx (labels, sign-up links, status summaries); apps/desktop/src/provider-setup-types.ts; apps/desktop/src/test/provider-credentials.test.tsx; crates/cuerithm-library/src/provider_setup/mod.rs (can_use, setup kinds, tests, rate limits); crates/cuerithm-credentials/src/native.rs (Keychain, Credential Manager, Secret Service); crates/cuerithm-autotag/src/config.rs (credentials rejected as settings); crates/cuerithm-autotag/src/volumo.rs (ISRC or track ID needed); legacy learn/auto-tagging.md#provider-setup re-checked -->
<!-- UNVERIFIED: where a Discogs personal access token is created (Discogs account developer settings); not in the code. Linux Secret Service naming (GNOME Keyring or KWallet) is inferred from the dbus-secret-service store. -->
