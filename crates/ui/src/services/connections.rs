use serde::{Deserialize, Serialize};

/// Information about a saved notify service connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConnection {
    pub name: String,
    pub url: String,
}

impl Default for ServiceConnection {
    fn default() -> Self {
        Self { name: "Local Service".to_string(), url: "http://localhost:7004".to_string() }
    }
}

/// Load saved service connections from configuration
pub async fn load_connections() -> Vec<ServiceConnection> {
    // For now, return a default local connection
    // In the future, this could load from a config file
    vec![ServiceConnection::default()]
}
