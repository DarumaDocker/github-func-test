use flowsnet_platform_sdk::write_error_log;
use github_flows::{
    get_octo, listen_to_event,
    octocrab::models::{events::payload::EventPayload, reactions::ReactionContent},
};
use openai_flows::{create_completion, CompletionRequest};
use slack_flows::send_message_to_channel;

#[no_mangle]
#[tokio::main(flavor = "current_thread")]
pub async fn run() {
    listen_to_event(
        "DarumaDocker",
        "DarumaDocker",
        "github-func-test",
        vec!["issue_comment"],
        handler,
    )
    .await;
}

async fn handler(payload: EventPayload) {
    let octo = get_octo(Some(String::from("DarumaDocker")));
    let issues = octo.issues("DarumaDocker", "github-func-test");

    let reaction = match payload {
        EventPayload::IssuesEvent(e) => {
            let issue_id = e.issue.number;
            Some(
                issues
                    .create_reaction(issue_id, react(&e.issue.title))
                    .await,
            )
        }
        EventPayload::IssueCommentEvent(e) => {
            let comment_id = e.comment.id.0;
            Some(
                issues
                    .create_comment_reaction(comment_id, react(&e.comment.body.unwrap_or_default()))
                    .await,
            )
        }
        _ => None,
    };

    if let Some(re) = reaction {
        match re {
            Ok(c) => send_message_to_channel("reactor-space", "t1", c.created_at.to_rfc2822()),
            Err(e) => send_message_to_channel("reactor-space", "t1", e.to_string()),
        }
    }
}

fn react(s: &str) -> ReactionContent {
    let prompt = format!(
        r#"
        Decide whether a Tweet's sentiment is positive, neutral, or negative.
        Tweet: "{}"
        Sentiment:
                "#,
        s.replace("\"", "'")
    );
    write_error_log!(prompt);
    let cr = CompletionRequest {
        prompt,
        ..Default::default()
    };
    let mut r = create_completion("Michael", cr);
    send_message_to_channel("reactor-space", "t1", format!("{:?}", r));
    match r.pop().unwrap_or_default().trim().to_lowercase().as_str() {
        "positive" => ReactionContent::Hooray,
        "neutral" => ReactionContent::Heart,
        "negative" => ReactionContent::MinusOne,
        _ => ReactionContent::Eyes,
    }
}
