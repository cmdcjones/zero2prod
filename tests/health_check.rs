//! tests/health_check.rs

use secrecy::ExposeSecret;
use sqlx::PgPool;
use std::net::TcpListener;
use std::sync::LazyLock;
use zero2prod::configuration::get_configuration;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

static TRACING: LazyLock<()> = LazyLock::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::sink);
        init_subscriber(subscriber);
    }
});

pub struct TestApp {
    app_address: String,
    db_pool: PgPool,
}

#[tokio::test]
async fn health_check_works() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/health_check", &app.app_address))
        .send()
        .await
        .expect("Failed to execute request");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[tokio::test]
async fn subscribe_returns_201_for_valid_data() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();

    let body = "name=dom%20jones&email=dom%40me.com";
    let response = client
        .post(&format!("{}/subscriptions", &app.app_address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request");

    let saved = sqlx::query!("SELECT email, name FROM subscriptions")
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscriptions");

    assert_eq!(saved.email, "dom@me.com");
    assert_eq!(saved.name, "dom jones");
    assert_eq!(201, response.status().as_u16());
}

#[tokio::test]
async fn subscribe_returns_400_for_invalid_data() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();

    let test_cases = vec![
        ("name=dom%20jones", "missing email"),
        ("email=dom%40me.com", "missing name"),
        ("", "empty request body"),
    ];

    for (invalid_body, error) in test_cases {
        let response = client
            .post(&format!("{}/subscriptions", &app.app_address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request");

        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 Bad Request when the payload was {}",
            error
        )
    }
}

async fn spawn_app() -> TestApp {
    LazyLock::force(&TRACING);
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to a random port");
    let port = listener.local_addr().unwrap().port();

    let configuration = get_configuration().expect("Failed to read configuration.");
    let db_pool = PgPool::connect(&configuration.database.connection_string().expose_secret())
        .await
        .expect("Failed to connect to Postgres.");

    let server =
        zero2prod::startup::run(listener, db_pool.clone()).expect("Failed to bind address");
    let _ = tokio::spawn(server);
    let app_address = format!("http://127.0.0.1:{}", port);
    TestApp {
        app_address,
        db_pool,
    }
}
