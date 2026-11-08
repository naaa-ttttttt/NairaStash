use axum::{routing::get, response::{Html, Redirect}, Router};
use askama::Template;
use crate::AppState;

#[derive(Template)]
#[template(path = "groups.html")]
pub struct GroupsTemplate { pub active: &'static str }

#[derive(Template)]
#[template(path = "new_group.html")]
pub struct NewGroupTemplate { pub active: &'static str }

#[derive(Template)]
#[template(path = "group_detail.html")]
pub struct GroupDetailTemplate { pub active: &'static str }

#[derive(Template)]
#[template(path = "group_rules.html")]
pub struct GroupRulesTemplate { pub active: &'static str }

#[derive(Template)]
#[template(path = "group_approvals.html")]
pub struct GroupApprovalsTemplate { pub active: &'static str }

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/groups", get(groups_handler))
        .route("/groups/new", get(new_group_handler).post(create_group_handler))
        .route("/groups/:id", get(group_detail_handler))
        .route("/rules", get(group_rules_handler))
        .route("/approvals", get(group_approvals_handler))
        .with_state(state)
}

pub async fn groups_handler() -> Html<String> {
    Html(GroupsTemplate { active: "groups" }.render().unwrap())
}

pub async fn new_group_handler() -> Html<String> {
    Html(NewGroupTemplate { active: "groups" }.render().unwrap())
}

pub async fn create_group_handler() -> Redirect { Redirect::to("/groups") }

pub async fn group_detail_handler() -> Html<String> {
    Html(GroupDetailTemplate { active: "groups" }.render().unwrap())
}

pub async fn group_rules_handler() -> Html<String> {
    Html(GroupRulesTemplate { active: "groups" }.render().unwrap())
}

pub async fn group_approvals_handler() -> Html<String> {
    Html(GroupApprovalsTemplate { active: "groups" }.render().unwrap())
}

