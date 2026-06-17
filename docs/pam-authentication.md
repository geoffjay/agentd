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

> [!CAUTION]
> Enabling PAM requires running `agentd-core` as a **system** service with
> permission to verify system passwords (see [Privilege model](#privilege-model)).
> This is a deployment-model change that touches systemd unit privileges — a
> human-approval-adjacent area per `docs/planning/autonomous-pipeline-gates.md`.
> The installer does **not** make this change automatically.

## Build

The PAM backend links system `libpam` and is gated behind a Cargo feature, off
by default. Build core with it enabled:

```bash
# Debian/Ubuntu: sudo apt-get install -y libpam0g-dev
# RHEL/Fedora:   sudo dnf install -y pam-devel
cargo build -p agentd-core --features pam --release
```

A build **without** `--features pam` still runs, but every `pam` login fails
closed with `500` and a startup warning is logged if `AGENTD_PAM_ENABLED=true`.

## Privilege model

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

| Variable                  | Default     | Description                                         |
|---------------------------|-------------|-----------------------------------------------------|
| `AGENTD_PAM_ENABLED`      | `false`     | Master switch. When off, `pam` users fail closed and JIT never triggers. |
| `AGENTD_PAM_SERVICE`      | `agentd`    | PAM service name → `/etc/pam.d/<service>`.          |
| `AGENTD_PAM_EMAIL_DOMAIN` | `pam.local` | Domain for the synthesized email of a JIT-provisioned user (`<user>@<domain>`). |

## Manual verification checklist

These cannot run in CI (they need a live PAM stack on a Linux host):

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
