#[cfg(test)]
mod tests {
    use crate::{PctxClient, SdkConfig, ServerConfig};

    #[test]
    fn test_create_client_with_config() {
        let config = SdkConfig { servers: vec![] };

        let client = PctxClient::new(config.clone());
        assert_eq!(client.upstream().len(), 0);
    }

    #[test]
    fn test_create_client_with_defaults() {
        let config = SdkConfig::default();
        let client = PctxClient::new(config);

        assert_eq!(client.upstream().len(), 0);
    }

    #[test]
    fn test_list_functions_empty() {
        let config = SdkConfig::default();
        let client = PctxClient::new(config);

        let result = client.list_functions();
        assert!(result.is_ok());
        // Should return empty string or minimal output when no servers
        let functions = result.unwrap();
        assert!(functions.is_empty() || functions.trim().is_empty());
    }

    #[test]
    fn test_get_function_details_empty() {
        let config = SdkConfig::default();
        let client = PctxClient::new(config);

        let result = client.get_function_details(vec!["Test.function".to_string()]);
        assert!(result.is_ok());
        let details = result.unwrap();
        assert!(details.contains("No namespaces/functions match"));
    }

    #[test]
    fn test_allowed_hosts_extraction() {
        let config = SdkConfig {
            servers: vec![
                ServerConfig {
                    name: "server1".to_string(),
                    url: url::Url::parse("http://localhost:3000").unwrap(),
                    auth: None,
                },
                ServerConfig {
                    name: "server2".to_string(),
                    url: url::Url::parse("https://api.example.com:8080").unwrap(),
                    auth: None,
                },
            ],
        };

        let allowed_hosts = config.allowed_hosts().unwrap();
        assert_eq!(allowed_hosts.len(), 2);
        assert!(allowed_hosts.contains(&"localhost:3000".to_string()));
        assert!(allowed_hosts.contains(&"api.example.com:8080".to_string()));
    }

    #[test]
    fn test_allowed_hosts_without_port() {
        let config = SdkConfig {
            servers: vec![ServerConfig {
                name: "server1".to_string(),
                url: url::Url::parse("https://example.com").unwrap(),
                auth: None,
            }],
        };

        let allowed_hosts = config.allowed_hosts().unwrap();
        assert_eq!(allowed_hosts.len(), 1);
        assert_eq!(allowed_hosts[0], "example.com");
    }

    #[test]
    fn test_config_from_json() {
        let json = r#"{
            "servers": [
                {
                    "name": "test-server",
                    "url": "http://localhost:3000"
                }
            ]
        }"#;

        let config = SdkConfig::from_json(json).unwrap();
        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.servers[0].name, "test-server");
    }

    #[test]
    fn test_config_from_json_empty() {
        let json = r#"{}"#;

        let config = SdkConfig::from_json(json).unwrap();
        assert_eq!(config.servers.len(), 0);
    }

    #[test]
    fn test_config_cli_conversion() {
        let mut cli_config = pctx_config::Config::default();
        cli_config.servers = vec![ServerConfig {
            name: "test".to_string(),
            url: url::Url::parse("http://localhost:3000").unwrap(),
            auth: None,
        }];

        // Convert to SDK config
        let sdk_config: SdkConfig = cli_config.clone().into();
        assert_eq!(sdk_config.servers.len(), 1);

        // Convert back to CLI config
        let back_to_cli: pctx_config::Config = sdk_config.into();
        assert_eq!(back_to_cli.servers.len(), 1);
    }
}
