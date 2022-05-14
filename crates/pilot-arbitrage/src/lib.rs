pub mod arbitrage;

pub(crate) async fn send_messages(token: &str, channel: u64, msg: String) {
    let client = serenity::http::client::Http::new(token);
    client
        .send_message(channel, &serde_json::json!({ "content": msg }))
        .await
        .unwrap();
}
