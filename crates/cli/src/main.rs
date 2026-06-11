use steward_core::pb::steward_service_client::StewardServiceClient;
use steward_core::pb::ExecuteTaskRequest;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let mut client = StewardServiceClient::connect("http://127.0.0.1:50051").await?;
    
    let request = tonic::Request::new(ExecuteTaskRequest {
        task: "Initial boot sequence".into(),
    });
    
    let response = client.execute_task(request).await?;
    println!("RESPONSE={:?}", response.into_inner());
    
    Ok(())
}
