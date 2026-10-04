# Security policy

## Do not publish secrets

Never put API keys, Xiaomi account credentials, SSH passwords, private keys, device IPs or unsanitized logs into issues, pull requests, commits or screenshots.

The runtime configuration file is intentionally excluded from Git. Use `ai.conf.example` as a template and keep the real `/data/open-xiaoai/ai.conf` permission at `600`.

## Reporting

For a security issue, avoid posting credentials publicly. Open a private report through GitHub if available, or contact the repository owner privately.
