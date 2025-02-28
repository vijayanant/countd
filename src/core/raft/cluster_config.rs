use std::collections::HashMap;

pub struct ClusterConfig {
    pub node_addresses: HashMap<u64, String>,
}

pub fn get_cluster_config() -> ClusterConfig {
    let mut node_addresses = HashMap::new();
    node_addresses.insert(1, "127.0.0.1:56781".to_string());
    node_addresses.insert(2, "127.0.0.1:56782".to_string());
    node_addresses.insert(3, "127.0.0.1:56783".to_string());

    ClusterConfig { node_addresses }
}

