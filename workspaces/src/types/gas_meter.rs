use std::{
    sync::{
        mpsc::{self, TryRecvError},
        Arc, Mutex,
    },
    thread::JoinHandle,
};

use crate::Worker;

use super::Gas;
use crate::result::Result;

/// Allows you to meter the amount of gas consumed by transaction(s).
/// Note: This only works with parallel transactions that resolve to [`crate::Result::ExecutionFinalResult`]
/// Example
/// ```rust, ignore, no_run
/// let mut worker = workspaces::sandbox().await?;
/// let meter = GasMeter::now(&mut worker);
///
/// let wasm = std::fs::read(STATUS_MSG_WASM_FILEPATH)?;
/// let contract = worker.dev_deploy(&wasm).await?;
///
/// contract
///    .call("set_status")
///    .args_json(json!({
///        "message": "hello_world",
///    }))
///    .transact()
///    .await?;
///
/// println!("Total Gas consumed: {}", meter.elapsed()?);
/// ```
pub struct GasMeter {
    // FIXME: https://users.rust-lang.org/t/spawn-threads-and-join-in-destructor/1613/2
    daemon_ref: Option<JoinHandle<Result<()>>>,
    meter: Arc<Mutex<Meter>>,
}

struct Meter {
    close: bool,
    elapsed: Gas,
}

impl GasMeter {
    /// Create a new gas meter with 0 gas consumed.
    pub fn now<T: ?Sized>(worker: &mut Worker<T>) -> Self {
        let (tx, rx) = mpsc::channel();

        worker.on_transact.push(tx);

        let meter = Arc::new(Mutex::new(Meter {
            close: false,
            elapsed: 0,
        }));

        let handle = {
            let meter_ = meter.clone();
            std::thread::spawn(move || -> Result<()> {
                loop {
                    let mut meter = meter_.lock()?;

                    match rx.try_recv() {
                        Ok(gas) => {
                            *meter = Meter {
                                close: false,
                                elapsed: meter.elapsed + gas,
                            };
                        }
                        Err(err) => {
                            if err == TryRecvError::Disconnected {
                                return Ok(());
                            }

                            if meter.close {
                                drop(rx);
                                // println!("daemon cleaned up");
                                return Ok(()); // naive way to stop the thread.
                            }
                        }
                    }

                    // allow for close to be called if waiting
                    drop(meter)
                }
            })
        };

        Self {
            daemon_ref: Some(handle),
            meter: meter.clone(),
        }
    }

    /// Get the total amount of gas consumed.
    pub fn elapsed(&self) -> Result<Gas> {
        Ok(self.meter.lock()?.elapsed)
    }

    /// Reset the gas consumed to 0.
    pub fn reset(&self) -> Result<()> {
        *self.meter.lock()? = Meter {
            close: false,
            elapsed: 0,
        };

        Ok(())
    }
}

impl Drop for GasMeter {
    fn drop(&mut self) {
        // close the daemon.
        *self.meter.lock().unwrap() = Meter {
            close: true,
            elapsed: 0,
        };

        _ = self.daemon_ref.take().unwrap().join().unwrap();
    }
}
