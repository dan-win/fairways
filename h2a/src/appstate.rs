#[derive(Debug, Deserialize, Serialize, Clone)]
struct TaskRecord {
    uri: String,
    exchange: String,
    routing_key: Option<String>,
}



