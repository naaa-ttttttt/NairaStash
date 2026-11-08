use axum::{routing::get, response::{Html, Redirect}, Router};
use askama::Template;
use crate::AppState;

#[derive(Template)]
#[template(path = "plans.html")]
pub struct PlansTemplate { pub active: &'static str }

#[derive(Template)]
#[template(path = "new_plan.html")]
pub struct NewPlanTemplate { pub active: &'static str }

#[derive(Template)]
#[template(path = "plan_detail.html")]
pub struct PlanDetailTemplate { pub active: &'static str }

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/plans", get(plans_handler))
        .route("/plans/new", get(new_plan_handler).post(create_plan_handler))
        .route("/plans/:id", get(plan_detail_handler))
        .with_state(state)
}

pub async fn plans_handler() -> Html<String> {
    Html(PlansTemplate { active: "plans" }.render().unwrap())
}

pub async fn new_plan_handler() -> Html<String> {
    Html(NewPlanTemplate { active: "plans" }.render().unwrap())
}

pub async fn create_plan_handler() -> Redirect { Redirect::to("/plans") }

pub async fn plan_detail_handler() -> Html<String> {
    Html(PlanDetailTemplate { active: "plans" }.render().unwrap())
}

