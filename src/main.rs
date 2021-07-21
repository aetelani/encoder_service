#![feature(proc_macro_hygiene, decl_macro)]
use std::collections::HashMap;
use std::os::raw::c_char;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::{fs, io, process, thread};

use log::{debug, error, info};
use rust_baltech_sdk_bindings::*;
use serde_json;

use encoding::is_selected as select_fn;
use encoding::try_auth_desfire as encode_fn;
use rocket::futures::io::Cursor;
use rocket::futures::{SinkExt, StreamExt, TryFutureExt};
use rocket::tokio;
use rocket::tokio::runtime::Runtime;
use serde_json::Value;
use std::cmp::Ordering;
use std::fs::File;
use std::fs::OpenOptions;
use std::future::Future;
use std::io::{Read, Write};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate rocket;

#[cfg_attr(feature = "internal", path = "secret/encoding_internal.rs")]
mod encoding;

use encoder_lib::Key;
use std::str::FromStr;

static ENCODER_RUNNING: AtomicBool = AtomicBool::new(true);
lazy_static! {
    static ref ENCODER_JOB: AtomicUsize = AtomicUsize::new(EncoderJob::new().0);
}

struct EncoderJob(usize);
impl EncoderJob {
    fn new() -> Self {
        Self(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as usize,
        )
    }
}

#[get("/stop/<name>")]
fn hello(name: &str) -> String {
    if name == "encoder" {
        ENCODER_RUNNING.store(false, SeqCst);
    }
    format!("Hello, {}", name)
}

#[get("/log/<id>")]
fn get_log_with_id(id: String) -> String {
    let mut file = OpenOptions::new()
        .read(true)
        .open(log_get_dir_name() + &format!("/job_{}.log", &id))
        .unwrap();
    let result = &mut String::new();
    file.read_to_string(result);
    result.to_string()
}

#[get("/logs")]
fn list_logs() -> String {
    let mut entries = fs::read_dir(log_get_dir_name())
        .unwrap()
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, io::Error>>()
        .unwrap();

    format!(
        "{:?}",
        entries
            .iter()
            .map(|e| { e.clone().into_os_string().into_string().unwrap() })
    )
}

fn main() {
    let init_run_first_time = env_logger::builder().is_test(false).try_init().is_ok();

    let mut encoder_delay: Option<Duration> = None;

    if let Some(value) = option_env!("ENCODER_LOOP_DELAY_MS") {
        if let Ok(delay) = u64::from_str(value) {
            if delay > 0 {
                encoder_delay.replace(Duration::from_millis(delay)); // ttyS<X>;
            }
        }
    }

    info!("Running Encoder loop with delay: {:?}", encoder_delay);

    const PORT: &str = env!("BALTECH_PORT"); // ttyS<X>;
    const PARITY_N: c_char = 78;
    const BAUDRATE: c_uint = 115200;

    let params = ContextParams(PORT, BAUDRATE, PARITY_N);
    let _ = init_context(params);

    // BUG open_session returns OK even parameters are wrong. ctx::opened true by default
    open_session().unwrap();

    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(1);

    let rt = Runtime::new().unwrap();

    let _encoder_thread_handle = encoder(encoder_delay, tx);

    let _handle = rt.spawn(logger(rx));

    let web_api_running = rocket::build()
        .mount("/", routes![hello, list_logs, get_log_with_id])
        .launch();

    println!("Ctrl+C for Exit...");
    rt.block_on(web_api_running);
    info!("Cleaning resources and Exiting...");

    ENCODER_RUNNING.store(false, SeqCst);

    let _ = close_session();
}

fn log_get_dir_name() -> String {
    let pid = process::id();
    format!("log_{}", pid)
}

fn write_log_entry(data: &String) {
    let dir_name = &log_get_dir_name();
    let result = fs::create_dir(&dir_name);
    if let Ok(()) = result {
        debug!("Created log dir: {}", &dir_name);
    } else {
        trace!("failed to create log dir {:?}", result);
    }
    let file_name = format!("{}/job_{}.log", &dir_name, ENCODER_JOB.load(SeqCst));
    let mut file = OpenOptions::new()
        .truncate(false)
        .create(true)
        .append(true)
        .open(file_name)
        .unwrap();
    write!(file, "{}\n", data);
    trace!("log: {}", data);
    file.flush();
}

async fn logger(mut ctrl_result_receiver: tokio::sync::mpsc::Receiver<String>) {
    loop {
        match ctrl_result_receiver.recv().await {
            Some(response) => {
                let v: Value = serde_json::from_str(&response).unwrap();
                info!("atr: {}", v["atr"]);
                write_log_entry(&response)
            }
            None => {
                break;
            }
        };
    }
}

fn encoder(
    sleep_duration: Option<Duration>,
    enc_result_sender: tokio::sync::mpsc::Sender<String>,
) -> JoinHandle<()> {
    let handle = thread::spawn(move || {
        while ENCODER_RUNNING.load(SeqCst) {
            if let Some(duration_ms) = sleep_duration {
                thread::sleep(duration_ms);
            }
            if select_fn() {
                let result = match encode_fn() {
                    Ok(response) => response,
                    Err(err) => {
                        info!("Failed to Encode Ticket: {}", err);
                        // TODO: Retry Logic
                        continue;
                    }
                };
                let value = match serde_json::to_string(&result) {
                    Ok(result) => result,
                    Err(_) => {
                        error!("Failed to convert Encoding result to JSON");
                        continue;
                    }
                };
                match enc_result_sender.blocking_send(value) {
                    Ok(_) => {
                        continue;
                    }
                    Err(_) => {
                        error!("Failed to send Encoding Result");
                        break;
                    }
                }
            }
        }
        drop(enc_result_sender);
    });
    handle
}
