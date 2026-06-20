# PAM (system-user) authentication

`agentd-core` can authenticate logins against the host's **system users** via PAM
(Pluggable Authentication Modules), in addition to the default database password
(argon2) backend. This lets a person log in with their OS credentials — the same
identity that agents run under via `sudo -u <user>`.

PAM is a **pluggable, opt-in** backend:

- Each user row has an `auth_provider`: `local` (argon2 password, the default and
  the bootstrap/superuser/test escape hatch) or `pam` (host PAM stack).
- The first time a system user logs in by username, the app user is
  **just-in-time provisioned** (a user record + a personal organization), linked
  to the OS account via `system_username`.
- Local password auth is unaffected and remains available even when PAM is on.

The backend works on both **macOS** (OpenPAM) and **Linux** (Linux-PAM) from the
same code. macOS is the easy path for local development; Linux production has an
extra privilege requirement (below).

> [!CAUTION]
> On **Linux**, enabling PAM requires running `agentd-core` as a **system**
> service with permission to verify system passwords (see
> [Privilege model (Linux)](#privilege-model-linux)). That deployment-model
> change touches systemd unit privileges — a human-approval-adjacent area per
> `docs/planning/autonomous-pipeline-gates.md` — so the installer does **not**
> make it automatically. macOS dev has no such requirement.

## Build

The PAM backend links the system PAM library and is gated behind a Cargo
feature, off by default:

```bash
# macOS:        no extra packages — links the SDK's libpam.
# Debian/Ubuntu: sudo apt-get install -y libpam0g-dev
# RHEL/Fedora:   sudo dnf install -y pam-devel
cargo build -p agentd-core --features pam --release
```

It uses raw `pam-sys` bindings plus a small hand-written conversation (no
bindgen/libclang). `pam-sys` hard-codes Linux-PAM's status-code numbering, so
the verifier re-interprets the raw `pam_authenticate`/`pam_acct_mgmt` return
codes against the correct per-platform constants (OpenPAM and Linux-PAM number
their error codes differently); without this a wrong password on macOS would be
misreported (e.g. as `ACCT_EXPIRED`) and surfaced as a `500` instead of a `401`.
The conversation message-array layout (an array of pointers) is shared by
Linux-PAM and OpenPAM; only Solaris/illumos differ there.

A build **without** `--features pam` still runs, but every `pam` login fails
closed with `500` and a startup warning is logged if `AGENTD_PAM_ENABLED=true`.

### Prebuilt artifact (Linux x86_64)

The default release tarballs are static **musl** binaries and do **not** include
PAM (musl can't `dlopen` PAM modules). A separate dynamically-linked **glibc**
artifact ships with PAM compiled in for Linux x86_64 only; install it via the
opt-in flag:

```bash
AGENTD_PAM=1 curl -fsSL https://github.com/geoffjay/agentd/releases/latest/download/install.sh | sh
```

This fetches `agentd-<version>-x86_64-unknown-linux-gnu-pam.tar.gz`. It requires
a glibc at least as new as the build runner's; on other platforms (arm64, macOS)
build from source with `--features pam` as above.

## macOS (development)

macOS needs no privilege setup. The system ships a ready-made `chkpasswd` PAM
service (backed by `pam_opendirectory`) that verifies a user's password without
root. Point agentd at it:

```bash
AGENTD_PAM_ENABLED=true AGENTD_PAM_SERVICE=chkpasswd \
  cargo run -p agentd-core --features pam
```

Then log in with your macOS account username + password. Because the OpenPAM
round-trip is exercised the same way on macOS and Linux, the real verifier path
can be developed and tested entirely on a Mac — see the `pam_smoke` tests below.

## Privilege model (Linux)

> This section applies to Linux only. macOS verifies via `pam_opendirectory`
> with no shadow-group or root requirement (see [macOS](#macos-development)).

`pam_unix` reads `/etc/shadow`. When the calling process cannot read shadow
directly it falls back to the SUID helper `unix_chkpwd`, which **refuses to
verify any account other than the caller's own UID** (an anti-brute-force
hardening). So an unprivileged / `systemd --user` core process **cannot**
authenticate other users.

Pick one of:

1. **shadow group + system service (recommended for self-hosted).** Run core as
   a dedicated system service and add its account to the `shadow` group so
   `pam_unix` reads `/etc/shadow` directly:

   ```bash
   usermod -aG shadow agentd      # core's service account
   systemctl restart agentd-core
   ```

   Trade-off: `shadow` group membership grants read access to all password
   hashes. A core compromise could exfiltrate hashes for offline cracking — but
   not plaintext passwords and not root. This is a far smaller blast radius than
   running core as root (which you should **not** do).

2. **SSSD/LDAP/AD-backed PAM (recommended for enterprise/managed hosts).** Point
   `/etc/pam.d/agentd` at `pam_sss`. `pam_sss` consults a privileged SSSD socket
   and needs **no** local shadow access, so core requires neither shadow-group
   membership nor root. Best security posture; requires SSSD infrastructure.

## Install the PAM service file

Install the sample stack ([`docs/assets/pam/agentd`](assets/pam/agentd)) as
`/etc/pam.d/<AGENTD_PAM_SERVICE>` (default `agentd`):

```bash
sudo install -m 0644 docs/assets/pam/agentd /etc/pam.d/agentd
```

Adjust the stack for your host (RHEL `system-auth`, or `pam_sss` for SSSD) per
the comments in that file.

## Configuration

PAM can be configured two ways. Settings are read from the shared `config.toml`
first, then any `AGENTD_PAM_*` environment variables are overlaid on top — **the
environment variable wins** when both are set. This lets you keep a stable base
in `config.toml` and override per-launch from the environment.

### `config.toml`

Add a `[services.core.pam]` section to the shared config file (on macOS,
`~/Library/Application Support/agentd-core/config.toml`; see the config guide for
your platform's path). This is the recommended way to run the **installed**
production binary, since it needs no environment wrangling at launch:

```toml
[services.core.pam]
enabled = true
service = "chkpasswd"   # macOS dev; "agentd" (the default) on Linux
email_domain = "pam.local"
```

### Environment variables

| Variable / `[services.core.pam]` key | Default | Description |
|--------------------------------------|---------|-------------|
| `AGENTD_PAM_ENABLED` / `enabled`           | `false`     | Master switch. When off, `pam` users fail closed and JIT never triggers. |
| `AGENTD_PAM_SERVICE` / `service`           | `agentd`    | PAM service name → `/etc/pam.d/<service>`. |
| `AGENTD_PAM_EMAIL_DOMAIN` / `email_domain` | `pam.local` | Domain for the synthesized email of a JIT-provisioned user (`<user>@<domain>`). |

## Manual verification

These need a live PAM stack, so they run outside CI.

### Verifier smoke tests (`--ignored`)

The `pam_smoke` integration tests drive the real verifier. The wrong-password
case needs no credentials and runs anywhere with a PAM stack (including macOS):

```bash
cargo test -p agentd-core --features pam --test pam_smoke -- --ignored --nocapture
# optional positive check (supply your own password out-of-band):
AGENTD_PAM_TEST_PASSWORD='...' \
  cargo test -p agentd-core --features pam --test pam_smoke -- --ignored --nocapture
```

### macOS end-to-end

1. `AGENTD_PAM_ENABLED=true AGENTD_PAM_SERVICE=chkpasswd cargo run -p agentd-core --features pam`
2. `curl -sX POST localhost:17000/auth/login -H 'content-type: application/json' -d '{"username":"'"$USER"'","password":"<your-mac-password>"}'`
   → `200` + token, and a JIT-created user (`auth_provider: "pam"`) with a personal org.
3. Wrong password → `401`. A registered `local` user still logs in (escape hatch intact).

### Linux production checklist

1. Install `/etc/pam.d/agentd`; run core as a **system** systemd unit; add its
   account to the `shadow` group (option 1) or enroll the host in SSSD (option 2).
2. Set `AGENTD_PAM_ENABLED=true`, restart core.
3. Log in as a real OS user:
   ```bash
   curl -sX POST localhost:7000/auth/login \
     -H 'content-type: application/json' \
     -d '{"username":"<sysuser>","password":"<syspass>"}'
   ```
   Expect `200` + a token, and a JIT-created user (`auth_provider: "pam"`) with a
   personal organization.
4. Wrong password → `401`. Expired/locked account (rejected by `acct_mgmt`) → `401`.
5. A registered `local` user still logs in by password (escape hatch intact).
6. Negative control: with core running as `systemd --user` (no shadow access),
   confirm PAM logins fail — demonstrating why the system-service requirement
   exists.
