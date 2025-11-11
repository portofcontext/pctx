# PTCX Authentication Examples

## Storage Methods

| Method | Format | Use Case |
|--------|--------|----------|
| Environment Variable | `${VAR_NAME}` | CI/CD, containerized apps |
| OS Keychain | `keychain://service/account` | Local development, production |
| External Command | `command://shell command` | Secret managers (1Password, Vault, AWS Secrets) |

---

### Using Secret Managers

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




