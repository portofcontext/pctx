use anyhow::Result;
use indexmap::IndexMap;
use pctx_codegen::case::Case;
use pctx_config::auth::{AuthConfig, AuthSecret, SecretString, write_to_keychain};
use tracing::info;

use crate::utils::styles::{fmt_dimmed, fmt_good_check};

pub(crate) fn prompt_auth(server_name: &str) -> Result<AuthConfig> {
    let options = vec![
        "Bearer Token".to_string(),
        "Headers".to_string(),
        // format!("OAuth2 {}", fmt_dimmed("(client credentials flow)")),
    ];
    let selection = inquire::Select::new(
        "How do you want to authenticate with the MCP server",
        options.clone(),
    )
    .prompt()?;
    match options.iter().position(|o| o == &selection) {
        Some(0) => {
            // bearer token
            let bearer_key = Case::Snake.sanitize(format!("{server_name}_bearer"));
            let token = prompt_secret("Select auth option for bearer token:", "", &bearer_key)?;
            Ok(AuthConfig::Bearer { token })
        }

        Some(1) => {
            // custom headers
            let mut prompt = true;
            let mut headers: IndexMap<String, SecretString> = IndexMap::new();
            while prompt {
                let key = inquire::Text::new("Header name:")
                    .with_placeholder("Authorizaiton")
                    .with_validator(inquire::min_length!(
                        1,
                        "header names should be at least 1 character"
                    ))
                    .prompt()?;
                let val = prompt_secret_parse("Header value")?;
                headers.insert(key, val);

                prompt = inquire::Confirm::new("Add another header?")
                    .with_default(false)
                    .prompt()?;
            }

            Ok(AuthConfig::Headers { headers })
        }
        // Some(2) => {
        //     // OAuth2
        //     let token_url = inquire::Text::new("├── Token URL:")
        //         .with_validator(validators::url)
        //         .prompt()?;

        //     let client_id_key = Case::Snake.sanitize(format!("{server_name}_client_id"));
        //     let client_id = prompt_secret("├── Client ID:", "│   ", &client_id_key)?;

        //     let client_secret_key = Case::Snake.sanitize(format!("{server_name}_client_secret"));
        //     let client_secret = prompt_secret("├── Client Secret:", "│   ", &client_secret_key)?;

        //     let scope = inquire::Text::new("└── Scopes:")
        //         .with_help_message("comma separated scopes, leave empty if does not apply")
        //         .prompt_skippable()?;

        //     Ok(AuthConfig::OAuthClientCredentials {
        //         client_id,
        //         client_secret,
        //         token_url: token_url.parse()?,
        //         scope,
        //     })
        // }
        _ => anyhow::bail!("Invalid selection {selection}"),
    }
}

/// Prompts user to create a simple `SecretString`, this can only output a
/// `SecretString` with one "part", for more complex `SecretString` syntax use `prompt_secret_parse`
pub(crate) fn prompt_secret(msg: &str, prefix: &str, key: &str) -> Result<SecretString> {
    let options = vec![
        "Use environment variable".to_string(),
        format!(
            "Create keychain entry {}",
            fmt_dimmed("(stored in system keychain)")
        ),
        format!(
            "Enter insecurely {}",
            fmt_dimmed("(stored as plain text in config file)")
        ),
    ];

    let selection = inquire::Select::new(msg, options.clone()).prompt()?;

    match options.iter().position(|o| o == &selection) {
        Some(0) => {
            // environment variable
            let env_var = inquire::Text::new(&format!("{prefix}Enter environment variable name:"))
                .with_validator(inquire::min_length!(1, "must be at least 1 character"))
                .prompt()?;
            Ok(SecretString::new_secret(AuthSecret::Env(env_var)))
        }
        Some(1) => {
            // keychain
            let secret = inquire::Text::new(&format!("{prefix}Enter value:"))
                .with_validator(inquire::min_length!(1, "must be at least 1 character"))
                .prompt()?;
            write_to_keychain(key, &secret)?;
            info!(
                "{}",
                fmt_good_check(&format!("{prefix}Value stored in keychain"))
            );
            Ok(SecretString::new_secret(AuthSecret::Keychain(key.into())))
        }
        Some(2) => {
            // plain text
            let secret = inquire::Text::new(&format!("{prefix}Enter value:")).prompt()?;
            Ok(SecretString::new_plain(&secret))
        }
        _ => anyhow::bail!("Invalid selection {selection}"),
    }
}

/// Prompts user to enter a secret by writing the syntax directly
pub(crate) fn prompt_secret_parse(msg: &str) -> Result<SecretString> {
    let secret = inquire::Text::new(msg)
        .with_help_message("accepts secret syntax that allows sourcing values from env, keychain, and commands. See docs for details")
        .with_validator(validators::secret_string)
        .prompt()?;

    SecretString::parse(&secret)
}

pub(crate) mod validators {
    use pctx_config::auth::SecretString;

    #[allow(clippy::unnecessary_wraps)]
    #[allow(unused)]
    pub(crate) fn url(
        val: &str,
    ) -> Result<inquire::validator::Validation, inquire::CustomUserError> {
        if url::Url::parse(val).is_ok() {
            Ok(inquire::validator::Validation::Valid)
        } else {
            Ok(inquire::validator::Validation::Invalid(
                "invalid url".into(),
            ))
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    pub(crate) fn secret_string(
        val: &str,
    ) -> Result<inquire::validator::Validation, inquire::CustomUserError> {
        if SecretString::parse(val).is_ok() {
            Ok(inquire::validator::Validation::Valid)
        } else {
            Ok(inquire::validator::Validation::Invalid(
                "invalid secret syntax, see the docs for syntax rules".into(),
            ))
        }
    }
}
