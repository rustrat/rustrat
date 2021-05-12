use crate::core::messages::*;

use rustrat_common::encryption::PublicKey;
use rustrat_common::messages as common_messages;

use serde::{Deserialize, Serialize};
use std::convert::{Infallible, TryInto};
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use warp::http::StatusCode;
use warp::{Filter, Reply};

fn with_tx(tx: Sender<Job>) -> impl Filter<Extract = (Sender<Job>,), Error = Infallible> + Clone {
    warp::any().map(move || tx.clone())
}

async fn checkin(
    tx: Sender<Job>,
    pkey_string: String,
) -> Result<warp::reply::Response, Infallible> {
    let pkey_vec = base64::decode_config(pkey_string, base64::URL_SAFE_NO_PAD);
    let pkey: PublicKey = match pkey_vec {
        Ok(vec) => match vec.try_into() {
            Ok(pkey) => pkey,
            Err(_) => {
                return Ok(StatusCode::NOT_FOUND.into_response());
            }
        },
        Err(_) => {
            return Ok(StatusCode::NOT_FOUND.into_response());
        }
    };

    // TODO prettify? Implement From for various structs?
    let (reply_tx, reply_rx) = oneshot::channel();
    let job = Job {
        message: Box::new(Task::RatToServer(
            common_messages::rat_to_server::Message::CheckIn(pkey),
        )),
        tx: reply_tx,
    };

    // TODO not silently drop error? Will reply_rx wait forever if channel is closed or exit as tx is dropped?
    let _ = tx.send(job).await;

    // TODO this is ugly, should probably not be _this_ ugly
    match reply_rx.await {
        Ok(reply_result) => match reply_result {
            Ok(reply) => match *reply {
                crate::core::messages::Reply::ServerToRat(msg) => {
                    let reply_vec = common_messages::serialize(&msg).unwrap();
                    Ok(reply_vec.into_response())
                }
            },
            Err(e) => match e {
                crate::error::Error::PublicKeyAlreadyExistsOnCheckin => {
                    Ok(StatusCode::CONFLICT.into_response())
                }
                _ => Ok(StatusCode::INTERNAL_SERVER_ERROR.into_response()),
            },
        },
        Err(_) => Ok(StatusCode::INTERNAL_SERVER_ERROR.into_response()),
    }
}

#[derive(Deserialize, Serialize)]
struct GetRequest {
    t: String,
}

async fn encrypted_get(
    tx: Sender<Job>,
    request: GetRequest,
) -> Result<warp::reply::Response, Infallible> {
    let serialized_message = base64::decode_config(request.t, base64::URL_SAFE_NO_PAD);
    return match serialized_message {
        Ok(vec) => encrypted_request(tx, &vec).await,
        Err(_) => Ok(StatusCode::NOT_FOUND.into_response()),
    };
}

async fn encrypted_request<T: AsRef<[u8]>>(
    tx: Sender<Job>,
    request: T,
) -> Result<warp::reply::Response, Infallible> {
    let msg = match common_messages::deserialize::<common_messages::rat_to_server::Message>(
        request.as_ref(),
    ) {
        Ok(msg) => msg,
        Err(_) => {
            return Ok(StatusCode::BAD_REQUEST.into_response());
        }
    };

    // TODO prettify? Implement From for various structs?
    let (reply_tx, reply_rx) = oneshot::channel();
    let job = Job {
        message: Box::new(Task::RatToServer(msg)),
        tx: reply_tx,
    };

    // TODO not silently drop error? Will reply_rx wait forever if channel is closed or exit as tx is dropped?
    let _ = tx.send(job).await;

    match reply_rx.await {
        Ok(reply_result) => match reply_result {
            Ok(reply) => match *reply {
                crate::core::messages::Reply::ServerToRat(msg) => {
                    let reply_vec = common_messages::serialize(&msg).unwrap();
                    Ok(reply_vec.into_response())
                }
            },
            Err(e) => match e {
                crate::error::Error::PublicKeyAlreadyExistsOnCheckin => {
                    Ok(StatusCode::CONFLICT.into_response())
                }
                _ => Ok(StatusCode::INTERNAL_SERVER_ERROR.into_response()),
            },
        },
        Err(_) => Ok(StatusCode::INTERNAL_SERVER_ERROR.into_response()),
    }
}

// TODO Accept CoreTask instead? Or a separate struct containing db pool and tx?
// TODO trait for listeners etc
// TODO handle rejections https://github.com/seanmonstar/warp/blob/master/examples/rejections.rs
pub async fn run(tx: Sender<Job>) {
    let checkin = warp::get()
        .and(warp::path::end())
        .and(with_tx(tx.clone()))
        .and(warp::filters::cookie::cookie("uid"))
        .and_then(checkin);

    let encrypted_get = warp::get()
        .and(warp::path("renew"))
        .and(warp::path::end())
        .and(with_tx(tx.clone()))
        .and(warp::query::<GetRequest>())
        .and_then(encrypted_get);

    let encrypted_post = warp::post()
        .and(warp::path("sync"))
        .and(warp::path::end())
        .and(with_tx(tx.clone()))
        .and(warp::body::bytes())
        .and_then(encrypted_request);

    let request_log = warp::log(module_path!());

    warp::serve(
        checkin
            .or(encrypted_get)
            .or(encrypted_post)
            .with(request_log),
    )
    .run(([127, 0, 0, 1], 1337))
    .await;
}
