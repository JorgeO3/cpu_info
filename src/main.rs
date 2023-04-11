#![allow(unused)]

use axum::{
    extract::State,
    response::{sse::Event, Sse},
    routing::{get, get_service},
    Router,
};
use std::{
    convert::Infallible,
    net::SocketAddr,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    time::Duration,
};

use futures_util::stream::{self, Stream};
use tokio_stream::StreamExt as _;
use tower_http::services::ServeDir;

type CusomSender = Arc<Mutex<Sender<u8>>>;
type CustomReceiver = Arc<Mutex<Receiver<u8>>>;

#[derive(Clone)]
struct ApiState {
    tx: CusomSender,
    rx: CustomReceiver,
}

#[tokio::main]
async fn main() {
    let (tx, rx) = mpsc::channel::<u8>();
    let (sender, receiver) = (Arc::new(Mutex::new(tx)), Arc::new(Mutex::new(rx)));

    let tx_sender = Arc::clone(&sender);
    let rx_receiver = Arc::clone(&receiver);

    let api_state = ApiState {
        tx: Arc::clone(&sender),
        rx: Arc::clone(&receiver),
    };

    tokio::spawn(async move {
        server(api_state).await;
    })
    .await;

    tokio::spawn(async move {
        cpu_info(tx_sender, rx_receiver);
    })
    .await;
}

async fn server(state: ApiState) {
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

async fn cpu_info(tx: CusomSender, rx: CustomReceiver) {
    use sysinfo::{CpuExt, System, SystemExt};

    let mut sys = System::new();

    loop {
        sys.refresh_cpu();
        for cpu in sys.cpus() {
            print!("{}% ", cpu.cpu_usage());
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

async fn sse_handler(
    State(state): State<ApiState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = stream::repeat_with(|| Event::default().data("hi!"))
        .map(Ok)
        .throttle(Duration::from_secs(1));

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive-text"),
    )
}
