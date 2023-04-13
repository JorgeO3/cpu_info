#![allow(unused)]

use axum::{
    extract::State,
    response::{sse::Event, Sse},
    routing::{get, get_service},
    Router,
};
use serde::{Deserialize, Serialize, Serializer, };
use std::{
    convert::Infallible,
    net::SocketAddr,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::stream::{self, Stream};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio_stream::StreamExt as _;
use tower_http::services::ServeDir;
use serde_json::Value;

type CusomSender = Arc<Mutex<Sender<Data>>>;
type CustomReceiver = Arc<Mutex<Receiver<Data>>>;

#[derive(Debug, Clone)]
struct ApiState {
    tx: CusomSender,
    rx: CustomReceiver,
}

#[derive(Serialize, Deserialize, Default)]
struct Data {
    values: Vec<(String, f32)>,
}

#[tokio::main]
async fn main() {
    let (tx, rx) = mpsc::channel::<Data>(10);
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

fn cpu_info(tx: CusomSender, rx: CustomReceiver) {
    use sysinfo::{CpuExt, System, SystemExt};

    let mut sys = System::new_all();
    sys.refresh_all();
    loop {
        let usage = sys
            .cpus()
            .iter()
            .map(|cpu| (cpu.name().to_string(), cpu.cpu_usage()))
            .collect::<Vec<(String, f32)>>();

        tx.lock().unwrap().send(Data { values: usage });

        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
}

async fn sse_handler(
    State(state): State<ApiState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let data = state.rx.lock().unwrap().recv().await.unwrap();

    let stream = stream::repeat_with(|| Event::default().data(serde_json::json!({name: "".to_string()})))
        .map(Ok)
        .throttle(Duration::from_secs(1));

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive-text"),
    )
}
