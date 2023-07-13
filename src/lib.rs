use flowsnet_platform_sdk::logger;
use google_cloud_service_flows::{cloud_vision::text_detection, vertex::chat};
use lambda_flows::{request_received, send_response};
use serde_json::Value;
use std::collections::HashMap;

#[no_mangle]
#[tokio::main(flavor = "current_thread")]
pub async fn run() {
    logger::init();
    request_received(handler).await;
}

async fn handler(_qry: HashMap<String, Value>, body: Vec<u8>) {
    // match text_detection::text_detection(String::from_utf8_lossy(&body).into_owned()).await {
    let co = chat::ChatOptions {
        ..Default::default()
    };

    let his = chat::chat_history("test-chat-for-vertex", 0);
    log::warn!("{:?}", his);

    match chat::chat(
        "test-chat-for-vertex",
        String::from_utf8_lossy(&body).into_owned().as_str(),
        &co,
    )
    .await
    {
        Ok(x) => {
            send_response(
                200,
                vec![(
                    String::from("content-type"),
                    String::from("text/plain; charset=UTF-8"),
                )],
                x.as_bytes().to_vec(),
            );
        }
        Err(e) => {
            send_response(
                500,
                vec![(
                    String::from("content-type"),
                    String::from("text/plain; charset=UTF-8"),
                )],
                e.as_bytes().to_vec(),
            );
        }
    }
}
