# PTCX Authentication Examples

Quick reference guide for configuring authentication in PTCX.

## Storage Methods

| Method | Format | Use Case |
|--------|--------|----------|----------|
| Environment Variable | `${VAR_NAME}` | CI/CD, containerized apps |
| OS Keychain | `keychain://service/account` | Local development, production |
| External Command | `command://shell command` | Secret managers (1Password, Vault, AWS Secrets) |

---

## Bearer Token Authentication

Standard HTTP Bearer token authentication.

### Using Environment Variables

**Config** (`~/.pctx/config.toml`):
```toml
[[servers]]
name = "production"
url = "https://api.example.com/mcp"

[servers.auth]
type = "bearer"
token = "${PROD_API_TOKEN}"
```

**Setup**:
```bash
# Add to ~/.bashrc or ~/.zshrc
export PROD_API_TOKEN="sk_live_abc123xyz789"

# Use the server
ptcx mcp list production
```

---

### Using OS Keychain

**Config**:
```toml
[[servers]]
name = "production"
url = "https://api.example.com/mcp"

[servers.auth]
type = "bearer"
token = "keychain://pctx/production"
```

**Setup (macOS)**:
```bash
security add-generic-password -s pctx -a production -w "sk_live_abc123xyz789"
```

**Setup (Linux)**:
```bash
secret-tool store --label='ptcx Production Token' service pctx account production
# Paste token when prompted
```

**Setup (Windows)**:
```powershell
cmdkey /generic:pctx-production /user:production /pass:sk_live_abc123xyz789
```

---

### Using 1Password

**Config**:
```toml
[[servers]]
name = "production"
url = "https://api.example.com/mcp"

[servers.auth]
type = "bearer"
token = "command://op read op://Personal/mcp-server/token"
```

**Setup**:
```bash
# Install 1Password CLI
brew install --cask 1password/tap/1password-cli

# Sign in
eval $(op signin)

# Store token in 1Password app (one-time):
# 1. Open 1Password
# 2. Create item "mcp-server" in "Personal" vault
# 3. Add field "token" with your token value
```

---

### Using Other Secret Managers

**AWS Secrets Manager**:
```toml
token = "command://aws secretsmanager get-secret-value --secret-id mcp-token --query SecretString --output text"
```

**Azure Key Vault**:
```toml
token = "command://az keyvault secret show --vault-name my-vault --name mcp-token --query value -o tsv"
```

**HashiCorp Vault**:
```toml
token = "command://vault kv get -field=token secret/mcp"
```

**Pass (password store)**:
```toml
token = "command://pass show mcp/production-token"
```

---

## OAuth 2.1 Client Credentials

Machine-to-machine authentication without browser interaction. Perfect for AI agents and server-to-server communication.

**Config**:
```toml
[[servers]]
name = "oauth-api"
url = "https://api.example.com/mcp"

[servers.auth]
type = "oauth-client-credentials"
client_id = "my-client-id"
client_secret = "${CLIENT_SECRET}"  # Supports ${VAR}, keychain://, command://
token_url = "https://auth.example.com/oauth/token"
scope = "api:read api:write"  # Optional
```

**Setup**:
```bash
# Store client secret securely
export CLIENT_SECRET="secret_abc123xyz789"

# Or use keychain
security add-generic-password -s pctx -a oauth-secret -w "secret_abc123xyz789"

# Update config to use keychain
# client_secret = "keychain://pctx/oauth-secret"

# ptcx automatically fetches and caches OAuth tokens
ptcx mcp list oauth-api
```

**How it works**:
1. ptcx exchanges `client_id` + `client_secret` for access token
2. Token is cached and automatically refreshed when expired
3. No browser or user interaction required

---

## Custom Headers & Query Parameters

For APIs with non-standard authentication.

### Custom Headers

**Config**:
```toml
[[servers]]
name = "custom-api"
url = "https://api.example.com/mcp"

[servers.auth]
type = "custom"

[servers.auth.headers]
"X-API-Key" = "${API_KEY}"
"X-Client-ID" = "ptcx-client"
"X-Request-ID" = "command://uuidgen"
```

**Resulting HTTP request**:
```http
GET /mcp HTTP/1.1
Host: api.example.com
X-API-Key: <value-from-env>
X-Client-ID: ptcx-client
X-Request-ID: <generated-uuid>
```

---

### Custom Query Parameters

**Config**:
```toml
[[servers]]
name = "query-auth-api"
url = "https://api.example.com/mcp"

[servers.auth]
type = "custom"

[servers.auth.query]
api_key = "${API_KEY}"
client_id = "ptcx-client"
session_id = "keychain://pctx/session"
```

**Resulting request**:
```
https://api.example.com/mcp?api_key=<value>&client_id=ptcx-client&session_id=<keychain-value>
```

---

### Mixed Headers and Query Parameters

**Config**:
```toml
[[servers]]
name = "complex-api"
url = "https://api.example.com/mcp"

[servers.auth]
type = "custom"

[servers.auth.headers]
"Authorization" = "${OAUTH_TOKEN}"
"X-Client-Version" = "1.0.0"

[servers.auth.query]
api_key = "${API_KEY}"
format = "json"
```

---

## CLI Commands

### Add Server with Bearer Auth

```bash
# Using environment variable
ptcx mcp add production https://api.example.com/mcp \
  --auth bearer \
  --auth-token '${PROD_TOKEN}'

# Using keychain
security add-generic-password -s pctx -a production -w "your_token"
ptcx mcp add production https://api.example.com/mcp \
  --auth bearer \
  --auth-token 'keychain://pctx/production'

# Using 1Password
ptcx mcp add production https://api.example.com/mcp \
  --auth bearer \
  --auth-token 'command://op read op://vault/token'
```

---

### View Server Configuration

```bash
ptcx mcp get production
```

**Output example**:
```
Server: production
  URL: https://api.example.com/mcp
  Auth:
    Type: bearer
    Token: ${PROD_TOKEN}
```

---

## Security Best Practices

1. **Never commit tokens to version control** - Use `.gitignore` for config files with secrets
2. **Prefer keychain for local development** - OS-managed, encrypted storage
3. **Use environment variables for CI/CD** - Easy to rotate, no files to manage
4. **Use secret managers for production** - Centralized, auditable, rotatable
5. **Use OAuth Client Credentials for M2M** - Automatic token refresh, standards-compliant
6. **Rotate credentials regularly** - Update tokens every 30-90 days

---

## Troubleshooting

### Environment variable not found
```bash
# Check if variable is set
echo $MY_TOKEN

# Set it in current shell
export MY_TOKEN="value"

# Make permanent: add to ~/.bashrc or ~/.zshrc
echo 'export MY_TOKEN="value"' >> ~/.bashrc
```

### Keychain access denied
```bash
# macOS: Grant terminal access in System Preferences > Security > Privacy > Keychain

# Linux: Ensure libsecret is installed
sudo apt-get install libsecret-1-0  # Debian/Ubuntu
```

### Command execution fails
```bash
# Test command manually
sh -c "your-command-here"

# Ensure command is in PATH
which op  # For 1Password
which aws # For AWS CLI
```

### OAuth token expired
ptcx automatically refreshes expired tokens. If issues persist:
```bash
# View server config to check token status
ptcx mcp get server-name

# Remove cached credentials to force refresh
# Edit ~/.pctx/config.toml and remove the [servers.auth.credentials] section
```
