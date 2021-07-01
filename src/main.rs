#![feature(maybe_uninit_extra)]

use log::{info, };

use rust_baltech_sdk_bindings::{Buf, get_version, BrpResult, ContextParams, create_context, open_session, get_firmware_version, close_session,
                                gen_brp_VHL_Select, brp_protocol, par_brp_VHL_Select, brp_CardFamilies};
use std::os::raw::{c_char, c_uint};
use std::mem::MaybeUninit;


fn select_exec_and_close(context: brp_protocol) -> BrpResult<()> {
    let mut par = par_brp_VHL_Select::default();

    par.CardFamiliesFilter.0.write(brp_CardFamilies {
        Iso14443A: true,
        ..brp_CardFamilies::default()
    });

    let _ = gen_brp_VHL_Select(par);
/*
    if vhl_select(context, select_params).is_ok()
        && vhl_get_serial_number(context).is_ok()
        && vhl_get_atr(context).is_ok()
        && desfire_select_application(context, 0).is_ok()
    {
        let _ = desfire_exec_cmd(context, Default::default());
    };
*/
    close_session(context)
}


fn main() {
    info!("Hello, from Baltech SDK bindings! Version: {}", get_version());

    const PORT: &str = "ttyS3";
    const PARITY_N: c_char = 78;
    const BAUDRATE: c_uint = 115200;

    let params = ContextParams ( PORT, BAUDRATE, PARITY_N, );
    let ctx = create_context(params).unwrap();

    // BUG open_session returns OK even parameters are wrong. ctx::opened true by default
    open_session(ctx);

    let firmware_version_result = get_firmware_version(ctx).unwrap();

    select_exec_and_close(ctx).ok();
}
