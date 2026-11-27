//! Two endpoints on loopback: who gets through to a listener, and what each side learns.

use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    time::Duration,
};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{API, Endpoint, Gate, Identity, NoPath};

const LOOPBACK: SocketAddr =
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0));
/// Long enough for a handshake on loopback, which takes milliseconds.
const SOON: Duration = Duration::from_secs(5);
/// How long a dial that must fail is given. QUIC sends its first packet at once.
const NEVER: Duration = Duration::from_millis(300);

fn listener(gate: Gate) -> (Endpoint, SocketAddr) {
    let endpoint = Endpoint::listener(
        LOOPBACK,
        Identity::generate().unwrap(),
        &[API],
        gate,
    )
    .unwrap();
    let address = endpoint.local_addr().unwrap();
    (endpoint, address)
}

fn dialer() -> Endpoint {
    Endpoint::dialer(LOOPBACK, Identity::generate().unwrap()).unwrap()
}

#[tokio::test]
async fn only_an_introduced_dialer_reaches_a_listener() {
    let (core, address) = listener(Gate::Introduced);
    let app = dialer();
    let neighbour = dialer();

    let core_id = core.id();

    // Nobody introduced the dialer: its packets are dropped unanswered.
    let stranger = app.dial(core_id, &[address], API, NEVER).await;
    assert_eq!(stranger.err(), Some(NoPath));

    core.expect(app.id(), &app.candidates(&[]).await);
    let serving = tokio::spawn(async move {
        // Only the introduced dialer is kept. The stranger's abandoned attempt may still say
        // goodbye, now that its address is let in.
        let connection = loop {
            let incoming = core.accept().await.unwrap();
            if let Ok(connection) = incoming.accept().await {
                break connection;
            }
        };
        let mut stream = connection.accept().await.unwrap();
        let mut heard = [0; 5];
        stream.read_exact(&mut heard).await.unwrap();
        stream.write_all(b"world").await.unwrap();
        // Kept open: dropping a connection closes it before what was written has left.
        (connection, heard)
    });
    // Another program at the same address, as behind the same carrier's router, is turned away
    // once it shows its certificate.
    let neighbouring = neighbour.dial(core_id, &[address], API, SOON).await;
    let refused = match neighbouring {
        Ok(connection) => tokio::time::timeout(SOON, connection.accept())
            .await
            .is_ok_and(|stream| stream.is_none()),
        Err(NoPath) => true,
    };
    assert!(refused, "a dialer nobody introduced got in");

    let connection = app.dial(core_id, &[address], API, SOON).await.unwrap();
    let mut stream = connection.open().await.unwrap();
    stream.write_all(b"hello").await.unwrap();
    let mut answer = [0; 5];
    stream.read_exact(&mut answer).await.unwrap();
    assert_eq!(&answer, b"world");

    let (served, heard) = serving.await.unwrap();
    assert_eq!(served.peer(), app.id(), "the listener learns who dialed");
    assert_eq!(served.alpn().as_deref(), Some(API));
    assert_eq!(&heard, b"hello");
}

#[tokio::test]
async fn a_dialer_trusts_only_the_certificate_it_was_told() {
    let (core, address) = listener(Gate::IntroducedOrLocal);
    let app = dialer();
    tokio::spawn(async move {
        while let Some(incoming) = core.accept().await {
            let _ = incoming.accept().await;
        }
    });
    let another = Identity::generate().unwrap().id();
    let wrong = app.dial(another, &[address], API, SOON).await;
    assert_eq!(wrong.err(), Some(NoPath));
}

#[tokio::test]
async fn the_first_candidate_to_answer_wins() {
    let (core, address) = listener(Gate::IntroducedOrLocal);
    let core_id = core.id();
    let app = dialer();
    tokio::spawn(async move {
        while let Some(incoming) = core.accept().await {
            tokio::spawn(incoming.accept());
        }
    });
    // Nothing listens on the first.
    let dead = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 9);
    let connection = app
        .dial(core_id, &[dead, address], API, SOON)
        .await
        .unwrap();
    assert_eq!(connection.remote(), address);
}
