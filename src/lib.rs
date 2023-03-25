use github_flows::{
    get_octo, listen_to_event,
    octocrab::{
        models::events::payload::PullRequestEventAction, models::repos::GitUser,
        Result as OctoResult,
    },
    EventPayload,
};
use sendgrid_flows::{send_email, Email};

#[no_mangle]
#[tokio::main(flavor = "current_thread")]
pub async fn run() -> anyhow::Result<()> {
    let login: &str = "DarumaDocker";
    let owner: &str = "DarumaDocker";
    let repo: &str = "github-func-test";
    let sender_email_sendgrid: &str = "vivian@secondstate.io";

    listen_to_event(login, owner, repo, vec!["pull_request"], |payload| {
        handler(login, sender_email_sendgrid, payload)
    })
    .await;

    Ok(())
}

async fn handler(login: &str, sender_email_sendgrid: &str, payload: EventPayload) {
    let octocrab = get_octo(Some(String::from(login)));

    if let EventPayload::PullRequestEvent(e) = payload {
        if e.action != PullRequestEventAction::Closed {
            return;
        }
        let pull = e.pull_request;
        let html_url = pull.html_url.expect("no html_url found").to_string();

        let user = pull.user.expect("no contributor info found");
        let contributor = user.login;
        let contributor_route = format!("users/{contributor}");

        if pull.merge_commit_sha.is_some() || pull.commits_url.is_some() {
            let response: OctoResult<GitUser> = octocrab.get(&contributor_route, None::<&()>).await;
            let contributor_email = match response {
                Err(_) => "".to_string(),
                Ok(user_obj) => user_obj.email,
            };

            let content = format!(
                r#"
Hi {contributor}, <br/>
Welcome to the {html_url} community, thank you for your contribution!"#
            );
            let email_obj = Email {
                to: vec![contributor_email.to_string()],
                subject: String::from("Thank you for contributing to this repository"),
                content: content,
            };
            send_email(sender_email_sendgrid, &email_obj).expect("failed to send email");
        }
    }
}
