# Security

## Reporting a vulnerability

Please do not open a public issue for security problems. Use GitHub's private vulnerability reporting
on this repository (the **Security** tab, then **Report a vulnerability**). Include what you found, how
to reproduce it, and what an attacker could do with it.

## What to know about the app

- AimForge runs locally. Recordings, settings and the plan are stored on your computer.
- If you connect an AI provider, its API key is stored **unencrypted** in the app's local data folder
  and sent only to the provider you chose. Report data sent to the provider is limited by the switches
  under Settings, Coach; movement recordings are never sent.
- The built-in report engine works offline and needs no key.

Encrypting stored keys is a known gap and is planned.
