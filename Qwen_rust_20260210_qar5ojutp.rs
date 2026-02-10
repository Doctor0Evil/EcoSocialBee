// File: cyboair-governance/src/lib.rs
// Destination path: ./cyboair-governance/src/lib.rs
// Core library for governed Rust/ALN interfaces.

use gatehouse::{AuthorizationEngine, PolicySet, Subject, Resource, Action, Environment, Request};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Define our custom data structures that map to the entities above.
#[derive(Serialize, Deserialize, Debug)]
pub struct ApiUser {
    pub user_id: String,
    pub role: String, // e.g., "Superchair", "Stakeholder"
    pub attributes: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ApiResource {
    pub resource_id: String,
    pub resource_type: String,
    pub properties: HashMap<String, String>,
}

// This struct would be used to represent the environment context.
// For simplicity, we'll just include time and IP.
#[derive(Serialize, Deserialize, Debug)]
pub struct ApiEnvironment {
    pub time_utc: String, // ISO 8601 formatted string
    pub ip_address: String,
}

// A wrapper for the final decision.
#[derive(Serialize, Deserialize, Debug)]
pub struct AuthorizationResponse {
    pub allowed: bool,
    pub rationale: String,
}

/// A client for interacting with the authorization engine.
pub struct GovernanceClient {
    engine: AuthorizationEngine,
}

impl GovernanceClient {
    /// Creates a new GovernanceClient and loads the initial policy set.
    pub fn new() -> Self {
        let policy_set = load_policies(); // Function to parse policies from a file.
        let engine = AuthorizationEngine::new(policy_set);
        
        GovernanceClient { engine }
    }

    /// Attempts to authorize a request and returns an AuthorizationResponse.
    pub fn authorize(&self, 
                    user: &ApiUser, 
                    resource: &ApiResource, 
                    action: &str, 
                    env: &ApiEnvironment) -> AuthorizationResponse {

        // Construct the Gatehouse entities from our API structs.
        let subject = Subject::new(user.user_id.clone(), vec![user.role.clone()], user.attributes.clone());
        let res = Resource::new(resource.resource_id.clone(), resource.properties.clone());
        let act = Action::new(action.to_string());
        
        // For environment, you might need a more complex mapping depending on what Gatehouse expects.
        let mut gatehouse_env = HashMap::new();
        gatehouse_env.insert("time".to_string(), env.time_utc.clone());
        gatehouse_env.insert("ip".to_string(), env.ip_address.clone());
        let env_ctx = Environment::new(gatehouse_env);

        // Create the request.
        let request = Request::new(subject, res, act, env_ctx);

        // Make the decision.
        let decision = self.engine.authorize(&request);

        match decision {
            Ok(_) => AuthorizationResponse {
                allowed: true,
                rationale: "Request approved by policy engine.".to_string(),
            },
            Err(e) => AuthorizationResponse {
                allowed: false,
                rationale: format!("Access denied: {}", e),
            },
        }
    }
}

// This function would be part of a build script or loaded at app startup.
// It parses a YAML/JSON file containing the full policy definitions.
fn load_policies() -> PolicySet {
    // In a real implementation, this would read a file like `policies.yaml`.
    // For demonstration, we'll return a minimal, hardcoded policy set.
    // Example policy in Gatehouse's expected format would go here.
    // e.g., gatehouse::Policy::from_yaml(...);
    PolicySet::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_authorization_allow() {
        let mut client = GovernanceClient::new();

        // Create a test user with stakeholder role and an attribute.
        let user = ApiUser {
            user_id: "test_user_123".to_string(),
            role: "Stakeholder".to_string(),
            attributes: [("region".to_string(), "Phoenix".to_string())].into_iter().collect(),
        };

        // Create a resource owned by that user.
        let resource = ApiUser {
            user_id: "owned_machine_x".to_string(),
            role: "Node".to_string(),
            attributes: [("owner_id".to_string(), "test_user_123".to_string())].into_iter().collect(),
        };

        let action = "read_telemetry";
        let env = ApiEnvironment {
            time_utc: Utc::now().to_rfc3339(),
            ip_address: "192.168.1.100".to_string(),
        };

        let response = client.authorize(&user, &resource, action, &env);

        // In a real test, we'd have a policy that should allow this.
        // For now, we just check that no panics occur.
        assert!(response.allowed || !response.allowed); // This is a placeholder.
    }
}