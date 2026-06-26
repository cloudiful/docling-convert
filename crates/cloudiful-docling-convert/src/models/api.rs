use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct TaskPostResponse {
    pub task_id: String,
}

#[derive(Deserialize, Debug)]
pub struct TaskStatusResponse {
    pub task_status: String,
}
