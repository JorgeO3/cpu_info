// #![allow(unused)]

use async_stream::try_stream;
use axum::{
    extract::State,
    response::sse::{Event, Sse},
    routing::{get, get_service},
    Router,
};
use futures::stream::Stream;
use std::{convert::Infallible, sync::Arc};
use std::{net::SocketAddr, time::Duration};
use tokio::sync::broadcast::Sender;
use tower_http::services::ServeDir;

type CustomSender = Arc<Sender<String>>;

#[tokio::main]
async fn main() {
    let (tx, _rx) = tokio::sync::broadcast::channel::<String>(1);

    let sender = Arc::new(tx);
    let sender2 = Arc::clone(&sender);
    tokio::spawn(async move {
        server(sender2).await;
    })
    .await
    .unwrap();

    tokio::spawn(async move {
        cpu_info(sender);
    })
    .await
    .unwrap();
}

async fn server(state: CustomSender) {
    let mux = Router::new()
        .route("/", get_service(ServeDir::new("./assets")))
        .route("/sse", get(sse_handler))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    axum::Server::bind(&addr)
        .serve(mux.into_make_service())
        .await
        .unwrap();
}

fn cpu_info(tx: CustomSender) {
    use sysinfo::{CpuExt, System, SystemExt};

    let mut sys = System::new_all();
    sys.refresh_all();
    loop {
        let usage = sys
            .cpus()
            .iter()
            .map(|cpu| format!("{}: {}", cpu.name(), cpu.cpu_usage()))
            .collect::<Vec<String>>();

        let data = usage.join(",");
        tx.send(data).expect("Error");

        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
}

async fn sse_handler(
    State(state): State<CustomSender>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut receiver = state.subscribe();
    Sse::new(try_stream! {
        loop {
            match receiver.recv().await {
                Ok(i) => {
                    println!("{}", i);
                    yield Event::default().event("message").data(i);
                }
                Err(e) => {
                    tracing::error!("Failed to get {}", e);
                }
            }
        }
    })
    .keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive-text"),
    )
}
