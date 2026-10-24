use askama::Template;
use axum::{
    extract::Path,
    http::StatusCode,
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
#[template(path = "new_group.html")]
struct NewGroupTemplate {
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

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    active: &'static str,
}

#[derive(Template)]
#[template(path = "profile.html")]
struct ProfileTemplate {
    active: &'static str,
}

#[derive(Template)]
#[template(path = "notifications.html")]
struct NotificationsTemplate {
    active: &'static str,
}

#[derive(Template)]
#[template(path = "activity.html")]
struct ActivityTemplate {
    active: &'static str,
}

#[derive(Template)]
#[template(path = "link_account.html")]
struct LinkAccountTemplate {
    active: &'static str,
}

#[derive(Template)]
#[template(path = "not_found.html")]
struct NotFoundTemplate {
    active: &'static str,
}

#[derive(Template)]
#[template(path = "withdraw_plan.html")]
struct WithdrawPlanTemplate {
    active: &'static str,
}

#[derive(Template)]
#[template(path = "withdraw_amount.html")]
struct WithdrawAmountTemplate {
    active: &'static str,
}

#[derive(Template)]
#[template(path = "withdraw_destination.html")]
struct WithdrawDestinationTemplate {
    active: &'static str,
}

#[derive(Template)]
#[template(path = "withdraw_review.html")]
struct WithdrawReviewTemplate {
    active: &'static str,
}

#[derive(Template)]
#[template(path = "loan_request.html")]
struct LoanRequestTemplate {
    active: &'static str,
}

#[derive(Template)]
#[template(path = "loan_review.html")]
struct LoanReviewTemplate {
    active: &'static str,
}

#[derive(Template)]
#[template(path = "loan_detail.html")]
struct LoanDetailTemplate {
    active: &'static str,
}

#[derive(Template)]
#[template(path = "group_rules.html")]
struct GroupRulesTemplate {
    active: &'static str,
}

#[derive(Template)]
#[template(path = "group_approvals.html")]
struct GroupApprovalsTemplate {
    active: &'static str,
}

async fn dashboard_handler() -> Html<String> {
    let t = DashboardTemplate {
        active: "home",
        total_balance: "₦2,485,000.00".to_string(),
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

// Create / join group screen
async fn new_group_handler() -> Html<String> {
    Html(NewGroupTemplate { active: "groups" }.render().unwrap())
}

// Stub: replace with Form<NewGroup> + SQLx insert later
async fn create_group_handler() -> Redirect {
    Redirect::to("/groups")
}

// group details page, the id is ignored until we have real data from SQLx
async fn group_detail_handler(Path(_id): Path<String>) -> Html<String> {
    Html(GroupDetailTemplate { active: "groups" }.render().unwrap())
}

async fn deposit_handler() -> Html<String> {
    Html(DepositTemplate { active: "home" }.render().unwrap())
}

// Stub: replace with Form<NewPlan> + SQLx insert later
async fn create_plan_handler() -> Redirect {
    Redirect::to("/plans")
}

async fn login_handler() -> Html<String> {
    Html(LoginTemplate { active: "" }.render().unwrap())
}

// Stub: replace with real login check + session later
async fn login_submit_handler() -> Redirect {
    Redirect::to("/")
}

// Stub: replace with real account creation + SQLx insert later
async fn signup_submit_handler() -> Redirect {
    Redirect::to("/")
}

async fn profile_handler() -> Html<String> {
    Html(ProfileTemplate { active: "profile" }.render().unwrap())
}

async fn notifications_handler() -> Html<String> {
    Html(NotificationsTemplate { active: "profile" }.render().unwrap())
}

// Activity = full transaction history (bottom nav tab)
async fn activity_handler() -> Html<String> {
    Html(ActivityTemplate { active: "activity" }.render().unwrap())
}

// Link a bank account or card (used by the withdraw flow and profile)
async fn link_account_handler() -> Html<String> {
    Html(LinkAccountTemplate { active: "profile" }.render().unwrap())
}

// Any unknown URL lands here
async fn not_found_handler() -> (StatusCode, Html<String>) {
    (
        StatusCode::NOT_FOUND,
        Html(NotFoundTemplate { active: "" }.render().unwrap()),
    )
}

// Withdrawal flow: plan -> amount -> destination -> review.
// Choices travel in the query string until we have real data from SQLx.
async fn withdraw_plan_handler() -> Html<String> {
    Html(WithdrawPlanTemplate { active: "home" }.render().unwrap())
}

async fn withdraw_amount_handler() -> Html<String> {
    Html(WithdrawAmountTemplate { active: "home" }.render().unwrap())
}

async fn withdraw_destination_handler() -> Html<String> {
    Html(WithdrawDestinationTemplate { active: "home" }.render().unwrap())
}

async fn withdraw_review_handler() -> Html<String> {
    Html(WithdrawReviewTemplate { active: "home" }.render().unwrap())
}

// Group loan flow: request -> review -> detail (all loans wait for admin approval).
async fn loan_request_handler() -> Html<String> {
    Html(LoanRequestTemplate { active: "groups" }.render().unwrap())
}

async fn loan_review_handler() -> Html<String> {
    Html(LoanReviewTemplate { active: "groups" }.render().unwrap())
}

async fn loan_detail_handler() -> Html<String> {
    Html(LoanDetailTemplate { active: "groups" }.render().unwrap())
}

// Group rules and the admin approvals queue (group id ignored until SQLx).
async fn group_rules_handler() -> Html<String> {
    Html(GroupRulesTemplate { active: "groups" }.render().unwrap())
}

async fn group_approvals_handler() -> Html<String> {
    Html(GroupApprovalsTemplate { active: "groups" }.render().unwrap())
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(dashboard_handler))
        .route("/login", get(login_handler).post(login_submit_handler))
        .route("/signup", axum::routing::post(signup_submit_handler))
        .route("/plans", get(plans_handler))
        .route("/plans/new", get(new_plan_handler).post(create_plan_handler))
        // axum 0.8 wants "/plans/{id}" instead
        .route("/plans/:id", get(plan_detail_handler))
        .route("/groups", get(groups_handler))
        .route("/groups/new", get(new_group_handler).post(create_group_handler))
        // axum 0.8 wants "/groups/{id}" instead
        .route("/groups/:id", get(group_detail_handler))
        .route("/deposit", get(deposit_handler))
        .route("/withdraw", get(withdraw_plan_handler))
        .route("/withdraw/amount", get(withdraw_amount_handler))
        .route("/withdraw/destination", get(withdraw_destination_handler))
        .route("/withdraw/review", get(withdraw_review_handler))
        .route("/loans/new", get(loan_request_handler))
        .route("/loans/review", get(loan_review_handler))
        .route("/loans/detail", get(loan_detail_handler))
        .route("/rules", get(group_rules_handler))
        .route("/approvals", get(group_approvals_handler))
        .route("/activity", get(activity_handler))
        .route("/accounts/link", get(link_account_handler))
        .route("/profile", get(profile_handler))
        .route("/notifications", get(notifications_handler))
        .fallback(not_found_handler);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    println!("Server running at http://127.0.0.1:3000");
    axum::serve(listener, app).await.unwrap();
}
