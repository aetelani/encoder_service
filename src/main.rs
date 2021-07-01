use std::collections::HashMap;
use std::os::raw::c_char;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::mpsc::channel;
use std::thread;

use ctrlc;
use log::{debug, error, info};
use rust_baltech_sdk_bindings::*;
use serde_json;

use encoding::is_selected as select_fn;
use encoding::try_auth_desfire as encode_fn;
use serde_json::Value;

#[cfg(test)]
mod test {
    #[test]
    fn todo() {}
}

#[cfg_attr(feature = "internal", path = "encoding_internal.rs")]
mod encoding;
mod key;

fn main() {
    let init_run_first_time = env_logger::builder().is_test(true).try_init().is_ok();

    const PORT: &str = "ttyS3";
    const PARITY_N: c_char = 78;
    const BAUDRATE: c_uint = 115200;
    static RUNNING: AtomicBool = AtomicBool::new(true);

    let params = ContextParams(PORT, BAUDRATE, PARITY_N);
    let _ = init_context(params);

    // BUG open_session returns OK even parameters are wrong. ctx::opened true by default
    open_session().unwrap();

    let _ = ctrlc::set_handler(|| {
        RUNNING.store(false, SeqCst);
    });

    let (enc_result_sender, ctrl_result_receiver) = channel::<String>();
    println!("Ctrl+C for Exit...");
    let encoder_thread_handle = thread::spawn(move || {
        while RUNNING.load(SeqCst) {
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
                match enc_result_sender.send(value) {
                    Ok(_) => continue,
                    Err(_) => {
                        error!("Failed to send Encoding Result");
                        break;
                    }
                }
            }
        }
        drop(enc_result_sender);
    });

    loop {
        match ctrl_result_receiver.recv() {
            Ok(response) => {
                let v: Value = serde_json::from_str(&response).unwrap();
                info!("atr: {}", v["atr"])
            }
            Err(_) => {
                break;
            }
        };
    }
    let _ = close_session();
}
