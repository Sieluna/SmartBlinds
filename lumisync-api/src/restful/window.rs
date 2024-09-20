use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWindowRequest {
    pub region_id: i32,
    pub name: String,
    pub state: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateWindowRequest {
    pub name: Option<String>,
    pub state: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowResponse {
    pub id: i32,
    pub region_id: i32,
    pub name: String,
    pub state: f32,
}
