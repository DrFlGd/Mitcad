// SPDX-License-Identifier: MIT
//! A test of how C++ exceptions cross the bridges (mitcad#937). Every
//! bridge includes `bridge/trycatch.hpp`, whose `rust::behavior::trycatch`
//! makes any exception under a `Result` function of C++ an `Err`: OCCT's
//! `Standard_Failure` with its message, a `std::exception`, and anything
//! else, which with cxx's own `trycatch` would end the process. The C++
//! test `core.bridge` calls [`bridge_exception_probe`] with each kind.

use super::failed;

#[cxx::bridge(namespace = "mitcad::bridge")]
pub mod ffi {
    unsafe extern "C++" {
        include!("bridge/exception_probe.hpp");
        include!("bridge/trycatch.hpp");

        /// Throws the exception `kind` names ("occt", "occt-empty", "std",
        /// "other") with `message`, outside any geometry operation.
        fn throw_for_bridge_test(kind: &str, message: &str) -> Result<()>;
    }

    extern "Rust" {
        /// For tests (`core.bridge`): what a bridge call that throws `kind`
        /// returns, as the kernel's error: "Err: <message>", or "Ok".
        fn bridge_exception_probe(kind: &str, message: &str) -> String;
    }
}

fn bridge_exception_probe(kind: &str, message: &str) -> String {
    match ffi::throw_for_bridge_test(kind, message).map_err(failed) {
        Ok(()) => "Ok".to_owned(),
        Err(error) => format!("Err: {error}"),
    }
}
