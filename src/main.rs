use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, get_service},
    Router,
};
use std::{convert::Infallible, net::SocketAddr, sync::Arc};
use async_stream::try_stream;
use futures_util::stream::Stream;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tower_http::services::ServeDir;

type CusomSender<'a> = Arc<Sender<Vec<String>>>;
type CustomReceiver<'a> = Arc<Receiver<Vec<String>>>;

#[derive(Debug, Clone)]
struct ApiState<'a> {
    tx: CusomSender<'a>,
    rx: CustomReceiver<'a>,
}

#[tokio::main]
async fn main() {
    let (tx, rx) = mpsc::channel::<Vec<(&str, f32)>>(1);
    let (sender, receiver) = (Arc::new(tx), Arc::new(rx));

    let api_state = ApiState {
        tx: Arc::clone(&sender),
        rx: Arc::clone(&receiver),
    };

    tokio::spawn(async move {
        server(api_state).await;
    })
    .await;

    tokio::spawn(async move {
        cpu_info(sender, receiver);
    })
    .await;
}

async fn server<'a>(state: ApiState<'a>) {
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
            .map(|cpu| format!("{}: {}", cpu.name(), cpu.cpu_usage()).as_ref())
            .collect::<Vec<&str>>();

        tx.send(usage);

        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
}

async fn sse_handler<'a>(
    State(state): State<ApiState<'a>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    Sse::new(try_stream! {
        loop {
            match state.rx.recv().await {
                Some(val) => {
                    yield Event::default().data(val);
                },
                None => {
                    tracing::error!("Failed to get");
                }
            };
        }
    })
    .keep_alive(KeepAlive::default())
}
