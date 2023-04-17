use axum::{
    extract::State,
    response::sse::{Event, Sse},
    routing::{get, get_service},
    Router,
};
use futures_util::stream::{self, Stream};
use std::{convert::Infallible, sync::Arc};
use std::{net::SocketAddr, time::Duration};
use tokio::sync::watch::Sender;
#[allow(unused)]
use tokio_stream::StreamExt as _;
use tower_http::{services::ServeDir, trace::TraceLayer};

type CustomSender = Arc<Sender<String>>;

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_LOG", "debug");
    std::env::set_var("RUST_BACKTRACE", "1");
    env_logger::init();

    let (tx, _rx) = tokio::sync::watch::channel(String::from("Hello"));

    let sender = Arc::new(tx);
    let sender2 = Arc::clone(&sender);

    let thread_one = tokio::spawn(async move {
        server(sender2).await;
    });

    let thread_two = tokio::spawn(async move {
        cpu_info(sender).await;
    });

    thread_one.await.unwrap();
    thread_two.await.unwrap();
}

async fn server(state: CustomSender) {
    let mux = Router::new()
        .layer(TraceLayer::new_for_http())
        .route("/", get_service(ServeDir::new("./assets")))
        .route("/sse", get(sse_handler))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    axum::Server::bind(&addr)
        .serve(mux.into_make_service())
        .await
        .unwrap();
}

async fn cpu_info(tx: CustomSender) {
    use sysinfo::{CpuExt, System, SystemExt};

    let mut sys = System::new_all();
    loop {
        sys.refresh_all();
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
    Sse::new(
        stream::repeat_with(move || Event::default().data(state.borrow().clone()))
            .map(Ok)
            .throttle(Duration::from_secs(1)),
    )
    .keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive-text"),
    )
}