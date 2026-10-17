use askama::Template;
use axum::{
    extract::Path,
    response::{Html, Redirect},
    routing::get,
    Router,
};

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    active: &'static str,
    total_balance: String,
}

#[derive(Template)]
#[template(path = "plans.html")]
struct PlansTemplate {
    active: &'static str,
}

#[derive(Template)]
#[template(path = "new_plan.html")]
struct NewPlanTemplate {
    active: &'static str,
}

#[derive(Template)]
#[template(path = "plan_detail.html")]
struct PlanDetailTemplate {
    active: &'static str,
}

#[derive(Template)]
#[template(path = "groups.html")]
struct GroupsTemplate {
    active: &'static str,
}

#[derive(Template)]
#[template(path = "group_detail.html")]
struct GroupDetailTemplate {
    active: &'static str,
}

#[derive(Template)]
#[template(path = "deposit.html")]
struct DepositTemplate {
    active: &'static str,
}

async fn dashboard_handler() -> Html<String> {
    let t = DashboardTemplate {
        active: "home",
        total_balance: "$24,850.00".to_string(),
    };
    Html(t.render().unwrap())
}

async fn plans_handler() -> Html<String> {
    Html(PlansTemplate { active: "plans" }.render().unwrap())
}

async fn new_plan_handler() -> Html<String> {
    Html(NewPlanTemplate { active: "plans" }.render().unwrap())
}

// plan details page, the id is ignored until we have real data from SQLx
async fn plan_detail_handler(Path(_id): Path<String>) -> Html<String> {
    Html(PlanDetailTemplate { active: "plans" }.render().unwrap())
}

async fn groups_handler() -> Html<String> {
    Html(GroupsTemplate { active: "groups" }.render().unwrap())
}

// group details page, the id is ignored until we have real data from SQLx
async fn group_detail_handler(Path(_id): Path<String>) -> Html<String> {
    Html(GroupDetailTemplate { active: "groups" }.render().unwrap())
}

// Stub: the "New group" button lands here until its screen is designed
async fn new_group_handler() -> Redirect {
    Redirect::to("/groups")
}

async fn deposit_handler() -> Html<String> {
    Html(DepositTemplate { active: "home" }.render().unwrap())
}

// Stub: replace with Form<NewPlan> + SQLx insert later
async fn create_plan_handler() -> Redirect {
    Redirect::to("/plans")
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(dashboard_handler))
        .route("/plans", get(plans_handler))
        .route("/plans/new", get(new_plan_handler).post(create_plan_handler))
        // axum 0.8 wants "/plans/{id}" instead
        .route("/plans/:id", get(plan_detail_handler))
        .route("/groups", get(groups_handler))
        .route("/groups/new", get(new_group_handler))
        // axum 0.8 wants "/groups/{id}" instead
        .route("/groups/:id", get(group_detail_handler))
        .route("/deposit", get(deposit_handler));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    println!("Server running at http://127.0.0.1:3000");
    axum::serve(listener, app).await.unwrap();
}
